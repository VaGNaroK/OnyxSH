# onyxsh/system/metrics.py
"""
High-performance, non-blocking system metrics collection engine for OnyxSH.

Collects CPU, RAM, Disk, and Network I/O metrics for local machine and remote SSH sessions
without impacting terminal responsiveness, UI framerate, or memory consumption.
"""

import os
import re
import shlex
import subprocess
import threading
import time
from dataclasses import dataclass
from typing import Any, Callable, Optional, Tuple

import gi
from gi.repository import GLib

from ..utils.logger import get_logger

logger = get_logger("onyxsh.system.metrics")

# Optional psutil import with pure /proc and POSIX fallback
try:
    import psutil
    _HAS_PSUTIL = True
except ImportError:
    psutil = None
    _HAS_PSUTIL = False


def format_bytes(num_bytes: float, suffix: str = "B") -> str:
    """Formats bytes into human-readable strings (e.g., 4.2 GB, 512 MB)."""
    if num_bytes < 0:
        return f"0.0 {suffix}"
    for unit in ["", "K", "M", "G", "T", "P"]:
        if abs(num_bytes) < 1024.0:
            return f"{num_bytes:3.1f} {unit}{suffix}".strip()
        num_bytes /= 1024.0
    return f"{num_bytes:.1f} E{suffix}"


def format_rate(bytes_per_sec: float) -> str:
    """Formats byte rate into transfer speed (e.g., 1.4 MB/s, 240 KB/s)."""
    return f"{format_bytes(bytes_per_sec)}/s"


@dataclass(slots=True)
class CPUData:
    """Snapshot of processor utilization and load."""
    usage_percent: float
    core_count: int
    load_avg: Tuple[float, float, float]


@dataclass(slots=True)
class MemoryData:
    """Snapshot of physical RAM and swap utilization."""
    total_bytes: int
    used_bytes: int
    free_bytes: int
    available_bytes: int
    used_percent: float
    swap_total_bytes: int
    swap_used_bytes: int
    swap_percent: float


@dataclass(slots=True)
class DiskData:
    """Snapshot of filesystem storage capacity."""
    mount_point: str
    total_bytes: int
    used_bytes: int
    free_bytes: int
    used_percent: float


@dataclass(slots=True)
class NetworkData:
    """Snapshot of network I/O throughput rates."""
    bytes_recv_rate: float  # Download (bytes/sec)
    bytes_sent_rate: float  # Upload (bytes/sec)
    total_recv_bytes: int
    total_sent_bytes: int


@dataclass(slots=True)
class MetricsSnapshot:
    """Consolidated immutable snapshot of host health metrics."""
    timestamp: float
    host_name: str
    is_remote: bool
    is_connected: bool
    error_message: Optional[str] = None
    cpu: Optional[CPUData] = None
    memory: Optional[MemoryData] = None
    disk: Optional[DiskData] = None
    network: Optional[NetworkData] = None


