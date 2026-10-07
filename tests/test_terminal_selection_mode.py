# tests/test_terminal_selection_mode.py

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
from onyxsh.terminal.tabs import _create_terminal_pane
from onyxsh.ui.actions import WindowActions
from onyxsh.ui.dialogs.command_palette_dialog import CommandPaletteDialog
from onyxsh.ui.dialogs.shortcuts_dialog import SHORTCUT_DATA


class TestTerminalSelectionMode(unittest.TestCase):
    """Test suite for terminal selection mode, mouse capture indicator, and escape handling."""

    def setUp(self):
        self.mock_window = MagicMock()
        self.mock_window.settings_manager = MagicMock()
        self.mock_window.tab_manager = MagicMock()
        self.mock_window.toast_overlay = MagicMock()

    def test_default_shortcut_and_actions_registration(self):
        """Verify that toggle-selection-mode is registered in shortcuts, actions, and dialogs."""
        defaults = DefaultSettings.get_defaults()
        self.assertIn("toggle-selection-mode", defaults["shortcuts"])
        self.assertEqual(defaults["shortcuts"]["toggle-selection-mode"], "<Control><Shift>s")

        actions = WindowActions(self.mock_window)
        self.assertTrue(hasattr(actions, "toggle_selection_mode"))
        self.assertTrue(callable(actions.toggle_selection_mode))

        all_shortcuts = []
        for group in SHORTCUT_DATA:
            for item in group["shortcuts"]:
                all_shortcuts.append(item[0])
        self.assertIn("toggle-selection-mode", all_shortcuts)

    def test_set_terminal_selection_mode_enable(self):
        """Test enabling selection mode dispatches DEC private resets and updates UI."""
        tm = TerminalManager.__new__(TerminalManager)
        tm.logger = MagicMock()
        tm.parent_window = self.mock_window

        mock_terminal = MagicMock(spec=Vte.Terminal)
        mock_terminal._selection_mode_active = False

        mock_pane = MagicMock()
        mock_toggle_btn = MagicMock()
        mock_toggle_btn.get_active.return_value = False
        mock_pane.selection_mode_btn = mock_toggle_btn
        mock_terminal.onyxsh_pane = mock_pane

        result = tm.set_terminal_selection_mode(mock_terminal, True)
        self.assertTrue(result)
        self.assertTrue(mock_terminal._selection_mode_active)

        # Verify DECSET reset sequence fed to VTE
        mock_terminal.feed.assert_called_once_with(b"\033[?1000l\033[?1002l\033[?1003l\033[?1006l")

        # Verify UI styling and toggle button state
        mock_pane.add_css_class.assert_called_once_with("selection-mode-active")
        mock_toggle_btn.set_active.assert_called_once_with(True)

    def test_set_terminal_selection_mode_disable(self):
        """Test disabling selection mode restores DEC mouse tracking and cleans UI."""
        tm = TerminalManager.__new__(TerminalManager)
        tm.logger = MagicMock()
        tm.parent_window = self.mock_window

        mock_terminal = MagicMock(spec=Vte.Terminal)
        mock_terminal._selection_mode_active = True

        mock_pane = MagicMock()
        mock_toggle_btn = MagicMock()
        mock_toggle_btn.get_active.return_value = True
        mock_pane.selection_mode_btn = mock_toggle_btn
        mock_terminal.onyxsh_pane = mock_pane

        result = tm.set_terminal_selection_mode(mock_terminal, False)
        self.assertFalse(result)
        self.assertFalse(mock_terminal._selection_mode_active)

        # Verify DECSET set sequence fed to VTE
        mock_terminal.feed.assert_called_once_with(b"\033[?1000h\033[?1002h\033[?1006h")

        # Verify UI class removal and toggle button state
        mock_pane.remove_css_class.assert_called_once_with("selection-mode-active")
        mock_toggle_btn.set_active.assert_called_once_with(False)

    def test_toggle_terminal_selection_mode(self):
        """Test toggling flips state between enabled and disabled."""
        tm = TerminalManager.__new__(TerminalManager)
        tm.logger = MagicMock()
        tm.parent_window = self.mock_window

        mock_terminal = MagicMock(spec=Vte.Terminal)
        mock_terminal._selection_mode_active = False

        state1 = tm.toggle_terminal_selection_mode(mock_terminal)
        self.assertTrue(state1)
        self.assertTrue(mock_terminal._selection_mode_active)

        state2 = tm.toggle_terminal_selection_mode(mock_terminal)
        self.assertFalse(state2)
        self.assertFalse(mock_terminal._selection_mode_active)

    def test_escape_key_exits_selection_mode(self):
        """Test pressing Escape exits selection mode and stops event propagation."""
        tm = TerminalManager.__new__(TerminalManager)
        tm.logger = MagicMock()
        tm.parent_window = self.mock_window

        mock_terminal = MagicMock(spec=Vte.Terminal)
        mock_terminal._selection_mode_active = True
        mock_terminal._completion_popup = None

        with patch.object(tm, "set_terminal_selection_mode") as mock_set:
            res = tm._on_terminal_key_pressed_for_detection(
                controller=MagicMock(),
                keyval=Gdk.KEY_Escape,
                _keycode=9,
                state=Gdk.ModifierType(0),
                terminal=mock_terminal,
                terminal_id=1,
            )
            self.assertEqual(res, Gdk.EVENT_STOP)
            mock_set.assert_called_once_with(mock_terminal, False)

    def test_window_actions_toggle_selection_mode(self):
        """Test calling WindowActions.toggle_selection_mode routes to active terminal."""
        actions = WindowActions(self.mock_window)
        mock_terminal = MagicMock(spec=Vte.Terminal)
        self.mock_window.tab_manager.get_selected_terminal.return_value = mock_terminal

        mock_tm = MagicMock()
        mock_terminal.terminal_manager = mock_tm

        actions.toggle_selection_mode()
        mock_tm.toggle_terminal_selection_mode.assert_called_once_with(mock_terminal)

    def test_pane_header_creates_selection_and_hint_widgets(self):
        """Test pane creation adds mouse hint badge and selection mode button."""
        real_terminal = Vte.Terminal()
        mock_settings = MagicMock()

        # 1. OpenCode pane: mouse hint badge is visible
        pane_opencode = _create_terminal_pane(
            terminal=real_terminal,
            title="OpenCode",
            on_close_callback=MagicMock(),
            on_move_to_tab_callback=MagicMock(),
            settings_manager=mock_settings,
        )
        self.assertTrue(hasattr(pane_opencode, "mouse_hint_badge"))
        self.assertTrue(hasattr(pane_opencode, "selection_mode_btn"))
        self.assertTrue(pane_opencode.mouse_hint_badge.get_visible())

        # 2. Regular terminal pane: mouse hint badge is hidden initially
        real_terminal_2 = Vte.Terminal()
        pane_regular = _create_terminal_pane(
            terminal=real_terminal_2,
            title="Terminal",
            on_close_callback=MagicMock(),
            on_move_to_tab_callback=MagicMock(),
            settings_manager=mock_settings,
        )
        self.assertFalse(pane_regular.mouse_hint_badge.get_visible())

    def test_command_palette_includes_selection_mode(self):
        """Test command palette registers the selection mode item."""
        mock_window = MagicMock()
        mock_window.settings_manager = None
        dialog = CommandPaletteDialog.__new__(CommandPaletteDialog)
        dialog.parent_window = mock_window
        dialog._items = []
        dialog.logger = MagicMock()
        dialog._build_catalog()

        item_actions = [item.action_name for item in dialog._items if item.action_name]
        self.assertIn("toggle-selection-mode", item_actions)


if __name__ == "__main__":
    unittest.main()
