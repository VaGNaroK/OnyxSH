"""
Unit tests for Keyboard Copy-Mode (tmux-style accessible selection) in OnyxSH.
Validates activation, navigation keys, selection bounds, clipboard copying, and event lifecycle.
"""

import unittest
from unittest.mock import MagicMock, patch

import gi

try:
    gi.require_version("Gdk", "4.0")
    gi.require_version("Gtk", "4.0")
    gi.require_version("Vte", "3.91")
except (ValueError, AttributeError):
    pass

from gi.repository import Gdk, Vte

from onyxsh.terminal.copy_mode import KeyboardCopyModeController


class TestKeyboardCopyMode(unittest.TestCase):
    def setUp(self):
        self.mock_terminal = MagicMock()
        self.mock_terminal.get_cursor_position.return_value = (10, 5)  # col 10, row 5
        self.mock_terminal.get_column_count.return_value = 80
        self.mock_terminal.get_row_count.return_value = 50
        self.mock_terminal.get_width.return_value = 640
        self.mock_terminal.get_parent.return_value = None
        self.mock_terminal._keyboard_copy_controller = None

        self.mock_clipboard = MagicMock()
        self.mock_terminal.get_clipboard.return_value = self.mock_clipboard

        self.mock_window = MagicMock()
        self.controller = KeyboardCopyModeController(
            self.mock_terminal, parent_window=self.mock_window
        )

    def test_initial_state(self):
        self.assertFalse(self.controller.is_active)
        self.assertFalse(self.controller.is_selecting)
        self.assertIsNone(self.controller.anchor_row)
        self.assertIsNone(self.controller.anchor_col)

    def test_activate_initializes_cursor(self):
        self.controller.activate()
        self.assertTrue(self.controller.is_active)
        self.assertEqual(self.controller.cursor_col, 10)
        self.assertEqual(self.controller.cursor_row, 5)
        self.assertFalse(self.controller.is_selecting)
        self.mock_terminal.add_css_class.assert_called_with("terminal-copy-mode-active")

    def test_deactivate_resets_state(self):
        self.controller.activate()
        self.controller.is_selecting = True
        self.controller.anchor_row = 2
        self.controller.anchor_col = 3

        self.controller.deactivate()
        self.assertFalse(self.controller.is_active)
        self.assertFalse(self.controller.is_selecting)
        self.assertIsNone(self.controller.anchor_row)
        self.assertIsNone(self.controller.anchor_col)
        self.mock_terminal.remove_css_class.assert_called_with(
            "terminal-copy-mode-active"
        )

    def test_toggle(self):
        active = self.controller.toggle()
        self.assertTrue(active)
        self.assertTrue(self.controller.is_active)

        active = self.controller.toggle()
        self.assertFalse(active)
        self.assertFalse(self.controller.is_active)

    def test_arrow_keys_navigation(self):
        self.controller.activate()
        self.assertEqual((self.controller.cursor_row, self.controller.cursor_col), (5, 10))

        # Move left
        handled = self.controller._on_key_pressed(None, Gdk.KEY_Left, 0, Gdk.ModifierType(0))
        self.assertTrue(handled)
        self.assertEqual(self.controller.cursor_col, 9)

        # Move right
        self.controller._on_key_pressed(None, Gdk.KEY_Right, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_col, 10)

        # Move up
        self.controller._on_key_pressed(None, Gdk.KEY_Up, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_row, 4)

        # Move down
        self.controller._on_key_pressed(None, Gdk.KEY_Down, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_row, 5)

    def test_vi_navigation_keys(self):
        self.controller.activate()
        # h (left)
        self.controller._on_key_pressed(None, Gdk.KEY_h, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_col, 9)
        # l (right)
        self.controller._on_key_pressed(None, Gdk.KEY_l, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_col, 10)
        # k (up)
        self.controller._on_key_pressed(None, Gdk.KEY_k, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_row, 4)
        # j (down)
        self.controller._on_key_pressed(None, Gdk.KEY_j, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_row, 5)

    def test_home_and_end_keys(self):
        self.controller.activate()
        # Home / 0
        self.controller._on_key_pressed(None, Gdk.KEY_Home, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_col, 0)

        # End / $
        self.controller._on_key_pressed(None, Gdk.KEY_End, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_col, 79)

    def test_page_up_and_down(self):
        self.controller.activate()
        self.controller.cursor_row = 30
        self.controller._on_key_pressed(None, Gdk.KEY_Page_Up, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_row, 15)

        self.controller._on_key_pressed(None, Gdk.KEY_Page_Down, 0, Gdk.ModifierType(0))
        self.assertEqual(self.controller.cursor_row, 30)

    def test_selection_toggle_with_v_and_space(self):
        self.controller.activate()
        self.assertFalse(self.controller.is_selecting)

        # Press 'v' to start selecting
        self.controller._on_key_pressed(None, Gdk.KEY_v, 0, Gdk.ModifierType(0))
        self.assertTrue(self.controller.is_selecting)
        self.assertEqual(self.controller.anchor_row, 5)
        self.assertEqual(self.controller.anchor_col, 10)

        # Press 'v' again to toggle off
        self.controller._on_key_pressed(None, Gdk.KEY_v, 0, Gdk.ModifierType(0))
        self.assertFalse(self.controller.is_selecting)
        self.assertIsNone(self.controller.anchor_row)

        # Space starts selecting
        self.controller._on_key_pressed(None, Gdk.KEY_space, 0, Gdk.ModifierType(0))
        self.assertTrue(self.controller.is_selecting)
        self.assertEqual(self.controller.anchor_row, 5)

    def test_shift_arrows_auto_starts_selection(self):
        self.controller.activate()
        self.assertFalse(self.controller.is_selecting)

        # Shift + Right
        self.controller._on_key_pressed(
            None, Gdk.KEY_Right, 0, Gdk.ModifierType.SHIFT_MASK
        )
        self.assertTrue(self.controller.is_selecting)
        self.assertEqual(self.controller.anchor_row, 5)
        self.assertEqual(self.controller.anchor_col, 10)
        self.assertEqual(self.controller.cursor_col, 11)

    def test_selection_bounds_forward_and_backward(self):
        self.controller.activate()
        self.controller.anchor_row = 2
        self.controller.anchor_col = 4
        self.controller.is_selecting = True

        # Forward
        self.controller.cursor_row = 5
        self.controller.cursor_col = 15
        bounds = self.controller.get_selection_bounds()
        self.assertEqual(bounds, (2, 4, 5, 15))

        # Backward
        self.controller.cursor_row = 1
        self.controller.cursor_col = 2
        bounds = self.controller.get_selection_bounds()
        self.assertEqual(bounds, (1, 2, 2, 4))

    def test_escape_cancels_selection_first(self):
        self.controller.activate()
        self.controller.is_selecting = True
        self.controller.anchor_row = 5
        self.controller.anchor_col = 10

        # First Escape: cancels selection, remains in Copy-Mode
        self.controller._on_key_pressed(None, Gdk.KEY_Escape, 0, Gdk.ModifierType(0))
        self.assertFalse(self.controller.is_selecting)
        self.assertIsNone(self.controller.anchor_row)
        self.assertTrue(self.controller.is_active)

        # Second Escape: exits Copy-Mode
        self.controller._on_key_pressed(None, Gdk.KEY_Escape, 0, Gdk.ModifierType(0))
        self.assertFalse(self.controller.is_active)

    def test_copy_selection_with_enter(self):
        self.controller.activate()
        self.controller.is_selecting = True
        self.controller.anchor_row = 5
        self.controller.anchor_col = 0
        self.controller.cursor_row = 5
        self.controller.cursor_col = 20

        self.mock_terminal.get_text_range_format.return_value = ("selected code block", 19)

        self.controller._on_key_pressed(None, Gdk.KEY_Return, 0, Gdk.ModifierType(0))

        self.mock_terminal.get_text_range_format.assert_called_once_with(
            Vte.Format.TEXT, 5, 0, 5, 20
        )
        self.mock_clipboard.set.assert_called_once_with("selected code block")
        self.assertFalse(self.controller.is_active)

    def test_copy_current_line_when_not_selecting(self):
        self.controller.activate()
        self.assertFalse(self.controller.is_selecting)
        self.controller.cursor_row = 3

        self.mock_terminal.get_text_range_format.return_value = ("ls -la /tmp\n", 12)

        self.controller._on_key_pressed(None, Gdk.KEY_y, 0, Gdk.ModifierType(0))

        self.mock_clipboard.set.assert_called_once_with("ls -la /tmp")
        self.assertFalse(self.controller.is_active)

    def test_terminal_manager_integration(self):
        from onyxsh.terminal.manager import TerminalManager

        mock_settings = MagicMock()
        tm = TerminalManager(self.mock_window, mock_settings)

        term = self.mock_terminal
        # Toggle on
        active = tm.toggle_terminal_keyboard_copy_mode(term)
        self.assertTrue(active)
        self.assertTrue(term._keyboard_copy_controller.is_active)

        # Toggle off
        active = tm.toggle_terminal_keyboard_copy_mode(term)
        self.assertFalse(active)
        self.assertFalse(term._keyboard_copy_controller.is_active)


if __name__ == "__main__":
    unittest.main()
