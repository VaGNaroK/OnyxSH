# tests/test_opencode_context.py

import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from onyxsh.agent.audit import AuditLogger
from onyxsh.terminal.semantic_tracker import SemanticCommand
from onyxsh.ui.actions import WindowActions
from onyxsh.ui.menus import create_terminal_menu


class TestOpenCodeContext(unittest.TestCase):
    """Test suite for contextual terminal actions, OSC 133 integration, and Zero-Trust untrusted tagging."""

    def setUp(self):
        self.mock_window = MagicMock()
        self.mock_window.tab_manager = MagicMock()
        self.mock_window.terminal_manager = MagicMock()
        self.mock_window.ui_builder = MagicMock()
        self.actions = WindowActions(self.mock_window)

    def test_opencode_selection_wraps_in_untrusted_and_redacts(self):
        mock_terminal = MagicMock()
        mock_terminal.get_has_selection.return_value = True
        mock_terminal.get_text_selected.return_value = (
            "def connect():\n    api_key = 'sk-123456789012345678901234567890'\n    return api_key"
        )
        self.mock_window.tab_manager.get_selected_terminal.return_value = mock_terminal

        self.actions.opencode_selection()

        self.mock_window.ui_builder.show_ai_panel.assert_called_once()
        ai_panel = self.mock_window.ui_builder.ai_chat_panel
        ai_panel.set_initial_text.assert_called_once()
        prompt_arg = ai_panel.set_initial_text.call_args[0][0]

        # Verify <untrusted> tags
        self.assertIn("<untrusted>", prompt_arg)
        self.assertIn("</untrusted>", prompt_arg)

        # Verify token was redacted
        self.assertNotIn("sk-123456789012345678901234567890", prompt_arg)
        self.assertIn("[REDACTED_API_KEY]", prompt_arg)

    def test_opencode_selection_noop_when_no_selection(self):
        mock_terminal = MagicMock()
        mock_terminal.get_has_selection.return_value = False
        self.mock_window.tab_manager.get_selected_terminal.return_value = mock_terminal

        self.actions.opencode_selection()

        self.mock_window.ui_builder.show_ai_panel.assert_not_called()

    def test_resolve_error_with_opencode_captures_osc133(self):
        mock_terminal = MagicMock()
        self.mock_window.tab_manager.get_active_terminal.return_value = mock_terminal

        mock_tracker = MagicMock()
        failed_cmd = SemanticCommand(
            command_id="cmd_99",
            command_text="python3 manage.py migrate",
            prompt_row=10,
            command_row=10,
            output_start_row=11,
            output_end_row=15,
            exit_code=1,
            cwd="/home/user/myproject",
        )
        mock_tracker.get_last_command.return_value = failed_cmd
        mock_tracker.get_last_output_text.return_value = (
            "django.db.utils.OperationalError: no such table: auth_user"
        )
        self.mock_window.terminal_manager.semantic_tracker = mock_tracker

        with tempfile.TemporaryDirectory() as tmp_dir:
            log_path = Path(tmp_dir) / "audit.jsonl"
            with patch("onyxsh.agent.audit.DEFAULT_AUDIT_LOG", log_path):
                self.actions.resolve_error_with_opencode(mock_terminal)

        self.mock_window.ui_builder.show_ai_panel.assert_called_once()
        ai_panel = self.mock_window.ui_builder.ai_chat_panel

        sent_prompt = ai_panel.send_message.call_args[0][0]
        self.assertIn("<untrusted>", sent_prompt)
        self.assertIn("</untrusted>", sent_prompt)
        self.assertIn("python3 manage.py migrate", sent_prompt)
        self.assertIn("django.db.utils.OperationalError", sent_prompt)
        self.assertIn("1", sent_prompt)

    def test_context_menu_opencode_item(self):
        mock_terminal = MagicMock()
        mock_terminal.get_has_selection.return_value = True

        mock_settings = MagicMock()
        mock_settings.get.side_effect = lambda k, d=None: True if k == "ai_assistant_enabled" else d

        with patch("onyxsh.utils.opencode_utils.is_opencode_available", return_value=True):
            menu = create_terminal_menu(mock_terminal, settings_manager=mock_settings)
            self.assertIsNotNone(menu)

        with patch("onyxsh.utils.opencode_utils.is_opencode_available", return_value=False):
            menu_disabled = create_terminal_menu(mock_terminal, settings_manager=mock_settings)
            self.assertIsNotNone(menu_disabled)

    def test_context_menu_resolve_error_when_no_selection(self):
        mock_terminal = MagicMock()
        mock_terminal.get_has_selection.return_value = False
        mock_terminal.get_text_format.return_value = "bash: command not found: foo\n"

        mock_settings = MagicMock()
        mock_settings.get.return_value = False  # generic AI assistant disabled

        with patch("onyxsh.utils.opencode_utils.is_opencode_available", return_value=True):
            menu = create_terminal_menu(mock_terminal, settings_manager=mock_settings)
            self.assertIsNotNone(menu)
            # Iterate through menu sections and items to find win.resolve-error-opencode
            found_action = False
            for i in range(menu.get_n_items()):
                item = menu.get_item_link(i, "section")
                if item:
                    for j in range(item.get_n_items()):
                        action = item.get_item_attribute_value(j, "action")
                        if action and "win.resolve-error-opencode" in str(action):
                            found_action = True
                            break
            self.assertTrue(found_action, "Expected 'win.resolve-error-opencode' in terminal menu")


if __name__ == "__main__":
    unittest.main()
