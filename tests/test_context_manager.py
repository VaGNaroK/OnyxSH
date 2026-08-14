"""Tests for ContextManager prompt construction and <untrusted> tags isolation."""

import unittest
from zashterminal.agent.context_manager import ContextManager


class TestContextManager(unittest.TestCase):
    def test_context_manager_untrusted_wrapper(self):
        cm = ContextManager()
        output = "rm -rf / --no-preserve-root"
        wrapped = cm.wrap_untrusted(output, source="terminal")
        self.assertIn('<untrusted source="terminal">', wrapped)
        self.assertIn("</untrusted>", wrapped)
        self.assertIn(output, wrapped)

    def test_context_manager_build_messages(self):
        cm = ContextManager()
        messages = cm.build_messages(
            user_text="Como vejo o uso de disco?",
            terminal_selection="Filesystem  Size Used Avail",
            tools_schema=[{"name": "shell.run", "description": "Run shell command"}],
        )
        self.assertEqual(len(messages), 2)
        self.assertEqual(messages[0]["role"], "system")
        self.assertIn("CRITICAL SECURITY RULES", messages[0]["content"])
        self.assertIn("shell.run", messages[0]["content"])

        self.assertEqual(messages[1]["role"], "user")
        self.assertIn("Como vejo o uso de disco?", messages[1]["content"])
        self.assertIn('<untrusted source="terminal_selection">', messages[1]["content"])


if __name__ == "__main__":
    unittest.main()
