# tests/test_signal_cleanup.py
"""
Unit tests for AppSignals cleanup in TabManager, SessionTreeView, and Window._perform_cleanup.
"""

import unittest
from unittest.mock import MagicMock, patch

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gio, GObject, Gtk

from onyxsh.core.signals import AppSignals
from onyxsh.sessions.tree import SessionTreeView
from onyxsh.terminal.tabs import TabManager
from onyxsh.terminal.tunnel_manager import SSHTunnelManager


class TestSignalCleanup(unittest.TestCase):
    """Verifies that TabManager and SessionTreeView cleanly disconnect their AppSignals handlers."""

    def test_tab_manager_cleanup_disconnects_signals(self):
        """Test that TabManager connects to ssh-health-updated and disconnects on cleanup."""
        mock_term_mgr = MagicMock()
        mock_term_mgr.settings_manager = MagicMock()
        mock_scrolled_tab_bar = Gtk.ScrolledWindow()

        tab_mgr = TabManager(
            terminal_manager=mock_term_mgr,
            on_quit_callback=MagicMock(),
            on_detach_tab_callback=MagicMock(),
            scrolled_tab_bar=mock_scrolled_tab_bar,
        )

        # Should have stored a valid signal handler id and be connected
        signals = AppSignals.get()
        hid = tab_mgr._ssh_health_signal_id
        self.assertIsNotNone(hid)
        self.assertTrue(signals.handler_is_connected(hid))

        # Perform cleanup
        tab_mgr.cleanup()
        self.assertIsNone(tab_mgr._ssh_health_signal_id)
        self.assertFalse(signals.handler_is_connected(hid))

        # Calling cleanup again should be safe and idempotent
        tab_mgr.cleanup()
        self.assertIsNone(tab_mgr._ssh_health_signal_id)

    def test_session_tree_cleanup_disconnects_all_signals(self):
        """Test that SessionTreeView connects to 8 signals and disconnects all on cleanup."""
        mock_window = MagicMock()
        mock_session_store = Gio.ListStore.new(GObject.GObject)
        mock_folder_store = Gio.ListStore.new(GObject.GObject)
        mock_settings = MagicMock()
        mock_settings.get.return_value = False
        mock_ops = MagicMock()

        tree_view = SessionTreeView(
            parent_window=mock_window,
            session_store=mock_session_store,
            folder_store=mock_folder_store,
            settings_manager=mock_settings,
            operations=mock_ops,
        )

        # Should have connected 8 signals
        signals = AppSignals.get()
        self.assertEqual(len(tree_view._signal_handler_ids), 8)
        registered_hids = list(tree_view._signal_handler_ids)
        for hid in registered_hids:
            self.assertTrue(signals.handler_is_connected(hid))

        # Perform cleanup
        tree_view.cleanup()
        self.assertEqual(len(tree_view._signal_handler_ids), 0)
        for hid in registered_hids:
            self.assertFalse(signals.handler_is_connected(hid))

        # Idempotency
        tree_view.cleanup()
        self.assertEqual(len(tree_view._signal_handler_ids), 0)

    def test_window_perform_cleanup_calls_component_cleanups(self):
        """Test that Window._perform_cleanup invokes cleanup on session_tree, tab_manager, and tunnel_manager."""
        from onyxsh.window import CommTerminalWindow

        mock_win = MagicMock()
        mock_win._cleanup_performed = False
        mock_win.settings_manager.get.return_value = False
        mock_win.tab_manager.file_managers = {}
        mock_win.tftp_server = None
        mock_win.ai_assistant = None

        with patch("onyxsh.terminal.tunnel_manager.SSHTunnelManager.get_instance") as mock_get_tm:
            mock_tm_instance = MagicMock()
            mock_get_tm.return_value = mock_tm_instance

            CommTerminalWindow._perform_cleanup(mock_win)

            # Verify components were cleaned up
            mock_win.session_tree.cleanup.assert_called_once()
            mock_win.tab_manager.cleanup.assert_called_once()
            mock_tm_instance.shutdown.assert_called_once()
            self.assertTrue(mock_win._cleanup_performed)


if __name__ == "__main__":
    unittest.main()
