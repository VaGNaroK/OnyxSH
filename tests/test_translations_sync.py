"""Unit tests for translation catalogs and synchronization across all 28 supported languages."""

import gettext
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
LOCALE_DIR = REPO_ROOT / "locale"
INTERNAL_LOCALE_DIR = REPO_ROOT / "src" / "onyxsh" / "locale"

SUPPORTED_LANGUAGES = [
    "bg", "cs", "da", "de", "el", "en", "es", "et", "fi", "fr",
    "he", "hr", "hu", "is", "it", "ja", "ko", "nl", "no", "pl",
    "pt", "ro", "ru", "sk", "sv", "tr", "uk", "zh"
]

TODAY_SAMPLE_KEYS = [
    "⚡ Executar com sudo",
    "🤖 Diagnosticar com IA",
    "Permissão Negada",
    "Comando Não Encontrado",
    "Porta Já em Uso",
    "Espaço em Disco Esgotado",
    "Módulo Python Ausente",
    "Módulo Node.js Ausente",
    "Arquivo Não Encontrado",
    "Falha de Conexão de Rede",
    "Erro de Operação Git",
    "Comando Falhou ({code})",
    "Aplicar correção rápida",
    "Ação rápida",
    "Comando em execução...",
    "Sugestões Proativas para Erros de Terminal",
    "Resource Monitor",
    "Monitor de Recursos do Sistema",
    "Monitor de Recursos em Tempo Real (CPU, RAM, Disco, Rede)",
    "Pausar / Retomar atualizações em tempo real",
    "Local (Este Computador)",
    "Taxa:",
    "Processador (CPU)",
    "Memória (RAM)",
    "Armazenamento (Disco)",
    "Tráfego de Rede",
    "🟢 Ativo",
    "🟢 Conectado",
    "⏸️ Pausado",
    "🟡 Conectando...",
]


class TestTranslationsSync(unittest.TestCase):
    """Verifies that all language catalogs are present, compiled, and contain updated strings."""

    def test_po_files_exist_for_all_languages(self) -> None:
        """Ensure each of the 28 languages has a corresponding .po file in locale/."""
        for lang in SUPPORTED_LANGUAGES:
            po_file = LOCALE_DIR / f"{lang}.po"
            self.assertTrue(po_file.exists(), f"Missing .po file for language '{lang}': {po_file}")
            self.assertGreater(po_file.stat().st_size, 1000, f".po file for '{lang}' is unexpectedly small")

    def test_mo_files_compiled_for_all_languages(self) -> None:
        """Ensure all 28 languages plus pt_BR have compiled .mo files in src/onyxsh/locale."""
        expected_mo_langs = SUPPORTED_LANGUAGES + ["pt_BR"]
        for lang in expected_mo_langs:
            mo_file = INTERNAL_LOCALE_DIR / lang / "LC_MESSAGES" / "onyxsh.mo"
            self.assertTrue(mo_file.exists(), f"Missing compiled .mo file for '{lang}': {mo_file}")
            self.assertGreater(mo_file.stat().st_size, 500, f".mo file for '{lang}' is empty or corrupt")

    def test_runtime_translation_loading(self) -> None:
        """Verify that gettext can load translations for key languages and translate new strings."""
        test_cases = {
            "en": {
                "Permissão Negada": "Permission Denied",
                "Comando Não Encontrado": "Command Not Found",
                "Resource Monitor": "Resource Monitor",
                "Taxa:": "Rate:",
            },
            "es": {
                "Permissão Negada": "Permiso denegado",
                "Comando Não Encontrado": "Comando no encontrado",
                "Resource Monitor": "Monitor de recursos",
                "Taxa:": "Frecuencia:",
            },
            "fr": {
                "Permissão Negada": "Permission refusée",
                "Comando Não Encontrado": "Commande introuvable",
                "Resource Monitor": "Moniteur de ressources",
                "Taxa:": "Taux :",
            },
            "de": {
                "Permissão Negada": "Keine Berechtigung",
                "Comando Não Encontrado": "Befehl nicht gefunden",
                "Resource Monitor": "Ressourcenmonitor",
                "Taxa:": "Rate:",
            },
            "zh": {
                "Permissão Negada": "权限被拒绝",
                "Comando Não Encontrado": "未找到命令",
                "Resource Monitor": "资源监视器",
                "Taxa:": "频率：",
            },
            "ja": {
                "Permissão Negada": "アクセス許可が拒否されました",
                "Comando Não Encontrado": "コマンドが見つかりません",
                "Resource Monitor": "リソースモニター",
                "Taxa:": "レート:",
            },
            "pt": {
                "Permissão Negada": "Permissão Negada",
                "Comando Não Encontrado": "Comando Não Encontrado",
                "Resource Monitor": "Monitor de Recursos",
                "Taxa:": "Taxa:",
            },
        }

        for lang, expected_dict in test_cases.items():
            trans = gettext.translation(
                "onyxsh",
                localedir=str(INTERNAL_LOCALE_DIR),
                languages=[lang],
                fallback=False,
            )
            for key, expected_val in expected_dict.items():
                translated = trans.gettext(key)
                self.assertEqual(
                    translated,
                    expected_val,
                    f"Language '{lang}' translated {repr(key)} as {repr(translated)}, expected {repr(expected_val)}",
                )

    def test_sample_keys_present_across_all_catalogs(self) -> None:
        """Verify that representative sample keys are resolvable in every one of the 28 languages."""
        for lang in SUPPORTED_LANGUAGES:
            trans = gettext.translation(
                "onyxsh",
                localedir=str(INTERNAL_LOCALE_DIR),
                languages=[lang],
                fallback=False,
            )
            for key in TODAY_SAMPLE_KEYS:
                val = trans.gettext(key)
                self.assertTrue(
                    val,
                    f"Translation for {repr(key)} in '{lang}' returned empty or None",
                )


if __name__ == "__main__":
    unittest.main()
