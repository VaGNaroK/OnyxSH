# onyxsh/terminal/tunnel_manager.py
"""
SSH Tunnel and Port Forwarding Service for OnyxSH.
Manages background SSH tunnels (Local -L, Remote -R, Dynamic SOCKS5 -D),
handling lifecycle, health checks, auto-reconnection, and status signals.
"""

from __future__ import annotations

import os
import shutil
import socket
import subprocess
import threading
import time
import uuid
from dataclasses import dataclass, field
from typing import Any, Callable, Dict, List, Optional

import gi

gi.require_version("GObject", "2.0")
gi.require_version("GLib", "2.0")
from gi.repository import GLib, GObject

from ..sessions.models import SessionItem
from ..utils.logger import get_logger
from ..utils.translation_utils import _


@dataclass
class SSHTunnel:
    """Represents a configured SSH tunnel."""

    id: str = field(default_factory=lambda: str(uuid.uuid4())[:8])
    name: str = ""
    session_name: str = ""
    session_item: Optional[SessionItem] = None
    type: str = "local"  # "local", "remote", "dynamic"
    local_host: str = "127.0.0.1"
    local_port: int = 8080
    remote_host: str = "localhost"
    remote_port: int = 80
    auto_start: bool = False

    # Runtime state
    status: str = "stopped"  # "stopped", "starting", "active", "error"
    error_message: str = ""
    process: Optional[subprocess.Popen] = None
    started_at: Optional[float] = None

    def get_display_source(self) -> str:
        """Returns human-readable source address."""
        if self.type == "remote":
            return f"remote:{self.remote_port}"
        return f"{self.local_host}:{self.local_port}"

    def get_display_target(self) -> str:
        """Returns human-readable destination address."""
        if self.type == "dynamic":
            return "SOCKS5 Proxy"
        if self.type == "remote":
            return f"{self.local_host}:{self.local_port}"
        return f"{self.remote_host}:{self.remote_port}"

    def get_type_label(self) -> str:
        """Returns translated type description."""
        if self.type == "dynamic":
            return _("Dynamic (SOCKS5)")
        elif self.type == "remote":
            return _("Remote (-R)")
        return _("Local (-L)")


