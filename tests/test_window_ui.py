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

    def test_glib_idle_add_prewarm_available(self):
        """Verify GLib is defined and GLib.idle_add can accept _prewarm_ai_panel."""
        from onyxsh.ui import window_ui
        self.assertTrue(hasattr(window_ui, "GLib"))
        self.assertTrue(callable(window_ui.GLib.idle_add))


if __name__ == "__main__":
    unittest.main()
