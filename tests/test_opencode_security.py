# tests/test_opencode_security.py

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

from onyxsh.agent.audit import AuditLogger
from onyxsh.agent.models import ActionPlan, ActionStep, AuditRecord, RiskLevel
from onyxsh.agent.path_guard import PathGuard
from onyxsh.agent.planner import PlanParser
from onyxsh.agent.policy_engine import PolicyEngine
from onyxsh.agent.redactor import redact_secrets


class TestOpenCodeSecurity(unittest.TestCase):
    """Test suite validating Zero-Trust security barriers for OpenCode generated plans."""

    def setUp(self):
        self.path_guard = PathGuard()
        self.policy_engine = PolicyEngine()

    def test_path_guard_blocks_critical_files_from_opencode(self):
        home = Path.home()
        blocked_targets = [
            home / ".ssh" / "id_rsa",
            home / ".bashrc",
            Path("/etc/shadow"),
            Path("/etc/sudoers"),
            home / "project" / ".env",
            home / "project" / ".env.production",
        ]

        for target in blocked_targets:
            allowed = self.path_guard.can_write(target)
            self.assertFalse(
                allowed,
                f"PathGuard deve bloquear escrita em caminho crítico '{target}', mas permitiu.",
            )

    def test_policy_engine_intercepts_dangerous_commands_from_opencode(self):
        dangerous_raw = json.dumps({
            "reply": "Executar limpeza no sistema",
            "commands": [
                "rm -rf /",
                ":(){ :|:& };:",
                "mkfs.ext4 /dev/sda1",
            ],
        })

        plan = PlanParser.parse(dangerous_raw, provider_name="opencode")
        self.assertIsInstance(plan, ActionPlan)

        # Evaluate through policy engine
        evaluated = self.policy_engine.evaluate_plan(plan)
        for step in evaluated.steps:
            self.assertEqual(
                step.approval,
                "blocked",
                f"Comando perigoso '{step.description}' não foi bloqueado pela PolicyEngine!",
            )
            self.assertEqual(step.risk, RiskLevel.BLOCKED)

    def test_redactor_sanitizes_opencode_prompts(self):
        sensitive_prompt = (
            "Deploy application using sk-abcdef12345678901234567890 "
            "and AKIAIOSFODNN7EXAMPLE with password = 'SuperSecretPassword123!'"
        )

        redacted, count = redact_secrets(sensitive_prompt)
        self.assertGreater(count, 0)
        self.assertNotIn("sk-abcdef12345678901234567890", redacted)
        self.assertNotIn("AKIAIOSFODNN7EXAMPLE", redacted)
        self.assertIn("[REDACTED_API_KEY]", redacted)
        self.assertIn("[REDACTED_AWS_KEY]", redacted)

    def test_audit_logger_tracks_opencode_execution(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            log_path = Path(tmp_dir) / "test_audit.jsonl"
            audit_logger = AuditLogger(log_path=log_path)

            record = AuditRecord(
                plan_id="plan_opencode_123",
                step_id="step_1",
                tool="fs.propose_edit",
                argv=["src/main.py", "--- diff"],
                risk=RiskLevel.USER_WRITE,
                user_decision="approved",
                result_status="ok",
            )
            audit_logger.append(record)

            records = audit_logger.get_records()
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0].plan_id, "plan_opencode_123")
            self.assertEqual(records[0].tool, "fs.propose_edit")


if __name__ == "__main__":
    unittest.main()
