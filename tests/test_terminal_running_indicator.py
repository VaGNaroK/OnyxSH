# tests/test_terminal_running_indicator.py

import time
import unittest
from unittest.mock import MagicMock, patch

import gi
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Vte", "3.91")
from gi.repository import GLib, Gtk, Adw, Vte

from onyxsh.terminal.semantic_tracker import (
    SemanticCommand,
    SemanticTerminalState,
    SemanticTracker,
)
from onyxsh.settings.config import DefaultSettings
from onyxsh.terminal.tabs import TabManager


class TestTerminalRunningIndicator(unittest.TestCase):
    """Test suite for live execution loading spinner and ticking duration timer."""

    def setUp(self):
        self.tracker = SemanticTracker()
        self.mock_terminal = MagicMock(spec=Vte.Terminal)
        self.mock_terminal.get_parent.return_value = None
        self.mock_terminal.get_vadjustment.return_value = None
        self.mock_terminal.get_cursor_position.return_value = (0, 5)
        self.mock_terminal.get_text_range.return_value = ("output\n", [])
        self.mock_terminal.get_text_range_format.return_value = ("output\n", 7)

    def test_default_config_includes_running_indicator(self):
        """Verify default configuration has show_command_running_indicator enabled."""
        defaults = DefaultSettings.get_defaults()
        self.assertIn("show_command_running_indicator", defaults)
        self.assertTrue(defaults["show_command_running_indicator"])

    def test_semantic_tracker_dispatches_started_callback(self):
        """Verify tracker invokes registered callback when OSC 133 C is received."""
        started_calls = []

        def on_started(term, cmd):
            started_calls.append((term, cmd))

        self.tracker.register_command_started_callback(on_started)

        # OSC 133 A -> B -> C
        self.tracker.handle_osc133(self.mock_terminal, "A")
        self.tracker.handle_osc133(self.mock_terminal, "B")
        self.tracker.handle_osc133(self.mock_terminal, "C")

        # Process GLib idle handlers
        ctx = GLib.main_context_default()
        while ctx.iteration(False):
            pass

        self.assertEqual(len(started_calls), 1)
        term, cmd = started_calls[0]
        self.assertEqual(term, self.mock_terminal)
        self.assertIsInstance(cmd, SemanticCommand)
        self.assertFalse(cmd.is_finished)

    def test_tab_manager_show_and_stop_running_indicator(self):
        """Test show_command_running_indicator setup and _stop_running_indicator teardown."""
        # Create a mock TabManager instance
        tab_manager = object.__new__(TabManager)
        tab_manager.tabs = []
        tab_manager.pages = {}
        tab_manager.terminal_manager = MagicMock()
        mock_page = MagicMock()
        tab_manager.get_page_for_terminal = MagicMock(return_value=mock_page)
        tab_manager._find_pane_for_terminal = MagicMock(return_value=None)
        tab_manager._get_tab_for_page = MagicMock(return_value=None)

        terminal = MagicMock()
        terminal._running_timer_id = None
        terminal._running_cmd = None
        terminal.semantic_status_box = MagicMock()
        terminal.semantic_status_label = MagicMock()
        terminal.semantic_spinner = MagicMock()
        terminal.semantic_quick_fix_btn = MagicMock()
        terminal.semantic_ai_btn = MagicMock()
        terminal.semantic_copy_btn = MagicMock()

        cmd = SemanticCommand(command_id="cmd_test", command_text="sleep 2")

        with patch("gi.repository.GLib.timeout_add", return_value=12345) as mock_timeout_add, \
             patch("gi.repository.GLib.source_remove") as mock_source_remove:

            # Start indicator
            tab_manager.show_command_running_indicator(terminal, cmd)
            self.assertEqual(terminal._running_cmd, cmd)
            self.assertEqual(terminal._running_timer_id, 12345)
            self.assertGreater(terminal._running_start_time, 0.0)
            mock_timeout_add.assert_called_once()

            # Stop indicator
            tab_manager._stop_running_indicator(terminal)
            self.assertIsNone(terminal._running_cmd)
            self.assertIsNone(terminal._running_timer_id)
            mock_source_remove.assert_called_once_with(12345)

    def test_running_timer_tick_debounce(self):
        """Test that _on_running_timer_tick debounces sub-200ms executions."""
        tab_manager = object.__new__(TabManager)
        tab_manager._set_running_visuals = MagicMock()

        terminal = MagicMock()
        terminal._running_cmd = SemanticCommand(command_id="cmd_1")
        terminal._running_start_time = time.time()  # 0ms elapsed

        # When elapsed < 0.2s, should not call _set_running_visuals
        res = tab_manager._on_running_timer_tick(terminal)
        self.assertEqual(res, GLib.SOURCE_CONTINUE)
        tab_manager._set_running_visuals.assert_not_called()

        # When elapsed >= 0.2s (e.g. simulated by setting start_time 0.5s in past)
        terminal._running_start_time = time.time() - 0.5
        res = tab_manager._on_running_timer_tick(terminal)
        self.assertEqual(res, GLib.SOURCE_CONTINUE)
        tab_manager._set_running_visuals.assert_called_once()
        args, kwargs = tab_manager._set_running_visuals.call_args
        self.assertEqual(args[0], terminal)
        self.assertTrue(kwargs["is_running"])
        self.assertIn("0.5s", kwargs["time_text"])

    def test_running_timer_tick_minutes_formatting(self):
        """Test formatted elapsed time when command runs for over a minute."""
        tab_manager = object.__new__(TabManager)
        tab_manager._set_running_visuals = MagicMock()

        terminal = MagicMock()
        terminal._running_cmd = SemanticCommand(command_id="cmd_long")
        # 65 seconds ago
        terminal._running_start_time = time.time() - 65.2

        tab_manager._on_running_timer_tick(terminal)
        tab_manager._set_running_visuals.assert_called_once()
        _, kwargs = tab_manager._set_running_visuals.call_args
        self.assertEqual(kwargs["time_text"], "1m 5s")

    def test_set_running_visuals_applies_and_removes_running_state(self):
        """Test _set_running_visuals updates badge classes, spinners, and buttons."""
        tab_manager = object.__new__(TabManager)
        mock_page = MagicMock()
        tab_manager.get_page_for_terminal = MagicMock(return_value=mock_page)
        tab_manager._find_pane_for_terminal = MagicMock(return_value=None)
        tab_manager._get_tab_for_page = MagicMock(return_value=None)

        terminal = MagicMock()
        status_box = MagicMock()
        status_label = MagicMock()
        spinner = MagicMock()
        quick_fix_btn = MagicMock()
        ai_btn = MagicMock()
        copy_btn = MagicMock()

        terminal.semantic_status_box = status_box
        terminal.semantic_status_label = status_label
        terminal.semantic_spinner = spinner
        terminal.semantic_quick_fix_btn = quick_fix_btn
        terminal.semantic_ai_btn = ai_btn
        terminal.semantic_copy_btn = copy_btn

        # 1. Enter running state
        tab_manager._set_running_visuals(terminal, is_running=True, time_text="1.2s")
        status_box.add_css_class.assert_called_with("running")
        status_box.set_visible.assert_called_with(True)
        spinner.set_visible.assert_called_with(True)
        spinner.start.assert_called_once()
        status_label.set_text.assert_called_with("1.2s")
        quick_fix_btn.set_visible.assert_called_with(False)
        ai_btn.set_visible.assert_called_with(False)
        copy_btn.set_visible.assert_called_with(False)

        # 2. Exit running state
        tab_manager._set_running_visuals(terminal, is_running=False)
        spinner.stop.assert_called_once()
        spinner.set_visible.assert_called_with(False)
        status_box.remove_css_class.assert_called_with("running")

    def test_update_semantic_badge_cleans_running_indicator(self):
        """Test update_semantic_badge_for_terminal stops the running indicator upon completion."""
        tab_manager = object.__new__(TabManager)
        mock_page = MagicMock()
        tab_manager.get_page_for_terminal = MagicMock(return_value=mock_page)
        tab_manager._find_pane_for_terminal = MagicMock(return_value=None)
        tab_manager._get_tab_for_page = MagicMock(return_value=None)
        tab_manager._stop_running_indicator = MagicMock()

        terminal = MagicMock()
        terminal.semantic_status_box = MagicMock()
        terminal.semantic_status_label = MagicMock()
        terminal.semantic_spinner = MagicMock()
        terminal.semantic_quick_fix_btn = MagicMock()
        terminal.semantic_ai_btn = MagicMock()
        terminal.semantic_copy_btn = MagicMock()

        cmd = SemanticCommand(command_id="cmd_finished", duration=3.5, exit_code=0)

        tab_manager.update_semantic_badge_for_terminal(terminal, cmd)

        # Must have stopped running indicator
        tab_manager._stop_running_indicator.assert_called_once_with(terminal)
        # Spinner stopped and removed running class
        terminal.semantic_spinner.stop.assert_called_once()
        terminal.semantic_spinner.set_visible.assert_called_with(False)
        terminal.semantic_status_box.remove_css_class.assert_any_call("running")
        # Final duration badge set
        terminal.semantic_status_label.set_text.assert_called_with("⏱ 3.5s")
        terminal.semantic_copy_btn.set_visible.assert_called_with(True)

    def test_manager_toggle_respects_setting(self):
        """Test that manager does not show indicator if setting is disabled."""
        from onyxsh.terminal.manager import TerminalManager

        mgr = object.__new__(TerminalManager)
        mgr.settings_manager = MagicMock()
        mgr.tab_manager = MagicMock()

        cmd = SemanticCommand(command_id="cmd_1")

        # Case 1: Enabled
        mgr.settings_manager.get.return_value = True
        mgr._on_semantic_command_started(self.mock_terminal, cmd)
        mgr.tab_manager.show_command_running_indicator.assert_called_once_with(
            self.mock_terminal, cmd
        )

        # Case 2: Disabled
        mgr.tab_manager.show_command_running_indicator.reset_mock()
        mgr.settings_manager.get.return_value = False
        mgr._on_semantic_command_started(self.mock_terminal, cmd)
        mgr.tab_manager.show_command_running_indicator.assert_not_called()


if __name__ == "__main__":
    unittest.main()