class SystemMetricsCollector:
    """
    Collects system metrics locally using psutil (with Linux /proc fallback)
    and remotely over SSH using lightweight, non-interactive one-liners.
    """

    def __init__(self) -> None:
        self._last_time: float = time.time()
        self._last_cpu_total: int = 0
        self._last_cpu_idle: int = 0
        self._last_net_recv: int = 0
        self._last_net_sent: int = 0
        self._remote_last_time: float = 0.0
        self._remote_last_cpu_total: int = 0
        self._remote_last_cpu_idle: int = 0
        self._remote_last_net_recv: int = 0
        self._remote_last_net_sent: int = 0

        # Initialize baseline counters
        self._init_local_counters()

    def _init_local_counters(self) -> None:
        """Initializes baseline CPU and network counters for accurate rate differentials."""
        now = time.time()
        self._last_time = now

        if _HAS_PSUTIL and psutil:
            try:
                psutil.cpu_percent(interval=None)
                net = psutil.net_io_counters()
                if net:
                    self._last_net_recv = net.bytes_recv
                    self._last_net_sent = net.bytes_sent
                return
            except Exception as e:
                logger.debug(f"psutil initialization fallback: {e}")

        # Fallback reading /proc
        try:
            with open("/proc/stat", "r", encoding="utf-8") as f:
                fields = [int(x) for x in f.readline().split()[1:8]]
                self._last_cpu_idle = fields[3] + (fields[4] if len(fields) > 4 else 0)
                self._last_cpu_total = sum(fields)
        except Exception:
            pass

        try:
            rx, tx = self._read_proc_net_bytes()
            self._last_net_recv = rx
            self._last_net_sent = tx
        except Exception:
            pass

    @staticmethod
    def _read_proc_net_bytes() -> Tuple[int, int]:
        """Sums bytes_recv and bytes_sent across all non-loopback interfaces in /proc/net/dev."""
        total_rx = 0
        total_tx = 0
        with open("/proc/net/dev", "r", encoding="utf-8") as f:
            for line in f:
                if ":" not in line:
                    continue
                iface, data = line.split(":", 1)
                if iface.strip() == "lo":
                    continue
                cols = data.split()
                if len(cols) >= 9:
                    total_rx += int(cols[0])
                    total_tx += int(cols[8])
        return total_rx, total_tx

    def collect_local_snapshot(self) -> MetricsSnapshot:
        """Collects a complete metrics snapshot of the local machine."""
        now = time.time()
        delta_t = max(0.001, now - self._last_time)
        self._last_time = now

        core_count = os.cpu_count() or 1
        try:
            load_avg = os.getloadavg()
        except (AttributeError, OSError):
            load_avg = (0.0, 0.0, 0.0)

        cpu_percent = 0.0
        mem_data: Optional[MemoryData] = None
        disk_data: Optional[DiskData] = None
        net_data: Optional[NetworkData] = None

        if _HAS_PSUTIL and psutil:
            try:
                # 1. CPU
                cpu_percent = float(psutil.cpu_percent(interval=None))

                # 2. Memory
                vm = psutil.virtual_memory()
                sm = psutil.swap_memory()
                mem_data = MemoryData(
                    total_bytes=vm.total,
                    used_bytes=vm.used,
                    free_bytes=vm.free,
                    available_bytes=vm.available,
                    used_percent=float(vm.percent),
                    swap_total_bytes=sm.total,
                    swap_used_bytes=sm.used,
                    swap_percent=float(sm.percent),
                )

                # 3. Disk
                du = psutil.disk_usage("/")
                disk_data = DiskData(
                    mount_point="/",
                    total_bytes=du.total,
                    used_bytes=du.used,
                    free_bytes=du.free,
                    used_percent=float(du.percent),
                )

                # 4. Network
                net = psutil.net_io_counters()
                if net:
                    rx_rate = max(0.0, (net.bytes_recv - self._last_net_recv) / delta_t)
                    tx_rate = max(0.0, (net.bytes_sent - self._last_net_sent) / delta_t)
                    self._last_net_recv = net.bytes_recv
                    self._last_net_sent = net.bytes_sent
                    net_data = NetworkData(
                        bytes_recv_rate=rx_rate,
                        bytes_sent_rate=tx_rate,
                        total_recv_bytes=net.bytes_recv,
                        total_sent_bytes=net.bytes_sent,
                    )
            except Exception as e:
                logger.debug(f"psutil local metrics query failed, falling back: {e}")

        # Fallback to pure /proc and POSIX calls if any section missing
        if mem_data is None:
            mem_data = self._collect_local_memory_proc()

        if disk_data is None:
            disk_data = self._collect_local_disk_statvfs()

        if net_data is None:
            try:
                rx, tx = self._read_proc_net_bytes()
                rx_rate = max(0.0, (rx - self._last_net_recv) / delta_t)
                tx_rate = max(0.0, (tx - self._last_net_sent) / delta_t)
                self._last_net_recv = rx
                self._last_net_sent = tx
                net_data = NetworkData(
                    bytes_recv_rate=rx_rate,
                    bytes_sent_rate=tx_rate,
                    total_recv_bytes=rx,
                    total_sent_bytes=tx,
                )
            except Exception:
                net_data = NetworkData(0.0, 0.0, 0, 0)

        if cpu_percent == 0.0:
            try:
                with open("/proc/stat", "r", encoding="utf-8") as f:
                    fields = [int(x) for x in f.readline().split()[1:8]]
                    idle = fields[3] + (fields[4] if len(fields) > 4 else 0)
                    total = sum(fields)
                    d_total = max(1, total - self._last_cpu_total)
                    d_idle = idle - self._last_cpu_idle
                    cpu_percent = max(0.0, min(100.0, 100.0 * (1.0 - (d_idle / d_total))))
                    self._last_cpu_total = total
                    self._last_cpu_idle = idle
            except Exception:
                pass

        cpu_data = CPUData(
            usage_percent=round(cpu_percent, 1),
            core_count=core_count,
            load_avg=load_avg,
        )

        return MetricsSnapshot(
            timestamp=now,
            host_name="Local",
            is_remote=False,
            is_connected=True,
            cpu=cpu_data,
            memory=mem_data,
            disk=disk_data,
            network=net_data,
        )

    @staticmethod
    def _collect_local_memory_proc() -> MemoryData:
        """Parses /proc/meminfo for memory and swap stats."""
        info = {}
        try:
            with open("/proc/meminfo", "r", encoding="utf-8") as f:
                for line in f:
                    parts = line.split(":")
                    if len(parts) == 2:
                        key = parts[0].strip()
                        val = parts[1].strip().split()[0]
                        info[key] = int(val) * 1024
        except Exception:
            pass

        total = info.get("MemTotal", 1024 * 1024 * 1024)
        available = info.get("MemAvailable", info.get("MemFree", 0))
        used = max(0, total - available)
        used_pct = round((used / total) * 100.0, 1) if total > 0 else 0.0

        swap_total = info.get("SwapTotal", 0)
        swap_free = info.get("SwapFree", 0)
        swap_used = max(0, swap_total - swap_free)
        swap_pct = round((swap_used / swap_total) * 100.0, 1) if swap_total > 0 else 0.0

        return MemoryData(
            total_bytes=total,
            used_bytes=used,
            free_bytes=info.get("MemFree", 0),
            available_bytes=available,
            used_percent=used_pct,
            swap_total_bytes=swap_total,
            swap_used_bytes=swap_used,
            swap_percent=swap_pct,
        )

    @staticmethod
    def _collect_local_disk_statvfs() -> DiskData:
        """Parses root partition capacity using os.statvfs."""
        try:
            st = os.statvfs("/")
            total = st.f_blocks * st.f_frsize
            free = st.f_bavail * st.f_frsize
            used = max(0, total - free)
            used_pct = round((used / total) * 100.0, 1) if total > 0 else 0.0
            return DiskData("/", total, used, free, used_pct)
        except Exception:
            return DiskData("/", 0, 0, 0, 0.0)

    def collect_remote_snapshot(self, session: Any) -> MetricsSnapshot:
        """
        Executes a lightweight, non-interactive SSH query with a strict 2s timeout.
        Extracts CPU, memory, disk, and network stats without touching user terminal PTY.
        """
        now = time.time()
        host = getattr(session, "host", "") or getattr(session, "name", "Remote")
        port = str(getattr(session, "port", 22) or 22)
        user = getattr(session, "username", "") or getattr(session, "user", "")
        key_path = getattr(session, "key_path", "") or getattr(session, "private_key", "")

        target = f"{user}@{host}" if user else host

        ssh_cmd = [
            "ssh",
            "-o", "BatchMode=yes",
            "-o", "ConnectTimeout=2",
            "-o", "StrictHostKeyChecking=accept-new",
            "-p", port,
        ]
        if key_path and os.path.exists(os.path.expanduser(key_path)):
            ssh_cmd.extend(["-i", os.path.expanduser(key_path)])

        # Single compact payload
        remote_script = (
            "cat /proc/stat 2>/dev/null | head -n 1; echo '===SECTION==='; "
            "cat /proc/meminfo 2>/dev/null | grep -E '^(MemTotal|MemAvailable|SwapTotal|SwapFree):'; echo '===SECTION==='; "
            "df -k / 2>/dev/null | tail -n 1; echo '===SECTION==='; "
            "cat /proc/net/dev 2>/dev/null; echo '===SECTION==='; "
            "nproc 2>/dev/null || grep -c ^processor /proc/cpuinfo 2>/dev/null || echo 1; echo '===SECTION==='; "
            "uptime 2>/dev/null"
        )
        ssh_cmd.extend([target, remote_script])

        try:
            proc = subprocess.run(
                ssh_cmd,
                capture_output=True,
                text=True,
                timeout=2.5,
                check=False,
            )
            if proc.returncode != 0:
                err = proc.stderr.strip() or f"SSH error (code {proc.returncode})"
                return MetricsSnapshot(
                    timestamp=now,
                    host_name=getattr(session, "name", host),
                    is_remote=True,
                    is_connected=False,
                    error_message=err,
                )

            return self._parse_remote_output(proc.stdout, getattr(session, "name", host))

        except subprocess.TimeoutExpired:
            return MetricsSnapshot(
                timestamp=now,
                host_name=getattr(session, "name", host),
                is_remote=True,
                is_connected=False,
                error_message="Tempo limite esgotado (timeout 2s)",
            )
        except Exception as e:
            return MetricsSnapshot(
                timestamp=now,
                host_name=getattr(session, "name", host),
                is_remote=True,
                is_connected=False,
                error_message=str(e),
            )

    def _parse_remote_output(self, raw_output: str, session_name: str) -> MetricsSnapshot:
        """Parses output from the composite remote SSH query."""
        now = time.time()
        delta_t = max(0.001, now - self._remote_last_time) if self._remote_last_time > 0 else 1.0
        self._remote_last_time = now

        sections = raw_output.split("===SECTION===")

        # 1. CPU
        cpu_pct = 0.0
        if len(sections) > 0 and sections[0].strip():
            try:
                line = sections[0].strip()
                fields = [int(x) for x in line.split()[1:8]]
                idle = fields[3] + (fields[4] if len(fields) > 4 else 0)
                total = sum(fields)
                if self._remote_last_cpu_total > 0:
                    d_total = max(1, total - self._remote_last_cpu_total)
                    d_idle = idle - self._remote_last_cpu_idle
                    cpu_pct = max(0.0, min(100.0, 100.0 * (1.0 - (d_idle / d_total))))
                self._remote_last_cpu_total = total
                self._remote_last_cpu_idle = idle
            except Exception:
                pass

        # Cores
        core_count = 1
        if len(sections) > 4 and sections[4].strip():
            try:
                core_count = int(sections[4].strip().split()[0])
            except Exception:
                core_count = 1

        # Load avg from uptime
        load_avg = (0.0, 0.0, 0.0)
        if len(sections) > 5 and sections[5].strip():
            m = re.search(r"load average[s]?:\s*([0-9.]+)[,\s]+([0-9.]+)[,\s]+([0-9.]+)", sections[5])
            if m:
                load_avg = (float(m.group(1)), float(m.group(2)), float(m.group(3)))

        cpu_data = CPUData(usage_percent=round(cpu_pct, 1), core_count=core_count, load_avg=load_avg)

        # 2. Memory
        mem_info = {}
        if len(sections) > 1:
            for line in sections[1].strip().splitlines():
                if ":" in line:
                    k, v = line.split(":", 1)
                    try:
                        mem_info[k.strip()] = int(v.strip().split()[0]) * 1024
                    except Exception:
                        pass

        total_ram = mem_info.get("MemTotal", 1)
        avail_ram = mem_info.get("MemAvailable", 0)
        used_ram = max(0, total_ram - avail_ram)
        used_ram_pct = round((used_ram / total_ram) * 100.0, 1) if total_ram > 0 else 0.0

        swap_total = mem_info.get("SwapTotal", 0)
        swap_free = mem_info.get("SwapFree", 0)
        swap_used = max(0, swap_total - swap_free)
        swap_pct = round((swap_used / swap_total) * 100.0, 1) if swap_total > 0 else 0.0

        mem_data = MemoryData(
            total_bytes=total_ram,
            used_bytes=used_ram,
            free_bytes=mem_info.get("MemFree", 0),
            available_bytes=avail_ram,
            used_percent=used_ram_pct,
            swap_total_bytes=swap_total,
            swap_used_bytes=swap_used,
            swap_percent=swap_pct,
        )

        # 3. Disk
        disk_data = DiskData("/", 0, 0, 0, 0.0)
        if len(sections) > 2 and sections[2].strip():
            try:
                cols = sections[2].strip().split()
                if len(cols) >= 5:
                    d_total = int(cols[1]) * 1024
                    d_used = int(cols[2]) * 1024
                    d_avail = int(cols[3]) * 1024
                    d_pct = round((d_used / d_total) * 100.0, 1) if d_total > 0 else 0.0
                    disk_data = DiskData("/", d_total, d_used, d_avail, d_pct)
            except Exception:
                pass

        # 4. Network
        net_rx = 0
        net_tx = 0
        if len(sections) > 3 and sections[3].strip():
            for line in sections[3].strip().splitlines():
                if ":" not in line:
                    continue
                iface, data = line.split(":", 1)
                if iface.strip() == "lo":
                    continue
                cols = data.split()
                if len(cols) >= 9:
                    net_rx += int(cols[0])
                    net_tx += int(cols[8])

        rx_rate = 0.0
        tx_rate = 0.0
        if self._remote_last_net_recv > 0:
            rx_rate = max(0.0, (net_rx - self._remote_last_net_recv) / delta_t)
            tx_rate = max(0.0, (net_tx - self._remote_last_net_sent) / delta_t)
        self._remote_last_net_recv = net_rx
        self._remote_last_net_sent = net_tx

        net_data = NetworkData(
            bytes_recv_rate=rx_rate,
            bytes_sent_rate=tx_rate,
            total_recv_bytes=net_rx,
            total_sent_bytes=net_tx,
        )

        return MetricsSnapshot(
            timestamp=now,
            host_name=session_name,
            is_remote=True,
            is_connected=True,
            cpu=cpu_data,
            memory=mem_data,
            disk=disk_data,
            network=net_data,
        )


