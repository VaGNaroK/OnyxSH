import json
import unittest
from unittest.mock import MagicMock
from onyxsh.ui.widgets.ai_chat_panel import AIChatPanel


class TestAIChatExport(unittest.TestCase):
    """Test suite for AI chat conversation export functionality."""

    def test_format_conversation_markdown_empty(self):
        panel = AIChatPanel.__new__(AIChatPanel)
        panel._history_manager = MagicMock()
        panel._history_manager.get_history.return_value = []
        result = AIChatPanel._format_conversation_markdown(panel)
        self.assertEqual(result, "")

    def test_format_conversation_markdown_with_messages(self):
        panel = AIChatPanel.__new__(AIChatPanel)
        panel._history_manager = MagicMock()
        panel._history_manager.get_history.return_value = [
            {"role": "user", "timestamp": "2026-08-16T20:00:00", "content": "Como listar arquivos?"},
            {
                "role": "assistant",
                "timestamp": "2026-08-16T20:00:02",
                "content": "Use o comando ls -la",
                "commands": ["ls -la"],
            },
        ]
        panel._history_manager.get_current_conversation.return_value = {
            "id": "test-conv-123",
            "created_at": "2026-08-16T20:00:00",
            "messages": panel._history_manager.get_history.return_value,
        }

        result = AIChatPanel._format_conversation_markdown(panel)
        self.assertIn("# OnyxSH AI Chat Export", result)
        self.assertIn("Como listar arquivos?", result)
        self.assertIn("Use o comando ls -la", result)
        self.assertIn("```bash\nls -la\n```", result)

    def test_format_conversation_json(self):
        panel = AIChatPanel.__new__(AIChatPanel)
        panel._history_manager = MagicMock()
        messages = [
            {"role": "user", "timestamp": "2026-08-16T20:00:00", "content": "Teste prompt"},
            {"role": "assistant", "timestamp": "2026-08-16T20:00:01", "content": "Teste resposta"},
        ]
        panel._history_manager.get_current_conversation.return_value = {
            "id": "test-json-456",
            "created_at": "2026-08-16T20:00:00",
            "messages": messages,
        }
        panel._history_manager.get_history.return_value = messages

        result = AIChatPanel._format_conversation_json(panel)
        data = json.loads(result)
        self.assertEqual(data["id"], "test-json-456")
        self.assertEqual(len(data["messages"]), 2)
        self.assertIn("exported_at", data)


if __name__ == "__main__":
    unittest.main()
