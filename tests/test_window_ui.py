"""Unit tests for WindowUIBuilder prewarming and UI construction safeguards."""

import unittest
from unittest.mock import MagicMock, patch

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, GLib, Gtk

from onyxsh.ui.window_ui import WindowUIBuilder


class TestWindowUIBuilder(unittest.TestCase):
    def setUp(self):
        self.mock_window = MagicMock()
        self.mock_window.settings_manager = MagicMock()
        self.mock_window.settings_manager.get.return_value = True
        self.builder = WindowUIBuilder(self.mock_window)

    def test_prewarm_ai_panel_execution(self):
        """Verify _prewarm_ai_panel runs without NameError or crash."""
        with patch.object(self.builder, "_create_ai_chat_panel") as mock_create:
            res = self.builder._prewarm_ai_panel()
            self.assertFalse(res)
            mock_create.assert_called_once()

    def test_prewarm_ai_panel_handles_exception_gracefully(self):
        """Verify _prewarm_ai_panel handles exceptions without raising."""
        with patch.object(
            self.builder, "_create_ai_chat_panel", side_effect=RuntimeError("Test error")
        ):
            res = self.builder._prewarm_ai_panel()
            self.assertFalse(res)

    def test_header_bar_toggle_buttons_types_and_flat_classes(self):
        """Verify all 5 header bar buttons are ToggleButtons and have the 'flat' CSS class."""
        self.builder.tab_manager = MagicMock()
        self.builder.tab_manager.get_tab_bar.return_value = Gtk.Box()
        header_bar = self.builder._create_header_bar()

        buttons = {
            "toggle_sidebar_button": self.builder.toggle_sidebar_button,
            "file_manager_button": self.builder.file_manager_button,
            "command_manager_button": self.builder.command_manager_button,
            "search_button": self.builder.search_button,
            "ai_assistant_button": self.builder.ai_assistant_button,
        }

        for name, btn in buttons.items():
            self.assertIsInstance(btn, Gtk.ToggleButton, f"{name} should be a Gtk.ToggleButton")
            self.assertIn("flat", btn.get_css_classes(), f"{name} should have 'flat' CSS class")

    def test_ai_assistant_button_sync_on_show_and_hide_panel(self):
        """Verify ai_assistant_button active state stays in sync with show/hide AI panel."""
        self.builder.tab_manager = MagicMock()
        self.builder.tab_manager.get_tab_bar.return_value = Gtk.Box()
        self.builder._create_header_bar()

        self.builder.ai_paned = MagicMock()
        self.builder.ai_paned.get_position.return_value = 250
        self.builder.ai_chat_panel = MagicMock()
        self.mock_window.get_height.return_value = 600

        with patch.object(self.builder, "_create_ai_chat_panel"):
            self.builder._ai_panel_visible = False
            self.builder.ai_assistant_button.set_active(False)

            # Show panel -> button should become active
            self.builder.show_ai_panel()
            self.assertTrue(self.builder._ai_panel_visible)
            self.assertTrue(self.builder.ai_assistant_button.get_active())

            # Hide panel -> button should become inactive
            self.builder.hide_ai_panel()
            self.assertFalse(self.builder._ai_panel_visible)
            self.assertFalse(self.builder.ai_assistant_button.get_active())

    def test_on_toggle_ai_assistant_button_handlers(self):
        """Verify _on_toggle_ai_assistant_button triggers appropriate actions."""
        self.builder.tab_manager = MagicMock()
        self.builder.tab_manager.get_tab_bar.return_value = Gtk.Box()
        self.builder._create_header_bar()

        btn = self.builder.ai_assistant_button
        self.mock_window._on_ai_assistant_requested = MagicMock()

        # When button activated and panel closed -> requests AI assistant via GTK toggled signal
        self.builder._ai_panel_visible = False
        btn.set_active(False)
        self.mock_window._on_ai_assistant_requested.reset_mock()
        btn.set_active(True)
        self.mock_window._on_ai_assistant_requested.assert_called_once()

        # When button deactivated and panel open -> hides AI panel
        self.builder._ai_panel_visible = True
        with patch.object(self.builder, "hide_ai_panel") as mock_hide:
            btn.set_active(False)
            mock_hide.assert_called_once()

    def test_window_css_no_sidebar_toggle_transparent_override(self):
        """Ensure window.css does not force .sidebar-toggle-button:checked to transparent."""
        from pathlib import Path
        import onyxsh.ui.window_ui as wui_mod

        css_file = Path(wui_mod.__file__).resolve().parent.parent / "data" / "styles" / "window.css"
        self.assertTrue(css_file.exists())
        content = css_file.read_text()
        self.assertNotIn(".sidebar-toggle-button:checked", content)


    def test_window_command_manager_toggle_sync(self):
        """Verify command_manager_button syncs with CommandManagerDialog."""
        mock_win = MagicMock()
        mock_win.command_manager_button = Gtk.ToggleButton()
        mock_win.command_manager_button.add_css_class("flat")
        mock_win.command_manager_dialog = MagicMock()

        from onyxsh.window import CommTerminalWindow

        # Test _on_toggle_command_manager when button is active
        mock_win.command_manager_dialog.get_visible.return_value = False
        mock_win.command_manager_button.set_active(True)
        CommTerminalWindow._on_toggle_command_manager(mock_win, mock_win.command_manager_button)
        mock_win._show_command_manager_dialog.assert_called_once()

        # Test _on_toggle_command_manager when button is inactive
        mock_win.command_manager_dialog.get_visible.return_value = True
        mock_win.command_manager_button.set_active(False)
        CommTerminalWindow._on_toggle_command_manager(mock_win, mock_win.command_manager_button)
        mock_win.command_manager_dialog.close.assert_called_once()

        # Test _on_command_manager_dialog_close_request
        mock_win.command_manager_button.set_active(True)
        CommTerminalWindow._on_command_manager_dialog_close_request(mock_win, mock_win.command_manager_dialog)
        self.assertFalse(mock_win.command_manager_button.get_active())

        # Test _on_command_manager_dialog_visible_changed
        mock_win.command_manager_dialog.get_visible.return_value = True
        CommTerminalWindow._on_command_manager_dialog_visible_changed(mock_win, mock_win.command_manager_dialog, None)
        self.assertTrue(mock_win.command_manager_button.get_active())

        mock_win.command_manager_dialog.get_visible.return_value = False
        CommTerminalWindow._on_command_manager_dialog_visible_changed(mock_win, mock_win.command_manager_dialog, None)
        self.assertFalse(mock_win.command_manager_button.get_active())

    def test_window_actions_show_command_manager_toggles_button(self):
        """Verify WindowActions.show_command_manager toggles command_manager_button."""
        from onyxsh.ui.actions import WindowActions
        mock_win = MagicMock()
        mock_win.command_manager_button = Gtk.ToggleButton()
        actions = WindowActions(mock_win)

        self.assertFalse(mock_win.command_manager_button.get_active())
        actions.show_command_manager()
        self.assertTrue(mock_win.command_manager_button.get_active())
        actions.show_command_manager()
        self.assertFalse(mock_win.command_manager_button.get_active())


if __name__ == "__main__":
    unittest.main()