class MetricsWorkerThread(threading.Thread):
    """
    Dedicated background worker thread that queries metrics and dispatches
    snapshots to GTK 4 on the main loop. Enforces 0% CPU consumption when paused.
    """

    def __init__(
        self,
        callback: Callable[[MetricsSnapshot], None],
        interval: float = 2.0,
    ) -> None:
        super().__init__(name="OnyxSH-MetricsWorker", daemon=True)
        self._callback = callback
        self._interval: float = max(0.5, interval)
        self._collector = SystemMetricsCollector()
        self._target_session: Optional[Any] = None
        self._stop_event = threading.Event()
        self._pause_event = threading.Event()
        # Start paused until UI explicitly requests updates
        self._pause_event.clear()
        self._lock = threading.Lock()

    @property
    def is_paused(self) -> bool:
        return not self._pause_event.is_set()

    def set_interval(self, interval: float) -> None:
        """Sets sampling interval in seconds (e.g. 1.0, 2.0, 5.0)."""
        with self._lock:
            self._interval = max(0.5, float(interval))

    def set_target_session(self, session: Optional[Any]) -> None:
        """Selects target host: None for Local, or a SessionItem for remote SSH."""
        with self._lock:
            self._target_session = session

    def resume(self) -> None:
        """Resumes active periodic sampling."""
        self._pause_event.set()

    def pause(self) -> None:
        """Pauses sampling completely (entering zero CPU consumption wait state)."""
        self._pause_event.clear()

    def stop(self) -> None:
        """Permanently stops the worker thread."""
        self._stop_event.set()
        self._pause_event.set()

    def run(self) -> None:
        """Main worker loop."""
        while not self._stop_event.is_set():
            # Wait efficiently until unpaused
            self._pause_event.wait()
            if self._stop_event.is_set():
                break

            # Collect snapshot off the main UI thread
            with self._lock:
                target = self._target_session
                interval = self._interval

            try:
                if target is None:
                    snapshot = self._collector.collect_local_snapshot()
                else:
                    snapshot = self._collector.collect_remote_snapshot(target)

                # Dispatch snapshot safely to GTK main thread
                if not self._stop_event.is_set() and self._pause_event.is_set():
                    GLib.idle_add(self._callback, snapshot)
            except Exception as e:
                logger.error(f"Error in metrics worker: {e}", exc_info=True)

            # Sleep for interval or until stopped
            self._stop_event.wait(timeout=interval)
