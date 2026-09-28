# tests/test_completion_browse.py
"""Unit tests for Ctrl+Space browse mode (all commands / subcommands)."""

import unittest

from onyxsh.terminal.completion.engine import CompletionEngine


class TestCompletionBrowse(unittest.TestCase):
    def setUp(self):
        self.engine = CompletionEngine()

    def test_browse_empty_line_lists_commands(self):
        items = self.engine.get_browse_completions("")
        texts = [i.text for i in items]
        self.assertGreater(len(items), 0)
        self.assertIn("apt", texts)
        self.assertIn("dnf", texts)
        self.assertIn("git", texts)

    def test_browse_command_lists_subcommands(self):
        items = self.engine.get_browse_completions("git ")
        texts = [i.text for i in items]
        self.assertIn("commit", texts)

    def test_browse_dnf_lists_subcommands(self):
        items = self.engine.get_browse_completions("dnf ")
        texts = [i.text for i in items]
        self.assertIn("install", texts)
        self.assertIn("repolist", texts)

    def test_browse_partial_prefix_still_filters(self):
        items = self.engine.get_browse_completions("git com")
        texts = [i.text for i in items]
        self.assertIn("commit", texts)

    def test_browse_no_match_falls_back_to_all_commands(self):
        items = self.engine.get_browse_completions("zzz-no-such-cmd xyz")
        texts = [i.text for i in items]
        self.assertGreater(len(items), 0)
        self.assertIn("apt", texts)
        self.assertIn("dnf", texts)


if __name__ == "__main__":
    unittest.main()
