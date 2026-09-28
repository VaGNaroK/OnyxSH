# tests/test_completion_browse_manager.py
"""Unit tests for manager browse-popup plumbing (Ctrl+Space path)."""

import unittest
from unittest.mock import MagicMock

from onyxsh.terminal.completion.engine import CompletionEngine
from onyxsh.terminal.manager import TerminalManager


class FakeSettings:
    def __init__(self, values):
        self.values = values

    def get(self, key, default=None):
        return self.values.get(key, default)


class FakePopup:
    def __init__(self):
        self.shown_items = None
        self.rect = None
        self.visible = False
        self.popped_down = False

    def get_visible(self):
        return self.visible

    def show_completions(self, items, rect):
        self.shown_items = items
        self.rect = rect
        self.visible = True

    def popdown(self):
        self.visible = False
        self.popped_down = True


class FakeTerminal:
    def __init__(self, line=""):
        self._popup = FakePopup()
        self._completion_popup = self._popup
        self._line = line

    def get_cursor_position(self):
        return (4, 10)

    def get_column_count(self):
        return 80

    def get_text_range_format(self, *args):
        return (f"$ {self._line}", None)

    def get_width(self):
        return 800

    def get_height(self):
        return 600

    def get_char_width(self):
        return 8

    def get_char_height(self):
        return 16

    def get_vadjustment(self):
        adj = MagicMock()
        adj.get_value.return_value = 0.0
        return adj

    def get_realized(self):
        return True

    def get_mapped(self):
        return True


def make_manager():
    mgr = TerminalManager.__new__(TerminalManager)
    mgr.logger = MagicMock()
    mgr.settings_manager = FakeSettings({"autocomplete_enabled": True})
    mgr._completion_engine = CompletionEngine()
    registry = MagicMock()
    registry.get_terminal_info.return_value = {"cwd": "/tmp", "host": "localhost"}
    mgr.registry = registry
    return mgr


class TestBrowseManager(unittest.TestCase):
    def test_open_browse_empty_line_shows_all_commands(self):
        mgr = make_manager()
        term = FakeTerminal(line="")
        result = mgr._open_browse_completions(term, 1)
        self.assertTrue(result)
        texts = [i.text for i in term._popup.shown_items]
        self.assertLessEqual(len(texts), 200)
        self.assertGreater(len(texts), 10)
        self.assertIn("apt", texts)
        self.assertIsNotNone(term._popup.rect)

    def test_open_browse_with_command_shows_subcommands(self):
        mgr = make_manager()
        term = FakeTerminal(line="dnf ")
        result = mgr._open_browse_completions(term, 1)
        self.assertTrue(result)
        texts = [i.text for i in term._popup.shown_items]
        self.assertIn("install", texts)

    def test_show_popup_without_popup_attached_returns_false(self):
        mgr = make_manager()
        term = FakeTerminal(line="")
        del term._completion_popup
        result = mgr._open_browse_completions(term, 1)
        self.assertFalse(result)


if __name__ == "__main__":
    unittest.main()
