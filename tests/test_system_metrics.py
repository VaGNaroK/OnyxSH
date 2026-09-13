# tests/test_system_metrics.py

import subprocess
import time
import unittest
from unittest.mock import MagicMock, patch

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import GLib, Gtk, Adw

from onyxsh.system.metrics import (
    CPUData,
    DiskData,
    MemoryData,
    MetricsSnapshot,
    MetricsWorkerThread,
    NetworkData,
    SystemMetricsCollector,
    format_bytes,
    format_rate,
)
from onyxsh.ui.widgets.sparkline import SparklineCanvas
from onyxsh.ui.dialogs.resource_dashboard_dialog import ResourceDashboardDialog


class TestSystemMetrics(unittest.TestCase):
    """Test suite for system resource metrics collection, models, and worker lifecycle."""

    def test_format_bytes(self):
        """Test human-readable byte unit formatting."""
        self.assertEqual(format_bytes(0), "0.0 B")
        self.assertEqual(format_bytes(1024), "1.0 KB")
        self.assertEqual(format_bytes(1024 * 1024 * 5), "5.0 MB")
        self.assertEqual(format_bytes(1024 * 1024 * 1024 * 2.5), "2.5 GB")
        self.assertEqual(format_bytes(-10), "0.0 B")

    def test_format_rate(self):
        """Test byte transfer speed rate formatting."""
        self.assertEqual(format_rate(1024 * 1024), "1.0 MB/s")
        self.assertEqual(format_rate(512), "512.0 B/s")

    def test_collect_local_snapshot(self):
        """Test local metrics snapshot collection produces valid metrics."""
        collector = SystemMetricsCollector()
        snapshot = collector.collect_local_snapshot()

        self.assertIsInstance(snapshot, MetricsSnapshot)
        self.assertEqual(snapshot.host_name, "Local")
        self.assertFalse(snapshot.is_remote)
        self.assertTrue(snapshot.is_connected)
        self.assertGreater(snapshot.timestamp, 0.0)

        # CPU
        self.assertIsNotNone(snapshot.cpu)
        self.assertGreaterEqual(snapshot.cpu.usage_percent, 0.0)
        self.assertLessEqual(snapshot.cpu.usage_percent, 100.0)
        self.assertGreaterEqual(snapshot.cpu.core_count, 1)
        self.assertEqual(len(snapshot.cpu.load_avg), 3)

        # Memory
        self.assertIsNotNone(snapshot.memory)
        self.assertGreater(snapshot.memory.total_bytes, 0)
        self.assertGreaterEqual(snapshot.memory.used_percent, 0.0)
        self.assertLessEqual(snapshot.memory.used_percent, 100.0)

        # Disk
        self.assertIsNotNone(snapshot.disk)
        self.assertEqual(snapshot.disk.mount_point, "/")
        self.assertGreater(snapshot.disk.total_bytes, 0)

        # Network
        self.assertIsNotNone(snapshot.network)
        self.assertGreaterEqual(snapshot.network.bytes_recv_rate, 0.0)
        self.assertGreaterEqual(snapshot.network.bytes_sent_rate, 0.0)

    def test_collect_local_memory_proc_fallback(self):
        """Test pure procfs memory fallback parser."""
        mem = SystemMetricsCollector._collect_local_memory_proc()
        self.assertIsInstance(mem, MemoryData)
        self.assertGreater(mem.total_bytes, 0)
        self.assertGreaterEqual(mem.used_percent, 0.0)

    def test_collect_local_disk_statvfs_fallback(self):
        """Test pure POSIX statvfs disk fallback parser."""
        disk = SystemMetricsCollector._collect_local_disk_statvfs()
        self.assertIsInstance(disk, DiskData)
        self.assertEqual(disk.mount_point, "/")
        self.assertGreater(disk.total_bytes, 0)

    def test_parse_remote_output(self):
        """Test parsing composite remote SSH response with section markers."""
        collector = SystemMetricsCollector()
        fake_remote_raw = (
            "cpu  1000 200 300 5000 100 10 20 0\n"
            "===SECTION===\n"
            "MemTotal:        8192000 kB\n"
            "MemAvailable:    4096000 kB\n"
            "SwapTotal:       2048000 kB\n"
            "SwapFree:        1024000 kB\n"
            "===SECTION===\n"
            "/dev/sda1        40960000 20480000 20480000  50% /\n"
            "===SECTION===\n"
            "eth0: 10000000 1000 0 0 0 0 0 0 5000000 500 0 0 0 0 0 0\n"
            "===SECTION===\n"
            "4\n"
            "===SECTION===\n"
            " 12:00:00 up 10 days,  2:30,  1 user,  load average: 0.15, 0.25, 0.35\n"
        )

        snapshot = collector._parse_remote_output(fake_remote_raw, "remote-host-01")
        self.assertTrue(snapshot.is_remote)
        self.assertTrue(snapshot.is_connected)
        self.assertEqual(snapshot.host_name, "remote-host-01")

        # CPU
        self.assertEqual(snapshot.cpu.core_count, 4)
        self.assertEqual(snapshot.cpu.load_avg, (0.15, 0.25, 0.35))

        # Memory (8192000 kB total, 4096000 kB avail -> 50% used)
        self.assertEqual(snapshot.memory.total_bytes, 8192000 * 1024)
        self.assertEqual(snapshot.memory.used_percent, 50.0)
        self.assertEqual(snapshot.memory.swap_percent, 50.0)

        # Disk (50% used)
        self.assertEqual(snapshot.disk.used_percent, 50.0)

    def test_collect_remote_snapshot_timeout(self):
        """Test that SSH timeout results in safe disconnected snapshot."""
        collector = SystemMetricsCollector()
        mock_session = MagicMock()
        mock_session.host = "192.0.2.1"
        mock_session.name = "TimeoutServer"

        with patch("subprocess.run", side_effect=subprocess.TimeoutExpired(cmd="ssh", timeout=2.0)):
            snapshot = collector.collect_remote_snapshot(mock_session)
            self.assertFalse(snapshot.is_connected)
            self.assertTrue(snapshot.is_remote)
            self.assertIn("timeout", snapshot.error_message.lower())

    def test_metrics_worker_thread_lifecycle(self):
        """Test MetricsWorkerThread start, pause, resume, and stop controls."""
        snapshots = []

        def on_snapshot(s):
            snapshots.append(s)

        worker = MetricsWorkerThread(callback=on_snapshot, interval=0.1)
        self.assertTrue(worker.is_paused)

        worker.start()
        # Resume active sampling
        worker.resume()
        self.assertFalse(worker.is_paused)

        # Wait briefly for snapshot
        time.sleep(0.3)
        ctx = GLib.main_context_default()
        while ctx.iteration(False):
            pass
        self.assertGreater(len(snapshots), 0)

        # Pause worker (zero CPU mode)
        worker.pause()
        self.assertTrue(worker.is_paused)
        count_before = len(snapshots)

        time.sleep(0.2)
        while ctx.iteration(False):
            pass
        # Should not accumulate further snapshots while paused
        self.assertEqual(len(snapshots), count_before)

        # Stop worker
        worker.stop()
        worker.join(timeout=1.0)
        self.assertFalse(worker.is_alive())

    def test_sparkline_canvas_bounded_buffer(self):
        """Test SparklineCanvas circular buffer never exceeds max_points."""
        sparkline = SparklineCanvas(max_points=5, height_request=30)
        self.assertEqual(len(sparkline._primary_data), 0)

        # Push 10 points
        for i in range(10):
            sparkline.push_value(float(i))

        # Must be bounded by max_points (5)
        self.assertEqual(len(sparkline._primary_data), 5)
        self.assertEqual(list(sparkline._primary_data), [5.0, 6.0, 7.0, 8.0, 9.0])

        sparkline.clear()
        self.assertEqual(len(sparkline._primary_data), 0)

    def test_resource_dashboard_dialog_singleton(self):
        """Test ResourceDashboardDialog singleton creation and cleanup."""
        mock_window = Gtk.Window()
        mock_window.terminal_manager = MagicMock()
        mock_window.terminal_manager.registry.get_all_terminal_ids.return_value = []

        dialog1 = ResourceDashboardDialog.show_dashboard(parent_window=mock_window)
        self.assertIsInstance(dialog1, ResourceDashboardDialog)

        # Calling again should return the same singleton
        dialog2 = ResourceDashboardDialog.show_dashboard(parent_window=mock_window)
        self.assertIs(dialog1, dialog2)

        # Close request should cleanup instance
        dialog1._on_close_request(dialog1)
        self.assertIsNone(ResourceDashboardDialog._instance)


if __name__ == "__main__":
    unittest.main()
