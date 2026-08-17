import unittest
from unittest.mock import MagicMock
from zashterminal.ui.dialogs.command_palette_dialog import (
    CommandPaletteItem,
    _normalize,
)


class TestCommandPalette(unittest.TestCase):
    """Test suite for Command Palette fuzzy searching, scoring and execution."""

    def test_normalize(self):
        self.assertEqual(_normalize("Aba"), "aba")
        self.assertEqual(_normalize("Sessão"), "sessao")
        self.assertEqual(_normalize("Configuração"), "configuracao")
        self.assertEqual(_normalize(""), "")

    def test_item_matching_and_scoring(self):
        item = CommandPaletteItem(
            item_id="new-local-tab",
            title="Nova Aba",
            category="Abas e Janelas",
            icon_name="tab-new-symbolic",
            action_name="new-local-tab",
            keywords=["aba", "tab", "novo", "terminal"],
        )

        # Match empty query
        self.assertTrue(item.matches(""))

        # Exact and partial match
        self.assertTrue(item.matches("nova"))
        self.assertTrue(item.matches("aba"))
        self.assertTrue(item.matches("terminal"))
        self.assertTrue(item.matches("janelas"))

        # Accent-insensitive match
        item_session = CommandPaletteItem(
            item_id="toggle-sidebar",
            title="Sessões SSH",
            category="Sessões & SSH",
            icon_name="view-dual-symbolic",
            keywords=["sessao", "sessoes", "ssh"],
        )
        self.assertTrue(item_session.matches("sessao"))
        self.assertTrue(item_session.matches("sessoes"))
        self.assertTrue(item_session.matches("ssh"))

        # Scoring
        score_exact = item.score("nova aba")
        score_prefix = item.score("nov")
        score_unrelated = item.score("xyz")

        self.assertGreater(score_exact, score_prefix)
        self.assertEqual(score_unrelated, 0)

    def test_item_callback_execution(self):
        mock_callback = MagicMock()
        item = CommandPaletteItem(
            item_id="custom-action",
            title="Ação Custom",
            category="Teste",
            icon_name="test-icon",
            callback=mock_callback,
        )

        item.callback()
        mock_callback.assert_called_once()


if __name__ == "__main__":
    unittest.main()
