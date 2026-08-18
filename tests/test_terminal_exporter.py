# tests/test_terminal_exporter.py
"""Unit tests for the TerminalExporter module."""

import json
import unittest
from unittest.mock import MagicMock

import gi

gi.require_version("Vte", "3.91")
from gi.repository import Vte

from onyxsh.terminal.exporter import TerminalExporter, get_terminal_exporter


class TestTerminalExporter(unittest.TestCase):
    """Test suite for TerminalExporter."""

    def setUp(self):
        self.exporter = TerminalExporter()
        self.mock_terminal = MagicMock(spec=Vte.Terminal)
        self.mock_terminal.get_has_selection.return_value = False
        self.mock_terminal.get_text_format.side_effect = self._mock_get_text_format
        self.mock_terminal.get_current_directory_uri.return_value = "file:///home/testuser/project"
        self.mock_terminal.get_column_count.return_value = 120
        self.mock_terminal.get_row_count.return_value = 30

    def _mock_get_text_format(self, fmt):
        if fmt == Vte.Format.TEXT:
            return "user@host:~$ ls -la\r\ntotal 4\r\n-rw-r--r-- 1 user user 100 file.txt\r\n"
        elif fmt == Vte.Format.HTML:
            return "<pre><span style=\"color: #00ff00;\">user@host:~$</span> ls -la\n</pre>"
        return ""

    def test_singleton(self):
        """Test get_terminal_exporter returns a valid instance."""
        exp1 = get_terminal_exporter()
        exp2 = get_terminal_exporter()
        self.assertIs(exp1, exp2)
        self.assertIsInstance(exp1, TerminalExporter)

    def test_extract_metadata(self):
        """Test metadata extraction from terminal."""
        meta = self.exporter.extract_metadata(self.mock_terminal)
        self.assertEqual(meta["cols"], 120)
        self.assertEqual(meta["rows"], 30)
        self.assertIn("timestamp", meta)
        self.assertIn("unix_timestamp", meta)
        self.assertIn("cwd", meta)

    def test_export_plain_text(self):
        """Test plain text export format."""
        text = self.exporter.export_plain_text(self.mock_terminal)
        self.assertIn("user@host:~$ ls -la", text)
        self.assertIn("file.txt", text)

    def test_export_log(self):
        """Test log export with header."""
        log_out = self.exporter.export_log(self.mock_terminal)
        self.assertIn("OnyxSH Terminal Output Log", log_out)
        self.assertIn("120x30", log_out)
        self.assertIn("user@host:~$ ls -la", log_out)

    def test_export_markdown(self):
        """Test markdown export structure."""
        md_out = self.exporter.export_markdown(self.mock_terminal)
        self.assertIn("# 🖥️ OnyxSH Terminal Export", md_out)
        self.assertIn("```bash", md_out)
        self.assertIn("user@host:~$ ls -la", md_out)
        self.assertTrue(md_out.endswith("```\n"))

    def test_export_html(self):
        """Test HTML export structure with embedded dark theme."""
        html_out = self.exporter.export_html(self.mock_terminal)
        self.assertIn("<!DOCTYPE html>", html_out)
        self.assertIn("OnyxSH Terminal Export", html_out)
        self.assertIn("copyTerminalText()", html_out)
        self.assertIn("user@host:~$", html_out)

    def test_export_asciinema(self):
        """Test Asciinema v2 export format."""
        cast_out = self.exporter.export_asciinema(self.mock_terminal)
        lines = cast_out.strip().splitlines()
        self.assertGreater(len(lines), 1)

        # First line is valid JSON header
        header = json.loads(lines[0])
        self.assertEqual(header["version"], 2)
        self.assertEqual(header["width"], 120)
        self.assertEqual(header["height"], 30)

        # Subsequent lines are event arrays [time, 'o', text]
        event = json.loads(lines[1])
        self.assertEqual(len(event), 3)
        self.assertEqual(event[1], "o")

    def test_format_content_dispatch(self):
        """Test format_content helper for all format IDs."""
        for fmt, ext, mime in [
            ("txt", ".txt", "text/plain"),
            ("log", ".log", "text/plain"),
            ("md", ".md", "text/markdown"),
            ("html", ".html", "text/html"),
            ("cast", ".cast", "application/json"),
        ]:
            content, def_ext, def_mime = self.exporter.format_content(
                self.mock_terminal, fmt
            )
            self.assertTrue(len(content) > 0)
            self.assertEqual(def_ext, ext)
            self.assertEqual(def_mime, mime)


if __name__ == "__main__":
    unittest.main()
