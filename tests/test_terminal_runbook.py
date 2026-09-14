"""Unit tests for OnyxSH Terminal Runbook (Caderno de Bordo) engine and exporters."""

import unittest
from unittest.mock import MagicMock

from onyxsh.terminal.exporter import TerminalExporter, get_terminal_exporter
from onyxsh.terminal.runbook import (
    RunbookData,
    RunbookGenerator,
    RunbookMetadata,
    RunbookStep,
    get_runbook_generator,
)


class TestTerminalRunbook(unittest.TestCase):
    """Tests for Runbook data structures, generator, and markdown/HTML/log formatters."""

    def setUp(self) -> None:
        self.generator = RunbookGenerator()
        self.exporter = TerminalExporter()

    def test_runbook_step_properties(self) -> None:
        """Verifies RunbookStep properties and defaults."""
        step_ok = RunbookStep(command="ls -la", exit_code=0)
        self.assertTrue(step_ok.is_success)
        self.assertTrue(step_ok.included)
        self.assertEqual(step_ok.annotation, "")

        step_fail = RunbookStep(command="make build", exit_code=2)
        self.assertFalse(step_fail.is_success)

        step_none = RunbookStep(command="echo hi", exit_code=None)
        self.assertTrue(step_none.is_success)

    def test_sanitize_text_strips_ansi(self) -> None:
        """Verifies ANSI escape sequences are completely stripped from outputs."""
        ansi_str = "\x1b[31;1mError:\x1b[0m File not found\r\n\x1b[32mOK\x1b[0m"
        clean = self.generator._sanitize_text(ansi_str)
        self.assertNotIn("\x1b", clean)
        self.assertEqual(clean, "Error: File not found\nOK")

    def test_parse_fallback_steps_from_scrollback(self) -> None:
        """Verifies parsing of steps from raw text when prompts are present."""
        sample_scrollback = (
            "user@host:~$ uname -a\n"
            "Linux workstation 6.8.0 #1 SMP\n"
            "user@host:~$ df -h\n"
            "/dev/sda1 100G 20G 80G 20% /\n"
        )
        steps = self.generator._parse_fallback_steps(sample_scrollback)
        self.assertEqual(len(steps), 2)
        self.assertEqual(steps[0].command, "uname -a")
        self.assertIn("Linux workstation", steps[0].output)
        self.assertEqual(steps[1].command, "df -h")
        self.assertIn("/dev/sda1", steps[1].output)

    def test_parse_fallback_steps_empty_and_plain(self) -> None:
        """Verifies graceful handling of empty or un-prompted text."""
        empty_steps = self.generator._parse_fallback_steps("")
        self.assertEqual(len(empty_steps), 0)

        plain_text = "Some unformatted log output without shell prompts"
        plain_steps = self.generator._parse_fallback_steps(plain_text)
        self.assertEqual(len(plain_steps), 1)
        self.assertIn("unformatted log output", plain_steps[0].output)

    def test_render_markdown_full_report(self) -> None:
        """Verifies Markdown rendering contains all metadata, annotations, callouts and steps."""
        meta = RunbookMetadata(
            title="Deploy de Produção v1.2",
            author="DevOps Team",
            objective="Atualizar serviços e migrar banco de dados.",
            general_notes="Backup realizado antes do início.",
            conclusion="Deploy finalizado sem indisponibilidade.",
            status="Concluído",
            session_name="Servidor Web",
            host="srv-prod-01",
            date_str="2026-09-13 21:00:00",
        )
        steps = [
            RunbookStep(
                command="git pull origin main",
                output="Updating a1b2c3d..e4f5g6h\nFast-forward",
                exit_code=0,
                duration_str="1.2s",
                timestamp_str="21:01:00",
                annotation="Código atualizado com a última versão estável.",
                included=True,
            ),
            RunbookStep(
                command="python3 manage.py migrate",
                output="Operations to perform:\n  Apply all migrations\nRunning migrations:\n  Applying 0002_add_index... OK",
                exit_code=0,
                duration_str="3.5s",
                timestamp_str="21:02:00",
                annotation="Migrações de esquema aplicadas com sucesso.",
                included=True,
            ),
            RunbookStep(
                command="echo 'skip this typo'",
                output="skip this typo",
                exit_code=0,
                included=False,  # Excluded step!
            ),
        ]
        runbook = RunbookData(metadata=meta, steps=steps)

        md = self.generator.render_markdown(runbook, collapse_long_outputs=True)

        # Header and meta
        self.assertIn("# 📘 Deploy de Produção v1.2", md)
        self.assertIn("**Status:** `Concluído`", md)
        self.assertIn("**Operador / Autor:** `DevOps Team`", md)
        self.assertIn("**Host:** `srv-prod-01`", md)
        self.assertIn("Atualizar serviços e migrar banco de dados.", md)
        self.assertIn("Backup realizado antes do início.", md)

        # Steps
        self.assertIn("1. `git pull origin main` 🟢", md)
        self.assertIn("💬 **Anotação:** Código atualizado com a última versão estável.", md)
        self.assertIn("2. `python3 manage.py migrate` 🟢", md)
        self.assertIn("💬 **Anotação:** Migrações de esquema aplicadas com sucesso.", md)

        # Excluded step must NOT appear
        self.assertNotIn("skip this typo", md)

        # Conclusion
        self.assertIn("Deploy finalizado sem indisponibilidade.", md)

    def test_render_markdown_collapsible_long_output(self) -> None:
        """Verifies long outputs are wrapped in <details> when collapse_long_outputs is True."""
        long_output = "\n".join([f"Line {i}" for i in range(30)])
        steps = [
            RunbookStep(
                command="systemctl status",
                output=long_output,
                exit_code=0,
                included=True,
            )
        ]
        runbook = RunbookData(metadata=RunbookMetadata(title="Teste"), steps=steps)

        # With collapse enabled
        md_collapsed = self.generator.render_markdown(
            runbook, collapse_long_outputs=True, max_preview_lines=10
        )
        self.assertIn("<details>", md_collapsed)
        self.assertIn("</details>", md_collapsed)
        self.assertIn("30 linhas", md_collapsed)

        # With collapse disabled
        md_expanded = self.generator.render_markdown(
            runbook, collapse_long_outputs=False
        )
        self.assertNotIn("<details>", md_expanded)
        self.assertIn("Line 29", md_expanded)

    def test_render_html_report_and_print_styles(self) -> None:
        """Verifies standalone HTML report generation and XSS escaping."""
        meta = RunbookMetadata(
            title="Relatório <script>alert(1)</script>",
            author="Admin & Operator",
            host="192.168.1.100",
            status="Concluído",
        )
        steps = [
            RunbookStep(
                command="echo '<b>safe test</b>'",
                output="<b>safe test</b>",
                exit_code=0,
                annotation="Validado com sucesso.",
                included=True,
            )
        ]
        runbook = RunbookData(metadata=meta, steps=steps)

        html_out = self.generator.render_html(runbook)

        # Structure
        self.assertIn("<!DOCTYPE html>", html_out)
        self.assertIn("<title>", html_out)
        self.assertIn("@media print", html_out)

        # XSS sanitization check
        self.assertNotIn("<script>alert(1)</script>", html_out)
        self.assertIn("&lt;script&gt;alert(1)&lt;/script&gt;", html_out)
        self.assertIn("&lt;b&gt;safe test&lt;/b&gt;", html_out)
        self.assertIn("Admin &amp; Operator", html_out)
        self.assertIn("Validado com sucesso.", html_out)

    def test_render_log_structured_text(self) -> None:
        """Verifies structured plain text log rendering."""
        meta = RunbookMetadata(
            title="Log de Manutenção",
            author="Operador",
            host="localhost",
            status="OK",
        )
        steps = [
            RunbookStep(
                command="uptime",
                output="up 4 days, 2 users",
                exit_code=0,
                duration_str="50ms",
                annotation="Sistema estável.",
                included=True,
            )
        ]
        runbook = RunbookData(metadata=meta, steps=steps)
        log_out = self.generator.render_log(runbook)

        self.assertIn("Log de Manutenção [OK]", log_out)
        self.assertIn("$ uptime [exit 0] (50ms)", log_out)
        self.assertIn("Nota: Sistema estável.", log_out)
        self.assertIn("up 4 days, 2 users", log_out)

    def test_exporter_runbook_integration(self) -> None:
        """Verifies TerminalExporter format_content integration with runbook formats."""
        mock_terminal = MagicMock()
        mock_terminal.get_has_selection.return_value = False
        mock_terminal.get_text_format.return_value = "user@host:~$ echo hello\nhello\n"
        mock_terminal.get_column_count.return_value = 80
        mock_terminal.get_row_count.return_value = 24

        meta = RunbookMetadata(title="Integration Test")
        steps = [RunbookStep(command="echo hello", output="hello", exit_code=0, included=True)]
        rb_data = RunbookData(metadata=meta, steps=steps)

        # Test runbook_md
        content, ext, mime = self.exporter.format_content(
            mock_terminal, "runbook_md", runbook_data=rb_data
        )
        self.assertEqual(ext, ".md")
        self.assertEqual(mime, "text/markdown")
        self.assertIn("Integration Test", content)
        self.assertIn("echo hello", content)

        # Test runbook_html
        content_html, ext_html, mime_html = self.exporter.format_content(
            mock_terminal, "runbook_html", runbook_data=rb_data
        )
        self.assertEqual(ext_html, ".html")
        self.assertEqual(mime_html, "text/html")
        self.assertIn("<!DOCTYPE html>", content_html)
        self.assertIn("Integration Test", content_html)

        # Test runbook_log
        content_log, ext_log, mime_log = self.exporter.format_content(
            mock_terminal, "runbook_log", runbook_data=rb_data
        )
        self.assertEqual(ext_log, ".log")
        self.assertEqual(mime_log, "text/plain")
        self.assertIn("Integration Test", content_log)

    def test_singletons(self) -> None:
        """Verifies singleton getters return valid generator instances."""
        gen1 = get_runbook_generator()
        gen2 = get_runbook_generator()
        self.assertIs(gen1, gen2)

        exp1 = get_terminal_exporter()
        exp2 = get_terminal_exporter()
        self.assertIs(exp1, exp2)


if __name__ == "__main__":
    unittest.main()
