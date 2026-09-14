# tests/test_ssh_health_monitor.py
"""
Unit tests for SSH Health Monitor & Latency (RTT) Tracker.
"""

import socket
import time
import unittest
from unittest.mock import MagicMock, patch

from onyxsh.core.signals import AppSignals
from onyxsh.sessions.models import SessionItem
from onyxsh.terminal.ssh_health_monitor import (
    SSHHealthMonitor,
    SSHHealthRecord,
    SSHHealthStatus,
    get_ssh_health_monitor,
)


class TestSSHHealthMonitor(unittest.TestCase):
    """Test suite for SSHHealthMonitor and SSHHealthRecord."""

    def setUp(self):
        SSHHealthMonitor.reset_instance()
        self.mock_settings = MagicMock()
        self.mock_settings.get.side_effect = lambda key, default=None: {
            "ssh_health_check_enabled": True,
            "ssh_health_check_interval": 10,
            "ssh_keepalive_interval": 15,
            "ssh_keepalive_count_max": 3,
            "ssh_auto_reconnect_enabled": True,
            "ssh_auto_reconnect_attempts": 5,
            "ssh_auto_reconnect_delay": 5,
        }.get(key, default)

        self.monitor = SSHHealthMonitor.get_instance(self.mock_settings)

    def tearDown(self):
        if self.monitor:
            self.monitor.stop()
        SSHHealthMonitor.reset_instance()

    def test_health_status_enums(self):
        """Verify all SSHHealthStatus values exist."""
        self.assertEqual(SSHHealthStatus.UNKNOWN.value, "unknown")
        self.assertEqual(SSHHealthStatus.HEALTHY.value, "healthy")
        self.assertEqual(SSHHealthStatus.DEGRADED.value, "degraded")
        self.assertEqual(SSHHealthStatus.POOR.value, "poor")
        self.assertEqual(SSHHealthStatus.UNREACHABLE.value, "unreachable")
        self.assertEqual(SSHHealthStatus.RECONNECTING.value, "reconnecting")

    def test_health_record_badge_labels_and_classes(self):
        """Verify badge label and CSS class generation across statuses."""
        # 1. Healthy
        rec = SSHHealthRecord(
            terminal_id=1,
            session_name="Prod-Server",
            host="192.168.1.100",
            port=22,
            status=SSHHealthStatus.HEALTHY,
            rtt_ms=45.6,
        )
        self.assertEqual(rec.get_badge_label(), "🟢 45ms")
        self.assertEqual(rec.get_css_class(), "latency-healthy")

        # 2. Degraded
        rec.status = SSHHealthStatus.DEGRADED
        rec.rtt_ms = 185.2
        self.assertEqual(rec.get_badge_label(), "🟡 185ms")
        self.assertEqual(rec.get_css_class(), "latency-degraded")

        # 3. Poor
        rec.status = SSHHealthStatus.POOR
        rec.rtt_ms = 420.0
        self.assertEqual(rec.get_badge_label(), "🟠 420ms")
        self.assertEqual(rec.get_css_class(), "latency-poor")

        # 4. Unreachable
        rec.status = SSHHealthStatus.UNREACHABLE
        self.assertIn("Inalcançável", rec.get_badge_label())
        self.assertEqual(rec.get_css_class(), "latency-unreachable")

        # 5. Reconnecting with countdown
        rec.is_auto_reconnecting = True
        rec.countdown_seconds = 4
        self.assertEqual(rec.get_badge_label(), "🔄 4s")
        self.assertEqual(rec.get_css_class(), "latency-reconnecting")

    def test_health_record_tooltip(self):
        """Verify detailed tooltip text formatting."""
        rec = SSHHealthRecord(
            terminal_id=1,
            session_name="Database-Primary",
            host="db.internal",
            port=2222,
            status=SSHHealthStatus.HEALTHY,
            rtt_ms=32.4,
        )
        tooltip = rec.get_tooltip_text()
        self.assertIn("Database-Primary", tooltip)
        self.assertIn("db.internal:2222", tooltip)
        self.assertIn("32.4 ms", tooltip)

    def test_register_and_unregister_terminal(self):
        """Verify registering and unregistering terminals."""
        session = SessionItem(
            name="Web-Server",
            session_type="ssh",
            host="10.0.0.5",
            port=2200,
        )

        record = self.monitor.register_terminal(10, session)
        self.assertEqual(record.terminal_id, 10)
        self.assertEqual(record.session_name, "Web-Server")
        self.assertEqual(record.host, "10.0.0.5")
        self.assertEqual(record.port, 2200)

        # Retrieve by terminal_id and by session_name
        self.assertIsNotNone(self.monitor.get_record(10))
        self.assertIsNotNone(self.monitor.get_record_by_session_name("Web-Server"))
        self.assertEqual(len(self.monitor.get_all_records()), 1)

        # Unregister
        self.monitor.unregister_terminal(10)
        self.assertIsNone(self.monitor.get_record(10))
        self.assertEqual(len(self.monitor.get_all_records()), 0)

    @patch("socket.create_connection")
    def test_probe_host_success(self, mock_create_connection):
        """Verify successful TCP handshake probe."""
        mock_sock = MagicMock()
        mock_create_connection.return_value = mock_sock

        success, rtt, err = SSHHealthMonitor.probe_host("1.1.1.1", 22, timeout=1.0)
        self.assertTrue(success)
        self.assertIsNotNone(rtt)
        self.assertGreaterEqual(rtt, 0.0)
        self.assertIsNone(err)
        mock_sock.close.assert_called_once()

    @patch("socket.create_connection")
    def test_probe_host_timeout(self, mock_create_connection):
        """Verify probe timeout handling."""
        mock_create_connection.side_effect = socket.timeout("timed out")

        success, rtt, err = SSHHealthMonitor.probe_host("10.255.255.1", 22, timeout=0.1)
        self.assertFalse(success)
        self.assertIsNone(rtt)
        self.assertIsNotNone(err)

    @patch("socket.create_connection")
    def test_probe_host_refused(self, mock_create_connection):
        """Verify probe connection refused handling."""
        mock_create_connection.side_effect = ConnectionRefusedError("Connection refused")

        success, rtt, err = SSHHealthMonitor.probe_host("127.0.0.1", 9999, timeout=0.1)
        self.assertFalse(success)
        self.assertIsNone(rtt)
        self.assertIn("9999", err)

    def test_probe_result_updates_thresholds(self):
        """Verify status categorization based on measured RTT."""
        session = SessionItem(name="Test-Srv", host="127.0.0.1", session_type="ssh")
        self.monitor.register_terminal(20, session)

        # Healthy (< 150ms)
        self.monitor._update_record_probe_result(20, True, 60.0, None)
        rec = self.monitor.get_record(20)
        self.assertEqual(rec.status, SSHHealthStatus.HEALTHY)
        self.assertEqual(rec.rtt_ms, 60.0)

        # Degraded (150ms - 350ms)
        self.monitor._update_record_probe_result(20, True, 220.0, None)
        self.assertEqual(rec.status, SSHHealthStatus.DEGRADED)

        # Poor (>= 350ms)
        self.monitor._update_record_probe_result(20, True, 450.0, None)
        self.assertEqual(rec.status, SSHHealthStatus.POOR)

        # Failure 1: consecutive failure incremented, still POOR
        self.monitor._update_record_probe_result(20, False, None, "Connection timeout")
        self.assertEqual(rec.consecutive_failures, 1)

        # Failure 2: reaches threshold -> UNREACHABLE
        self.monitor._update_record_probe_result(20, False, None, "Connection timeout")
        self.assertEqual(rec.consecutive_failures, 2)
        self.assertEqual(rec.status, SSHHealthStatus.UNREACHABLE)

    def test_notify_connection_lost_and_countdown(self):
        """Verify connection lost notification and auto-reconnect countdown."""
        session = SessionItem(name="Cluster-01", host="10.1.1.1", session_type="ssh")
        reconnect_called = []

        self.monitor.register_terminal(
            30,
            session,
            reconnect_callback=lambda tid: reconnect_called.append(tid),
        )

        # Notify lost
        rec = self.monitor.notify_connection_lost(30, reason="Broken pipe")
        self.assertEqual(rec.status, SSHHealthStatus.RECONNECTING)
        self.assertTrue(rec.is_auto_reconnecting)
        self.assertEqual(rec.reconnect_attempt, 1)
        self.assertEqual(rec.countdown_seconds, 5)

        # Cancel auto-reconnect
        self.monitor.cancel_auto_reconnect(30)
        self.assertFalse(rec.is_auto_reconnecting)
        self.assertEqual(rec.status, SSHHealthStatus.UNREACHABLE)

        # Force trigger reconnect now
        self.monitor.trigger_reconnect_attempt(30)
        self.assertIn(30, reconnect_called)

        # Notify connection restored
        self.monitor.notify_connection_restored(30)
        self.assertFalse(rec.is_auto_reconnecting)
        self.assertEqual(rec.reconnect_attempt, 0)
        self.assertEqual(rec.status, SSHHealthStatus.HEALTHY)

    def test_app_signals_propagation(self):
        """Verify that ssh-health-updated and ssh-connection-lost signals emit properly."""
        received_health = []
        received_lost = []

        signals = AppSignals.get()
        hid1 = signals.connect("ssh-health-updated", lambda _, r: received_health.append(r))
        hid2 = signals.connect("ssh-connection-lost", lambda _, tid, s, r: received_lost.append((tid, r)))

        try:
            session = SessionItem(name="Signal-Test", host="srv.lan", session_type="ssh")
            self.monitor.register_terminal(40, session)
            self.monitor._update_record_probe_result(40, True, 28.5, None)

            self.assertGreaterEqual(len(received_health), 1)
            self.assertEqual(received_health[-1].terminal_id, 40)

            self.monitor.notify_connection_lost(40, reason="Connection reset")
            self.assertEqual(len(received_lost), 1)
            self.assertEqual(received_lost[0], (40, "Connection reset"))
        finally:
            signals.disconnect(hid1)
            signals.disconnect(hid2)

    def test_session_menu_includes_ping_item(self):
        """Verify that create_session_menu includes the Test Connection (Ping / Latency) item for SSH."""
        from onyxsh.ui.menus import create_session_menu
        session = SessionItem(name="SshMenuTest", host="10.0.0.1", session_type="ssh")
        store = MagicMock()
        store.find.return_value = (True, 0)
        menu = create_session_menu(session, store, 0)

        # Check that one of the items has action "win.ping-session"
        has_ping = False
        for i in range(menu.get_n_items()):
            action = menu.get_item_attribute_value(i, "action")
            if action and action.get_string() == "win.ping-session":
                has_ping = True
                break
        self.assertTrue(has_ping, "create_session_menu should include win.ping-session for SSH")

    def test_settings_change_listener(self):
        """Verify that SSHHealthMonitor reacts to settings manager changes."""
        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda k, d=None: {
            "ssh_health_check_enabled": True,
            "ssh_health_check_interval": 15,
            "ssh_auto_reconnect_enabled": True,
            "ssh_auto_reconnect_attempts": 3,
            "ssh_auto_reconnect_delay": 7,
        }.get(k, d)

        from onyxsh.terminal.ssh_health_monitor import SSHHealthMonitor
        monitor = SSHHealthMonitor(mock_settings)
        mock_settings.add_change_listener.assert_called()

        # Simulate setting changed
        mock_settings.get.side_effect = lambda k, d=None: {
            "ssh_health_check_enabled": False,
            "ssh_health_check_interval": 20,
            "ssh_auto_reconnect_enabled": False,
            "ssh_auto_reconnect_attempts": 2,
            "ssh_auto_reconnect_delay": 10,
        }.get(k, d)

        monitor._on_setting_changed("ssh_health_check_interval", 15, 20)
        self.assertEqual(monitor.check_interval, 20)
        self.assertFalse(monitor.enabled)

        # Stop and verify listener removed
        monitor.stop()
        mock_settings.remove_change_listener.assert_called()

    def test_error_banner_signal_cleanup(self):
        """Verify that SSHErrorBanner disconnects AppSignals on destroy()."""
        from onyxsh.ui.widgets.ssh_error_banner import SSHErrorBanner, SSHErrorBannerManager
        banner = SSHErrorBanner(session_name="TestSsh", error_message="Error", terminal_id=99)
        self.assertIsNotNone(banner._health_signal_id)

        banner.destroy()
        self.assertIsNone(banner._health_signal_id)


if __name__ == "__main__":
    unittest.main()
