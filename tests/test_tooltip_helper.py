"""Unit tests for tooltip helper performance and query-tooltip handling."""

import unittest
from unittest.mock import MagicMock

import gi
gi.require_version("Gtk", "4.0")
from gi.repository import Gtk

from onyxsh.utils.tooltip_helper import TooltipHelper, get_tooltip_helper


class TestTooltipHelper(unittest.TestCase):
    def setUp(self):
        self.helper = TooltipHelper()

    def test_add_tooltip_uses_query_tooltip_on_native(self):
        self.helper._use_native_tooltips = True
        btn = Gtk.Button()
        self.helper.add_tooltip(btn, "Clique para enviar")

        self.assertTrue(btn.get_has_tooltip())
        self.assertEqual(getattr(btn, "_custom_tooltip_text", None), "Clique para enviar")
        self.assertTrue(getattr(btn, "_has_native_tooltip_handler", False))

        # Updating tooltip text
        self.helper.add_tooltip(btn, "Novo texto atualizado")
        self.assertEqual(getattr(btn, "_custom_tooltip_text", None), "Novo texto atualizado")

    def test_add_tooltip_with_shortcut_on_native(self):
        self.helper._use_native_tooltips = True
        btn = Gtk.Button()
        self.helper.add_tooltip_with_shortcut(btn, "Fechar aba", "close-tab")

        self.assertTrue(btn.get_has_tooltip())
        self.assertTrue(getattr(btn, "_has_native_tooltip_handler", False))
        self.assertIn("Fechar aba", getattr(btn, "_custom_tooltip_text", ""))

    def test_empty_tooltip_is_noop(self):
        btn = Gtk.Button()
        self.helper.add_tooltip(btn, "")
        self.assertFalse(btn.get_has_tooltip())


if __name__ == "__main__":
    unittest.main()
