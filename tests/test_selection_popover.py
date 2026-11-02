# tests/test_selection_popover.py

import unittest
from unittest.mock import MagicMock, patch

import gi
gi.require_version("Gdk", "4.0")
gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Vte", "3.91")
from gi.repository import Gdk, Gtk, Vte

from onyxsh.settings.config import DefaultSettings
from onyxsh.terminal.manager import TerminalManager
from onyxsh.ui.widgets.selection_popover import SelectionActionPopover
from onyxsh.utils.translation_utils import _


class TestSelectionActionPopover(unittest.TestCase):
    """Test suite for floating text selection toolbar and its action handlers."""

    def setUp(self):
        self.mock_window = MagicMock()
        self.mock_window.settings_manager = MagicMock()
        self.mock_window.tab_manager = MagicMock()
        self.mock_window.ui_builder = MagicMock()
        self.mock_window.action_handler = MagicMock()

        self.mock_chat_panel = MagicMock()
        self.mock_window.ui_builder.ai_chat_panel = self.mock_chat_panel

        self.real_terminal = Vte.Terminal()

    def tearDown(self):
        # Ensure terminal has no dangling popover child
        popover = getattr(self.real_terminal, "_selection_popover", None)
        if popover:
            try:
                popover.unparent()
            except Exception:
                pass

    def test_default_setting_enabled(self):
        """Verify that show_selection_toolbar is enabled by default in settings."""
        defaults = DefaultSettings.get_defaults()
        self.assertIn("show_selection_toolbar", defaults)
        self.assertTrue(defaults["show_selection_toolbar"])

    def test_popover_initialization_and_widgets(self):
        """Test popover creates copy, ask AI, and refactor buttons with correct classes."""
        popover = SelectionActionPopover(self.real_terminal, parent_window=self.mock_window)
        try:
            self.assertEqual(popover.get_parent(), self.real_terminal)
            self.assertFalse(popover.get_autohide())
            self.assertTrue(popover.get_has_arrow())
            self.assertTrue(hasattr(popover, "copy_btn"))
            self.assertTrue(hasattr(popover, "ask_ai_btn"))
            self.assertTrue(hasattr(popover, "refactor_btn"))
        finally:
            popover.unparent()

    def test_show_for_selection_clamps_coordinates(self):
        """Test show_for_selection clamps pointing coordinates safely within terminal allocation."""
        popover = SelectionActionPopover(self.real_terminal, parent_window=self.mock_window)
        try:
            with patch.object(popover, "popup") as mock_popup, \
                 patch.object(popover, "set_pointing_to") as mock_pointing:
                popover.show_for_selection("ls -la", pointer_pos=(-100, 99999))
                mock_popup.assert_called_once()
                mock_pointing.assert_called_once()
                rect = mock_pointing.call_args[0][0]
                self.assertGreaterEqual(rect.x, 0)
                self.assertGreaterEqual(rect.y, 0)
        finally:
            popover.unparent()

    def test_show_for_selection_with_text(self):
        """Test show_for_selection popups with valid text and sets pointing rect."""
        popover = SelectionActionPopover(self.real_terminal, parent_window=self.mock_window)
        try:
            with patch.object(popover, "popup") as mock_popup:
                popover.show_for_selection("git status", pointer_pos=(200, 150))
                self.assertEqual(popover._current_text, "git status")
                mock_popup.assert_called_once()
        finally:
            popover.unparent()

    def test_show_for_selection_empty_text_popdown(self):
        """Test show_for_selection popdown when text is empty or whitespace."""
        popover = SelectionActionPopover(self.real_terminal, parent_window=self.mock_window)
        try:
            with patch.object(popover, "popdown") as mock_popdown:
                popover.show_for_selection("   \n  ")
                mock_popdown.assert_called_once()
        finally:
            popover.unparent()

    def test_copy_clicked_writes_to_clipboard(self):
        """Test clicking copy button writes selected text to clipboard and gives feedback."""
        popover = SelectionActionPopover(self.real_terminal, parent_window=self.mock_window)
        try:
            popover._current_text = "def hello(): pass"
            mock_clipboard = MagicMock()
            with patch.object(self.real_terminal, "get_clipboard", return_value=mock_clipboard):
                popover._on_copy_clicked(popover.copy_btn)
                mock_clipboard.set.assert_called_once_with("def hello(): pass")
                self.assertIn(popover.copy_label.get_label(), ["Copiado!", "Copied!", _("Copiado!")])
        finally:
            popover.unparent()

    def test_ask_ai_clicked_prefills_draft(self):
        """Test clicking Ask AI opens panel and prefills draft text without auto-sending."""
        popover = SelectionActionPopover(self.real_terminal, parent_window=self.mock_window)
        try:
            popover._current_text = "console.log('test');"
            with patch.object(self.real_terminal, "get_current_directory_uri", return_value="file:///home/user/app"):
                popover._on_ask_ai_clicked(popover.ask_ai_btn)

                # Verify AI panel opened
                self.mock_window.ui_builder.show_ai_panel.assert_called_once()

                # Verify draft text pre-filled with CWD and selected code
                self.mock_chat_panel.set_initial_text.assert_called_once()
                args, _ = self.mock_chat_panel.set_initial_text.call_args
                draft_content = args[0]
                self.assertIn("/home/user/app", draft_content)
                self.assertIn("console.log('test');", draft_content)
        finally:
            popover.unparent()

    def test_refactor_clicked_calls_opencode_selection(self):
        """Test clicking Refactor triggers opencode_selection on action handler."""
        popover = SelectionActionPopover(self.real_terminal, parent_window=self.mock_window)
        try:
            popover._current_text = "fn calculate() {}"
            popover._on_refactor_clicked(popover.refactor_btn)
            self.mock_window.action_handler.opencode_selection.assert_called_once()
        finally:
            popover.unparent()

    def test_manager_update_selection_popover_shows_and_hides(self):
        """Test TerminalManager._update_selection_popover shows popover when selection exists."""
        tm = TerminalManager.__new__(TerminalManager)
        tm.logger = MagicMock()
        tm.parent_window = self.mock_window
        tm.settings_manager = MagicMock()
        tm.settings_manager.get.return_value = True

        mock_terminal = MagicMock(spec=Vte.Terminal)
        mock_terminal.get_has_selection.return_value = True
        mock_terminal.get_text_selected.return_value = "cargo build"
        mock_terminal._last_pointer_pos = (100, 80)
        mock_popover = MagicMock()
        mock_terminal._selection_popover = mock_popover

        # 1. Selection exists: should call show_for_selection
        tm._update_selection_popover(mock_terminal)
        mock_popover.show_for_selection.assert_called_once_with("cargo build", (100, 80))

        # 2. Selection cleared: should call popdown
        mock_terminal.get_has_selection.return_value = False
        tm._update_selection_popover(mock_terminal)
        mock_popover.popdown.assert_called_once()

    def test_manager_update_selection_popover_disabled_in_settings(self):
        """Test _copy_selection_after_change skips popover when disabled in settings."""
        tm = TerminalManager.__new__(TerminalManager)
        tm.logger = MagicMock()
        tm.parent_window = self.mock_window
        tm.settings_manager = MagicMock()
        tm.settings_manager.get.return_value = False  # Disabled
        tm._selection_copy_timers = {1: 123}

        mock_terminal = MagicMock(spec=Vte.Terminal)
        with patch.object(tm, "_update_selection_popover") as mock_update, \
             patch.object(tm, "_copy_terminal_selection_to_clipboard"):
            tm._copy_selection_after_change(mock_terminal, 1)
            mock_update.assert_not_called()


if __name__ == "__main__":
    unittest.main()
