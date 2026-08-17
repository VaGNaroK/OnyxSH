import json
import os
import tempfile
import unittest
from unittest.mock import MagicMock, patch

from zashterminal.sessions.models import SessionItem
from zashterminal.state.window_state import WindowStateManager


class TestSessionRestore(unittest.TestCase):
    """Test suite for intelligent session saving and restoration."""

    def setUp(self):
        self.mock_window = MagicMock()
        self.mock_settings = MagicMock()
        self.mock_tab_manager = MagicMock()
        self.mock_terminal_manager = MagicMock()

        self.mock_window.settings_manager = self.mock_settings
        self.mock_window.tab_manager = self.mock_tab_manager
        self.mock_window.terminal_manager = self.mock_terminal_manager
        self.mock_window.logger = MagicMock()
        self.mock_tab_manager.get_tab_count.return_value = 1
        self.mock_tab_manager.tabs = []

        self.state_manager = WindowStateManager(self.mock_window)

    def test_has_saved_state_empty_and_valid(self):
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump({"tabs": [{"type": "terminal", "session_type": "local"}]}, f)
            temp_path = f.name

        try:
            with patch("zashterminal.state.window_state.STATE_FILE", temp_path):
                self.assertTrue(self.state_manager.has_saved_state())

            with open(temp_path, "w") as f:
                json.dump({"tabs": []}, f)

            with patch("zashterminal.state.window_state.STATE_FILE", temp_path):
                self.assertFalse(self.state_manager.has_saved_state())
        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)

    def test_restore_policy_never(self):
        self.mock_settings.get.side_effect = lambda key, default=None: (
            "never" if key == "session_restore_policy" else default
        )
        with patch.object(self.state_manager, "clear_session_state") as mock_clear:
            result = self.state_manager.restore_session_state()
            self.assertFalse(result)
            mock_clear.assert_called_once()

    def test_restore_policy_ask(self):
        self.mock_settings.get.side_effect = lambda key, default=None: (
            "ask" if key == "session_restore_policy" else default
        )
        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump({"tabs": [{"type": "terminal"}]}, f)
            temp_path = f.name

        try:
            with patch("zashterminal.state.window_state.STATE_FILE", temp_path):
                # When not forced, 'ask' should return False so initial tab + toast is shown
                self.assertFalse(self.state_manager.restore_session_state(force=False))

                # When forced (user clicked restore in toast/dialog), it should restore
                self.assertTrue(self.state_manager.restore_session_state(force=True))
                self.mock_tab_manager.recreate_tab_from_structure.assert_called_once()
        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)

    def test_restore_policy_always_with_panels(self):
        self.mock_settings.get.side_effect = lambda key, default=None: {
            "session_restore_policy": "always",
            "session_restore_ui_panels": True,
        }.get(key, default)

        mock_page = MagicMock()
        self.mock_tab_manager.tabs = [mock_page, MagicMock()]

        with tempfile.NamedTemporaryFile(mode="w", suffix=".json", delete=False) as f:
            json.dump(
                {
                    "tabs": [{"type": "terminal", "session_type": "local"}],
                    "active_tab_index": 0,
                    "sidebar_visible": True,
                    "ai_panel_visible": True,
                },
                f,
            )
            temp_path = f.name

        try:
            self.mock_window.sidebar_revealer = MagicMock()
            self.mock_window.ai_chat_revealer = MagicMock()
            self.mock_window.ai_chat_revealer.get_reveal_child.return_value = False

            with patch("zashterminal.state.window_state.STATE_FILE", temp_path):
                res = self.state_manager.restore_session_state()
                self.assertTrue(res)
                self.mock_tab_manager.recreate_tab_from_structure.assert_called_once()
                self.mock_tab_manager.set_active_tab.assert_called_with(mock_page)
                self.mock_window.sidebar_revealer.set_reveal_child.assert_called_with(True)
                self.mock_window._on_ai_assistant_requested.assert_called_once()
        finally:
            if os.path.exists(temp_path):
                os.remove(temp_path)


if __name__ == "__main__":
    unittest.main()