class SSHTunnelManager(GObject.Object):
    """
    Singleton service that manages active SSH tunnels across the application.
    """

    __gsignals__ = {
        "tunnel-status-changed": (
            GObject.SignalFlags.RUN_FIRST,
            None,
            (str, str),  # tunnel_id, status
        ),
        "tunnel-added": (
            GObject.SignalFlags.RUN_FIRST,
            None,
            (str,),  # tunnel_id
        ),
        "tunnel-removed": (
            GObject.SignalFlags.RUN_FIRST,
            None,
            (str,),  # tunnel_id
        ),
    }

    _instance: Optional[SSHTunnelManager] = None

    def __init__(self) -> None:
        super().__init__()
        self.logger = get_logger("onyxsh.terminal.tunnel_manager")
        self._tunnels: Dict[str, SSHTunnel] = {}
        self._lock = threading.Lock()
        self._health_check_source_id: Optional[int] = None
        self._start_health_monitor()

    @classmethod
    def get_instance(cls) -> SSHTunnelManager:
        if cls._instance is None:
            cls._instance = SSHTunnelManager()
        return cls._instance

    def _start_health_monitor(self) -> None:
        """Schedules periodic health checking of active tunnels."""
        if self._health_check_source_id is None:
            self._health_check_source_id = GLib.timeout_add_seconds(
                3, self._check_tunnels_health
            )

    def _check_tunnels_health(self) -> bool:
        """Inspects running subprocesses to detect unexpected terminations."""
        with self._lock:
            for tunnel_id, tunnel in list(self._tunnels.items()):
                if tunnel.status in ("active", "starting") and tunnel.process:
                    ret_code = tunnel.process.poll()
                    if ret_code is not None:
                        err_out = ""
                        try:
                            if tunnel.process.stderr:
                                err_out = tunnel.process.stderr.read().decode(
                                    "utf-8", errors="replace"
                                ).strip()
                        except Exception:
                            pass

                        tunnel.status = "error" if ret_code != 0 else "stopped"
                        tunnel.error_message = (
                            err_out
                            or _("Process exited with code {code}").format(
                                code=ret_code
                            )
                        )
                        tunnel.process = None
                        self.logger.warning(
                            f"Tunnel {tunnel.name} ({tunnel_id}) exited unexpectedly: {tunnel.error_message}"
                        )
                        GLib.idle_add(
                            self.emit, "tunnel-status-changed", tunnel_id, tunnel.status
                        )
        return True

    def register_tunnel(self, tunnel: SSHTunnel) -> str:
        """Registers a new or existing tunnel into the manager."""
        with self._lock:
            self._tunnels[tunnel.id] = tunnel
        self.logger.info(f"Registered tunnel: {tunnel.name} [{tunnel.type}] ({tunnel.id})")
        GLib.idle_add(self.emit, "tunnel-added", tunnel.id)
        return tunnel.id

    def unregister_tunnel(self, tunnel_id: str) -> bool:
        """Stops and removes a tunnel configuration."""
        self.stop_tunnel(tunnel_id)
        with self._lock:
            if tunnel_id in self._tunnels:
                del self._tunnels[tunnel_id]
                self.logger.info(f"Unregistered tunnel: {tunnel_id}")
                GLib.idle_add(self.emit, "tunnel-removed", tunnel_id)
                return True
        return False

    def get_tunnel(self, tunnel_id: str) -> Optional[SSHTunnel]:
        with self._lock:
            return self._tunnels.get(tunnel_id)

    def get_all_tunnels(self) -> List[SSHTunnel]:
        with self._lock:
            return list(self._tunnels.values())

    def start_tunnel(self, tunnel_id: str) -> bool:
        """Spawns background SSH process for the given tunnel."""
        tunnel = self.get_tunnel(tunnel_id)
        if not tunnel:
            self.logger.error(f"Cannot start tunnel: {tunnel_id} not found")
            return False

        if tunnel.status == "active" and tunnel.process and tunnel.process.poll() is None:
            self.logger.debug(f"Tunnel {tunnel.name} already active")
            return True

        session = tunnel.session_item
        if not session:
            tunnel.status = "error"
            tunnel.error_message = _("No associated SSH session configured")
            self.emit("tunnel-status-changed", tunnel_id, "error")
            return False

        # Build SSH command
        ssh_bin = shutil.which("ssh") or "ssh"
        cmd = [
            ssh_bin,
            "-N",  # Do not execute remote command
            "-T",  # Disable pseudo-tty allocation
            "-o", "ExitOnForwardFailure=yes",
            "-o", "ServerAliveInterval=15",
            "-o", "ServerAliveCountMax=3",
            "-o", "ConnectTimeout=10",
            "-o", "BatchMode=yes",  # Do not prompt interactively on terminal
        ]

        if session.port and session.port != 22:
            cmd.extend(["-p", str(session.port)])

        if session.uses_key_auth() and session.auth_value:
            key_path = os.path.expanduser(session.auth_value)
            if os.path.exists(key_path):
                cmd.extend(["-i", key_path])

        # Forwarding argument
        if tunnel.type == "dynamic":
            spec = f"{tunnel.local_host}:{tunnel.local_port}" if tunnel.local_host else str(tunnel.local_port)
            cmd.extend(["-D", spec])
        elif tunnel.type == "remote":
            spec = f"{tunnel.remote_port}:{tunnel.local_host}:{tunnel.local_port}"
            cmd.extend(["-R", spec])
        else:  # local
            spec = f"{tunnel.local_host}:{tunnel.local_port}:{tunnel.remote_host}:{tunnel.remote_port}"
            cmd.extend(["-L", spec])

        # Destination target
        target = f"{session.user}@{session.host}" if session.user else session.host
        cmd.append(target)

        tunnel.status = "starting"
        tunnel.error_message = ""
        self.emit("tunnel-status-changed", tunnel_id, "starting")

        def _spawn_worker():
            try:
                self.logger.info(f"Starting tunnel process: {' '.join(cmd)}")
                proc = subprocess.Popen(
                    cmd,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    stdin=subprocess.DEVNULL,
                )
                tunnel.process = proc
                tunnel.started_at = time.time()

                # Give it a short moment to detect fast failure (e.g. port conflict or auth failure)
                time.sleep(0.6)
                ret = proc.poll()
                if ret is not None:
                    err = proc.stderr.read().decode("utf-8", errors="replace").strip() if proc.stderr else ""
                    tunnel.status = "error"
                    tunnel.error_message = err or _("Process exited with code {code}").format(code=ret)
                    tunnel.process = None
                    self.logger.error(f"Tunnel {tunnel.name} start failed: {tunnel.error_message}")
                else:
                    tunnel.status = "active"
                    tunnel.error_message = ""
                    self.logger.info(f"Tunnel {tunnel.name} is now ACTIVE (PID: {proc.pid})")

            except Exception as e:
                tunnel.status = "error"
                tunnel.error_message = str(e)
                tunnel.process = None
                self.logger.error(f"Exception starting tunnel {tunnel.name}: {e}")

            GLib.idle_add(self.emit, "tunnel-status-changed", tunnel_id, tunnel.status)

        threading.Thread(target=_spawn_worker, daemon=True).start()
        return True

    def stop_tunnel(self, tunnel_id: str) -> bool:
        """Terminates active background SSH process for the tunnel."""
        tunnel = self.get_tunnel(tunnel_id)
        if not tunnel:
            return False

        if tunnel.process:
            try:
                tunnel.process.terminate()
                tunnel.process.wait(timeout=1.5)
            except Exception:
                try:
                    tunnel.process.kill()
                except Exception:
                    pass

        tunnel.process = None
        tunnel.status = "stopped"
        tunnel.error_message = ""
        tunnel.started_at = None
        self.logger.info(f"Stopped tunnel: {tunnel.name} ({tunnel_id})")
        self.emit("tunnel-status-changed", tunnel_id, "stopped")
        return True

    def restart_tunnel(self, tunnel_id: str) -> bool:
        """Stops and immediately restarts a tunnel."""
        self.stop_tunnel(tunnel_id)
        time.sleep(0.3)
        return self.start_tunnel(tunnel_id)

    def stop_all_tunnels(self) -> None:
        """Stops all running tunnels."""
        with self._lock:
            ids = list(self._tunnels.keys())
        for tid in ids:
            self.stop_tunnel(tid)


def get_ssh_tunnel_manager() -> SSHTunnelManager:
    """Returns singleton SSHTunnelManager instance."""
    return SSHTunnelManager.get_instance()
