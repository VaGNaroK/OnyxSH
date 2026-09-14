# onyxsh/terminal/ssh_health_monitor.py
"""
Proactive SSH Connection Health Monitor & Latency (RTT) Tracker for OnyxSH.

Monitors remote SSH sessions in real-time, calculates network round-trip time (RTT),
detects connection drops and socket resets, and manages smart auto-reconnect workflows
without blocking the UI or terminal responsiveness.
"""

from __future__ import annotations

import socket
import threading
import time
from dataclasses import dataclass
from enum import Enum
from typing import TYPE_CHECKING, Any, Callable, Dict, List, Optional, Tuple

import gi

gi.require_version("Gtk", "4.0")
from gi.repository import GLib

from ..core.signals import AppSignals
from ..utils.logger import get_logger
from ..utils.translation_utils import _

if TYPE_CHECKING:
    from ..sessions.models import SessionItem
    from ..settings.manager import SettingsManager

logger = get_logger("onyxsh.terminal.ssh_health_monitor")


class SSHHealthStatus(Enum):
    """Health and connection status of an SSH session."""

    UNKNOWN = "unknown"
    HEALTHY = "healthy"          # RTT < 150ms (Green)
    DEGRADED = "degraded"        # 150ms <= RTT < 350ms (Yellow)
    POOR = "poor"                # RTT >= 350ms (Orange)
    UNREACHABLE = "unreachable"  # Socket down / timeout / connection refused (Red)
    RECONNECTING = "reconnecting"# Auto-reconnect in progress / countdown active (Blue/Yellow)


@dataclass
class SSHHealthRecord:
    """Consolidated state and metrics for an SSH connection."""

    terminal_id: int
    session_name: str
    host: str
    port: int = 22
    session_item: Optional[SessionItem] = None
    status: SSHHealthStatus = SSHHealthStatus.UNKNOWN
    rtt_ms: Optional[float] = None
    last_checked: float = 0.0
    consecutive_failures: int = 0
    error_message: Optional[str] = None
    is_auto_reconnecting: bool = False
    countdown_seconds: int = 0
    reconnect_attempt: int = 0
    max_reconnect_attempts: int = 5

    def get_badge_label(self) -> str:
        """Returns concise badge label text with status icon and RTT."""
        if self.is_auto_reconnecting and self.countdown_seconds > 0:
            return f"🔄 {self.countdown_seconds}s"
        if self.status == SSHHealthStatus.HEALTHY and self.rtt_ms is not None:
            return f"🟢 {int(self.rtt_ms)}ms"
        if self.status == SSHHealthStatus.DEGRADED and self.rtt_ms is not None:
            return f"🟡 {int(self.rtt_ms)}ms"
        if self.status == SSHHealthStatus.POOR and self.rtt_ms is not None:
            return f"🟠 {int(self.rtt_ms)}ms"
        if self.status == SSHHealthStatus.UNREACHABLE:
            return _("🔴 Inalcançável")
        if self.status == SSHHealthStatus.RECONNECTING:
            return _("🔄 Reconectando...")
        return _("⚪ Verificando...")

    def get_css_class(self) -> str:
        """Returns CSS class name for styling the badge."""
        if self.is_auto_reconnecting:
            return "latency-reconnecting"
        if self.status == SSHHealthStatus.HEALTHY:
            return "latency-healthy"
        if self.status == SSHHealthStatus.DEGRADED:
            return "latency-degraded"
        if self.status == SSHHealthStatus.POOR:
            return "latency-poor"
        if self.status == SSHHealthStatus.UNREACHABLE:
            return "latency-unreachable"
        return "latency-unknown"

    def get_tooltip_text(self) -> str:
        """Generates a detailed status description for tooltips."""
        parts = [
            f"<b>{_('Sessão SSH:')}</b> {self.session_name}",
            f"<b>{_('Destino:')}</b> {self.host}:{self.port}",
        ]
        if self.rtt_ms is not None and self.status in (
            SSHHealthStatus.HEALTHY,
            SSHHealthStatus.DEGRADED,
            SSHHealthStatus.POOR,
        ):
            parts.append(f"<b>{_('Latência de Rede (RTT):')}</b> {self.rtt_ms:.1f} ms")
        if self.is_auto_reconnecting:
            parts.append(
                f"<b>{_('Auto-Reconexão:')}</b> {_('Tentativa')} {self.reconnect_attempt}/{self.max_reconnect_attempts} "
                f"({self.countdown_seconds}s)"
            )
        elif self.error_message:
            parts.append(f"<b>{_('Status:')}</b> <span color='#e05555'>{self.error_message}</span>")
        else:
            status_names = {
                SSHHealthStatus.HEALTHY: _("Conexão Estável"),
                SSHHealthStatus.DEGRADED: _("Latência Moderada"),
                SSHHealthStatus.POOR: _("Alta Latência"),
                SSHHealthStatus.UNREACHABLE: _("Conexão Perdida"),
                SSHHealthStatus.UNKNOWN: _("Aguardando verificação"),
            }
            parts.append(f"<b>{_('Status:')}</b> {status_names.get(self.status, _('Desconhecido'))}")
        return "\n".join(parts)


