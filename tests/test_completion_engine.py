# tests/test_completion_engine.py
"""Unit tests for the multi-source completion engine."""

import unittest

from onyxsh.terminal.completion.engine import CompletionEngine, get_completion_engine
from onyxsh.terminal.completion.models import CompletionSource, CompletionType


class TestCompletionEngine(unittest.TestCase):
    """Test suite for CompletionEngine."""

    def setUp(self):
        self.engine = CompletionEngine()

    def test_parse_context_simple(self):
        """Test simple command line parsing."""
        ctx = self.engine.parse_context("apt ins", 7)
        self.assertEqual(ctx.tokens, ["apt", "ins"])
        self.assertEqual(ctx.current_word, "ins")
        self.assertEqual(ctx.command_root, "apt")
        self.assertFalse(ctx.is_sudo)

    def test_parse_context_sudo(self):
        """Test sudo unwrapping."""
        ctx = self.engine.parse_context("sudo systemctl start ng", 23)
        self.assertTrue(ctx.is_sudo)
        self.assertEqual(ctx.command_root, "systemctl")
        self.assertEqual(ctx.current_word, "ng")

    def test_parse_context_pipe(self):
        """Test compound command parsing after pipe."""
        ctx = self.engine.parse_context("cat file.txt | grep er", 23)
        self.assertEqual(ctx.command_root, "grep")
        self.assertEqual(ctx.current_word, "er")

    def test_get_completions_root_commands(self):
        """Test root command suggestions (e.g. 'ap' -> 'apt')."""
        items = self.engine.get_completions("ap")
        self.assertTrue(len(items) > 0)
        texts = [i.text for i in items]
        self.assertIn("apt", texts)

    def test_get_completions_sudo_subcommand(self):
        """Test subcommands with sudo prefix (e.g. 'sudo apt ins')."""
        items = self.engine.get_completions("sudo apt ins")
        self.assertTrue(len(items) > 0)
        texts = [i.text for i in items]
        self.assertIn("install", texts)

    def test_get_completions_empty_line(self):
        """Test empty line produces no suggestions."""
        items = self.engine.get_completions("")
        self.assertEqual(items, [])


if __name__ == "__main__":
    unittest.main()
