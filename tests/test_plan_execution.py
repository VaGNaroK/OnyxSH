# tests/test_plan_execution.py
"""Unit tests for Plan before Execute mode, StepStatus, and PlanParser fallbacks."""

import unittest
from onyxsh.agent.models import (
    ActionPlan,
    ActionStep,
    RiskLevel,
    StepStatus,
    VALID_STEP_STATUSES,
)
from onyxsh.agent.planner import PlanParser


class TestPlanExecution(unittest.TestCase):
    def test_step_status_constants(self):
        """StepStatus constants and validation set should contain all required lifecycle states."""
        self.assertIn(StepStatus.PENDING, VALID_STEP_STATUSES)
        self.assertIn(StepStatus.RUNNING, VALID_STEP_STATUSES)
        self.assertIn(StepStatus.COMPLETED, VALID_STEP_STATUSES)
        self.assertIn(StepStatus.FAILED, VALID_STEP_STATUSES)
        self.assertIn(StepStatus.SKIPPED, VALID_STEP_STATUSES)

    def test_action_step_status_defaults(self):
        """ActionStep should default to pending status and selected=True."""
        step = ActionStep(
            step_id="step_1",
            tool="shell.run",
            argv=["df", "-h"],
            description="Check disk space",
            risk=RiskLevel.READ_ONLY,
        )
        self.assertEqual(step.status, StepStatus.PENDING)
        self.assertTrue(step.selected)

        step_dict = step.to_dict()
        self.assertEqual(step_dict["status"], "pending")
        self.assertTrue(step_dict["selected"])

    def test_plan_parser_structured_json(self):
        """PlanParser should parse structured ActionPlan JSON with multiple steps."""
        json_text = """
        {
            "plan_id": "plan_test_123",
            "intent": "Disk cleanup",
            "summary": "Check disk space and clean cache",
            "steps": [
                {
                    "step_id": "step_1",
                    "tool": "shell.run",
                    "argv": ["df", "-h"],
                    "description": "Check disk",
                    "risk": 0
                },
                {
                    "step_id": "step_2",
                    "tool": "shell.run",
                    "argv": ["rm", "-rf", "/tmp/junk"],
                    "description": "Clean tmp",
                    "risk": 1
                }
            ]
        }
        """
        plan = PlanParser.parse(json_text, provider_name="test_provider")
        self.assertIsInstance(plan, ActionPlan)
        self.assertEqual(len(plan.steps), 2)
        self.assertEqual(plan.steps[0].risk, 0)
        self.assertEqual(plan.steps[1].risk, 1)

    def test_plan_parser_markdown_code_block_fallback(self):
        """PlanParser should extract multi-step plans from plain markdown code blocks for local LLMs."""
        markdown_text = """
        Aqui estão os comandos para verificar e reiniciar o serviço:
        ```bash
        systemctl status nginx
        systemctl restart nginx
        ```
        """
        plan = PlanParser.parse(markdown_text, provider_name="ollama")
        self.assertIsInstance(plan, ActionPlan)
        self.assertEqual(len(plan.steps), 2)
        self.assertEqual(plan.steps[0].argv, ["systemctl", "status", "nginx"])
        self.assertEqual(plan.steps[1].argv, ["systemctl", "restart", "nginx"])

    def test_read_only_step_filtering(self):
        """Verify diagnostic filtering correctly separates read-only steps from modifying steps."""
        steps = [
            ActionStep("step_1", "shell.run", ["uptime"], "Check uptime", 0),
            ActionStep("step_2", "shell.run", ["cat", "/etc/os-release"], "Check OS", 0),
            ActionStep("step_3", "shell.run", ["systemctl", "restart", "app"], "Restart app", 2),
        ]
        diag_steps = [s for s in steps if s.risk == 0]
        mod_steps = [s for s in steps if s.risk > 0]
        self.assertEqual(len(diag_steps), 2)
        self.assertEqual(len(mod_steps), 1)

    def test_ai_assistant_parse_payload_action_plan(self):
        """ai_assistant._parse_assistant_payload should parse ActionPlan JSON with summary and steps."""
        from onyxsh.terminal.ai_assistant import TerminalAiAssistant
        from unittest.mock import MagicMock

        mock_window = MagicMock()
        mock_settings = MagicMock()
        mock_term_mgr = MagicMock()
        assistant = TerminalAiAssistant(mock_window, mock_settings, mock_term_mgr)

        payload_json = """
        ```json
        {
            "plan_id": "p1",
            "intent": "system check",
            "summary": "Verificando o sistema...",
            "steps": [
                {"step_id": "s1", "tool": "shell.run", "argv": ["df", "-h"], "risk": 0},
                {"step_id": "s2", "tool": "shell.run", "argv": ["free", "-h"], "risk": 0}
            ]
        }
        ```
        """
        reply, commands, _ = assistant._parse_assistant_payload(payload_json)
        self.assertEqual(reply, "Verificando o sistema...")
        self.assertEqual(len(commands), 2)
        self.assertEqual(commands[0]["command"], "df -h")
        self.assertEqual(commands[1]["command"], "free -h")

    def test_ai_assistant_parse_payload_multiline_markdown(self):
        """ai_assistant._parse_assistant_payload should extract multiple lines of commands from markdown code blocks."""
        from onyxsh.terminal.ai_assistant import TerminalAiAssistant
        from unittest.mock import MagicMock

        mock_window = MagicMock()
        mock_settings = MagicMock()
        mock_term_mgr = MagicMock()
        assistant = TerminalAiAssistant(mock_window, mock_settings, mock_term_mgr)

        text_with_code = """
        Execute estes passos no terminal:
        ```bash
        df -h
        sudo du -sh /var/log/* | sort -rh | head -n 5
        free -h
        ```
        """
        reply, commands, _ = assistant._parse_assistant_payload(text_with_code)
        self.assertIn("Execute estes passos", reply)
        self.assertEqual(len(commands), 3)
        self.assertEqual(commands[0]["command"], "df -h")
        self.assertEqual(commands[1]["command"], "sudo du -sh /var/log/* | sort -rh | head -n 5")
        self.assertEqual(commands[2]["command"], "free -h")

    def test_ai_assistant_parse_payload_json_with_comments(self):
        """ai_assistant should parse JSON containing // comments produced by local LLMs."""
        from onyxsh.terminal.ai_assistant import TerminalAiAssistant
        from unittest.mock import MagicMock

        mock_window = MagicMock()
        mock_settings = MagicMock()
        mock_term_mgr = MagicMock()
        assistant = TerminalAiAssistant(mock_window, mock_settings, mock_term_mgr)

        json_with_comments = """
        ```json
        {
          "reply": "Para verificar o espaço em disco do sistema, listar os maiores arquivos em `/var/log` e mostrar a memória livre, siga as instruções abaixo:",
          "commands": [
            "df -h", // Verifica o espaço em disco do sistema
            "du -sh /var/log/* | sort -rh | head -n 10", // Lista os maiores arquivos em /var/log
            "free -h" // Mostra a memória livre
          ]
        }
        ```
        """
        reply, commands, _ = assistant._parse_assistant_payload(json_with_comments)
        self.assertIn("Para verificar o espaço em disco", reply)
        self.assertEqual(len(commands), 3)
        self.assertEqual(commands[0]["command"], "df -h")
        self.assertEqual(commands[1]["command"], "du -sh /var/log/* | sort -rh | head -n 10")
        self.assertEqual(commands[2]["command"], "free -h")


if __name__ == "__main__":
    unittest.main()