class SSHHealthMonitor:
    """
    Central monitor for SSH connection health and latency measurement.

    Maintains active SSH sessions, performs non-blocking periodic TCP probes,
    notifies listeners via AppSignals, and manages auto-reconnect lifecycles.
    """

    _instance: Optional[SSHHealthMonitor] = None

    def __init__(self, settings_manager: Optional[SettingsManager] = None) -> None:
        self.settings_manager = settings_manager
        self.logger = get_logger("onyxsh.terminal.ssh_health_monitor")
        self._records: Dict[int, SSHHealthRecord] = {}
        self._lock = threading.Lock()
        self._timer_id: Optional[int] = None
        self._countdown_timers: Dict[int, int] = {}
        self._reconnect_callbacks: Dict[int, Callable[[int], None]] = {}

        # Default settings if settings_manager is unavailable
        self.enabled = True
        self.check_interval = 10
        self.auto_reconnect_enabled = True
        self.max_reconnect_attempts = 5
        self.auto_reconnect_delay = 5

        self._load_settings()
        if self.settings_manager:
            try:
                self.settings_manager.add_change_listener(self._on_setting_changed)
            except Exception:
                pass
        self._start_periodic_check()

    @classmethod
    def get_instance(
        cls, settings_manager: Optional[SettingsManager] = None
    ) -> SSHHealthMonitor:
        """Singleton accessor."""
        if cls._instance is None:
            cls._instance = cls(settings_manager)
        elif settings_manager and not cls._instance.settings_manager:
            cls._instance.settings_manager = settings_manager
            try:
                cls._instance.settings_manager.add_change_listener(
                    cls._instance._on_setting_changed
                )
            except Exception:
                pass
            cls._instance._load_settings()
            cls._instance._start_periodic_check()
        return cls._instance

    @classmethod
    def reset_instance(cls) -> None:
        """Resets singleton (used in test suites)."""
        if cls._instance:
            cls._instance.stop()
        cls._instance = None

    def _load_settings(self) -> None:
        """Loads configuration from SettingsManager."""
        if not self.settings_manager:
            return
        self.enabled = bool(
            self.settings_manager.get("ssh_health_check_enabled", True)
        )
        self.check_interval = max(
            3, int(self.settings_manager.get("ssh_health_check_interval", 10))
        )
        self.auto_reconnect_enabled = bool(
            self.settings_manager.get("ssh_auto_reconnect_enabled", True)
        )
        self.max_reconnect_attempts = max(
            1, int(self.settings_manager.get("ssh_auto_reconnect_attempts", 5))
        )
        self.auto_reconnect_delay = max(
            1, int(self.settings_manager.get("ssh_auto_reconnect_delay", 5))
        )

    def _on_setting_changed(self, key: str, old_value: Any, new_value: Any) -> None:
        """Handles dynamic settings adjustments at runtime."""
        if key in (
            "ssh_health_check_enabled",
            "ssh_health_check_interval",
            "ssh_auto_reconnect_enabled",
            "ssh_auto_reconnect_attempts",
            "ssh_auto_reconnect_delay",
        ):
            self.logger.debug(
                f"SSHHealthMonitor setting '{key}' changed: {old_value} -> {new_value}"
            )
            self._load_settings()
            self._start_periodic_check()

    def _start_periodic_check(self) -> None:
        """Starts the periodic GLib timer."""
        if self._timer_id is not None:
            try:
                GLib.source_remove(self._timer_id)
            except Exception:
                pass
            self._timer_id = None

        if self.enabled:
            self._timer_id = GLib.timeout_add_seconds(
                self.check_interval, self._on_periodic_tick
            )

    def stop(self) -> None:
        """Stops all timers, background checks, and removes change listeners."""
        if self.settings_manager:
            try:
                self.settings_manager.remove_change_listener(self._on_setting_changed)
            except Exception:
                pass

        if self._timer_id is not None:
            try:
                GLib.source_remove(self._timer_id)
            except Exception:
                pass
            self._timer_id = None

        for tid, cid in list(self._countdown_timers.items()):
            try:
                GLib.source_remove(cid)
            except Exception:
                pass
        self._countdown_timers.clear()

    def register_terminal(
        self,
        terminal_id: int,
        session_item: SessionItem,
        reconnect_callback: Optional[Callable[[int], None]] = None,
    ) -> SSHHealthRecord:
        """
        Registers an active SSH terminal for health and latency monitoring.
        """
        host = getattr(session_item, "host", "") or "localhost"
        port = getattr(session_item, "port", 22) or 22
        session_name = getattr(session_item, "name", str(terminal_id))

        record = SSHHealthRecord(
            terminal_id=terminal_id,
            session_name=session_name,
            host=host,
            port=port,
            session_item=session_item,
            status=SSHHealthStatus.UNKNOWN,
            max_reconnect_attempts=self.max_reconnect_attempts,
        )

        with self._lock:
            self._records[terminal_id] = record
            if reconnect_callback:
                self._reconnect_callbacks[terminal_id] = reconnect_callback

        self.logger.info(
            f"Registered SSH terminal {terminal_id} ({session_name} -> {host}:{port}) for health checks."
        )

        # Trigger immediate initial probe
        self.probe_terminal_async(terminal_id)
        return record

    def unregister_terminal(self, terminal_id: int) -> None:
        """Unregisters an SSH terminal when closed."""
        self.cancel_auto_reconnect(terminal_id)
        with self._lock:
            self._records.pop(terminal_id, None)
            self._reconnect_callbacks.pop(terminal_id, None)
        self.logger.debug(f"Unregistered terminal {terminal_id} from SSH health monitor.")

    def get_record(self, terminal_id: int) -> Optional[SSHHealthRecord]:
        """Gets health record for a specific terminal."""
        with self._lock:
            return self._records.get(terminal_id)

    def get_record_by_session_name(self, session_name: str) -> Optional[SSHHealthRecord]:
        """Finds first active health record matching session name."""
        with self._lock:
            for record in self._records.values():
                if record.session_name == session_name:
                    return record
        return None

    def get_all_records(self) -> List[SSHHealthRecord]:
        """Returns copy of all active records."""
        with self._lock:
            return list(self._records.values())

    @staticmethod
    def probe_host(
        host: str, port: int = 22, timeout: float = 2.0
    ) -> Tuple[bool, Optional[float], Optional[str]]:
        """
        Measures TCP Handshake Round-Trip Time (RTT) in milliseconds.

        Returns:
            (success: bool, rtt_ms: Optional[float], error_message: Optional[str])
        """
        if not host:
            return False, None, _("Host não configurado")

        start_time = time.perf_counter()
        try:
            # Non-blocking TCP connection handshake
            sock = socket.create_connection((host, port), timeout=timeout)
            rtt_ms = round((time.perf_counter() - start_time) * 1000.0, 1)
            sock.close()
            return True, rtt_ms, None
        except socket.timeout:
            return False, None, _("Tempo limite de conexão esgotado (timeout)")
        except ConnectionRefusedError:
            return False, None, _("Conexão recusada na porta %d") % port
        except OSError as e:
            return False, None, str(e)
        except Exception as e:
            return False, None, str(e)

    def probe_host_async(
        self,
        host: str,
        port: int = 22,
        timeout: float = 2.0,
        callback: Optional[Callable[[bool, Optional[float], Optional[str]], None]] = None,
    ) -> None:
        """Executes a host probe off the main GTK thread."""
        def _worker():
            success, rtt, err = self.probe_host(host, port, timeout)
            if callback:
                GLib.idle_add(callback, success, rtt, err)

        threading.Thread(target=_worker, name="OnyxSH-SSHProbe", daemon=True).start()

    def probe_terminal_async(self, terminal_id: int) -> None:
        """Asynchronously probes host for a registered terminal."""
        record = self.get_record(terminal_id)
        if not record or record.is_auto_reconnecting:
            return

        def _on_probe_result(success: bool, rtt: Optional[float], err: Optional[str]) -> bool:
            self._update_record_probe_result(terminal_id, success, rtt, err)
            return False

        self.probe_host_async(record.host, record.port, timeout=2.5, callback=_on_probe_result)

    def _update_record_probe_result(
        self, terminal_id: int, success: bool, rtt: Optional[float], err: Optional[str]
    ) -> None:
        """Updates record state from probe result and notifies event bus."""
        record = self.get_record(terminal_id)
        if not record:
            return

        record.last_checked = time.time()
        if success and rtt is not None:
            record.rtt_ms = rtt
            record.consecutive_failures = 0
            record.error_message = None

            if rtt < 150.0:
                record.status = SSHHealthStatus.HEALTHY
            elif rtt < 350.0:
                record.status = SSHHealthStatus.DEGRADED
            else:
                record.status = SSHHealthStatus.POOR
        else:
            record.consecutive_failures += 1
            record.error_message = err or _("Falha de conexão")
            # Require at least 2 consecutive failures before marking unreachable to avoid transient blips
            if record.consecutive_failures >= 2:
                record.status = SSHHealthStatus.UNREACHABLE

        # Emit global signal
        AppSignals.get().emit("ssh-health-updated", record)

    def _on_periodic_tick(self) -> bool:
        """Periodic timer callback to probe all active SSH terminals."""
        if not self.enabled:
            return True

        records = self.get_all_records()
        for record in records:
            if not record.is_auto_reconnecting:
                self.probe_terminal_async(record.terminal_id)

        return True

    def notify_connection_lost(
        self, terminal_id: int, reason: str = ""
    ) -> SSHHealthRecord:
        """
        Called when a terminal process or socket unexpectedly dies.
        Transitions state to UNREACHABLE and begins auto-reconnect if enabled.
        """
        record = self.get_record(terminal_id)
        if not record:
            # Create a placeholder record if not previously registered
            record = SSHHealthRecord(
                terminal_id=terminal_id,
                session_name=str(terminal_id),
                host="",
                status=SSHHealthStatus.UNREACHABLE,
                error_message=reason or _("Conexão SSH interrompida"),
            )
            with self._lock:
                self._records[terminal_id] = record

        record.status = SSHHealthStatus.UNREACHABLE
        record.error_message = reason or _("Conexão SSH interrompida")
        record.consecutive_failures += 1

        AppSignals.get().emit("ssh-connection-lost", terminal_id, record.session_item, reason)
        AppSignals.get().emit("ssh-health-updated", record)

        if self.auto_reconnect_enabled and record.reconnect_attempt < self.max_reconnect_attempts:
            self._start_auto_reconnect_countdown(terminal_id)

        return record

    def _start_auto_reconnect_countdown(self, terminal_id: int) -> None:
        """Begins an interactive 5-second countdown to auto-reconnect."""
        record = self.get_record(terminal_id)
        if not record:
            return

        self.cancel_auto_reconnect(terminal_id)

        record.is_auto_reconnecting = True
        record.reconnect_attempt += 1
        record.countdown_seconds = self.auto_reconnect_delay
        record.status = SSHHealthStatus.RECONNECTING

        AppSignals.get().emit("ssh-health-updated", record)

        def _countdown_step() -> bool:
            rec = self.get_record(terminal_id)
            if not rec or not rec.is_auto_reconnecting:
                return False

            rec.countdown_seconds -= 1
            AppSignals.get().emit("ssh-health-updated", rec)

            if rec.countdown_seconds <= 0:
                self._countdown_timers.pop(terminal_id, None)
                self.trigger_reconnect_attempt(terminal_id)
                return False

            return True

        timer_id = GLib.timeout_add_seconds(1, _countdown_step)
        self._countdown_timers[terminal_id] = timer_id

    def cancel_auto_reconnect(self, terminal_id: int) -> None:
        """Cancels any active auto-reconnect countdown for a terminal."""
        if terminal_id in self._countdown_timers:
            try:
                GLib.source_remove(self._countdown_timers.pop(terminal_id))
            except Exception:
                pass

        record = self.get_record(terminal_id)
        if record:
            record.is_auto_reconnecting = False
            record.countdown_seconds = 0
            if record.status == SSHHealthStatus.RECONNECTING:
                record.status = SSHHealthStatus.UNREACHABLE
            AppSignals.get().emit("ssh-health-updated", record)

    def trigger_reconnect_attempt(self, terminal_id: int) -> None:
        """Fires the reconnect callback for a terminal."""
        self.cancel_auto_reconnect(terminal_id)
        record = self.get_record(terminal_id)
        if record:
            record.status = SSHHealthStatus.RECONNECTING
            AppSignals.get().emit("ssh-health-updated", record)

        callback = self._reconnect_callbacks.get(terminal_id)
        if callback:
            self.logger.info(
                f"Triggering SSH reconnect attempt for terminal {terminal_id} (Attempt {record.reconnect_attempt if record else 1})"
            )
            try:
                callback(terminal_id)
            except Exception as e:
                self.logger.error(f"Error during SSH reconnect callback: {e}")
        else:
            self.logger.warning(
                f"No reconnect callback registered for terminal {terminal_id}"
            )

    def notify_connection_restored(self, terminal_id: int) -> None:
        """Clears reconnect state when SSH reconnects successfully."""
        self.cancel_auto_reconnect(terminal_id)
        record = self.get_record(terminal_id)
        if record:
            record.is_auto_reconnecting = False
            record.reconnect_attempt = 0
            record.consecutive_failures = 0
            record.error_message = None
            record.status = SSHHealthStatus.HEALTHY
            AppSignals.get().emit("ssh-health-updated", record)
            # Re-probe immediately
            self.probe_terminal_async(terminal_id)


def get_ssh_health_monitor(
    settings_manager: Optional[SettingsManager] = None,
) -> SSHHealthMonitor:
    """Helper to retrieve the global SSHHealthMonitor instance."""
    return SSHHealthMonitor.get_instance(settings_manager)
