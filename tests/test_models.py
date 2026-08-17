"""Tests for agent data models and schema validation."""

import unittest
from onyxsh.agent.models import (
    ActionPlan,
    ActionStep,
    AuditRecord,
    RiskLevel,
    ToolResult,
)


class TestModels(unittest.TestCase):
    def test_risk_level_values(self):
        self.assertEqual(RiskLevel.READ_ONLY, 0)
        self.assertEqual(RiskLevel.USER_WRITE, 1)
        self.assertEqual(RiskLevel.ADMIN, 2)
        self.assertEqual(RiskLevel.CRITICAL, 3)
        self.assertEqual(RiskLevel.BLOCKED, 4)

    def test_action_step_serialization(self):
        step = ActionStep(
            step_id="step_1",
            tool="shell.run",
            argv=["df", "-h"],
            description="Check disk space",
            risk=RiskLevel.READ_ONLY,
            requires_admin=False,
        )
        d = step.to_dict()
        self.assertEqual(d["step_id"], "step_1")
        self.assertEqual(d["tool"], "shell.run")
        self.assertEqual(d["argv"], ["df", "-h"])
        self.assertEqual(d["risk"], 0)

        reconstructed = ActionStep.from_dict(d)
        self.assertEqual(reconstructed.step_id, step.step_id)
        self.assertEqual(reconstructed.tool, step.tool)
        self.assertEqual(reconstructed.argv, step.argv)
        self.assertEqual(reconstructed.risk, RiskLevel.READ_ONLY)

    def test_action_step_invalid_tool(self):
        with self.assertRaises(ValueError):
            ActionStep.from_dict({
                "step_id": "step_1",
                "tool": "invalid.tool.name!",
                "argv": ["ls"],
                "description": "Invalid tool test",
            })

    def test_action_step_unknown_fields_rejected(self):
        with self.assertRaises(ValueError):
            ActionStep.from_dict({
                "step_id": "step_1",
                "tool": "shell.run",
                "argv": ["ls"],
                "description": "Testing unknown field",
                "hack_field": "injected",
            })

    def test_action_plan_serialization(self):
        plan = ActionPlan(
            plan_id="plan_123",
            intent="Check system health",
            summary="Checking system resource usage",
            steps=[
                ActionStep(
                    step_id="s1",
                    tool="shell.run",
                    argv=["uptime"],
                    description="Check uptime",
                    risk=RiskLevel.READ_ONLY,
                )
            ],
            provider="gemini",
        )
        data = plan.to_dict()
        self.assertEqual(data["plan_id"], "plan_123")
        self.assertEqual(len(data["steps"]), 1)

        parsed = ActionPlan.from_dict(data)
        self.assertEqual(parsed.plan_id, plan.plan_id)
        self.assertEqual(len(parsed.steps), 1)
        self.assertEqual(parsed.steps[0].argv, ["uptime"])

    def test_audit_record_serialization(self):
        record = AuditRecord(
            plan_id="plan_abc",
            step_id="step_1",
            tool="shell.run",
            argv=["ls", "-l"],
            risk=RiskLevel.READ_ONLY,
            user_decision="approved",
            result_status="success",
        )
        d = record.to_dict()
        self.assertEqual(d["plan_id"], "plan_abc")
        self.assertEqual(d["result_status"], "success")
        self.assertEqual(d["risk"], 0)

        rec2 = AuditRecord.from_dict(d)
        self.assertEqual(rec2.plan_id, record.plan_id)
        self.assertEqual(rec2.user_decision, "approved")


if __name__ == "__main__":
    unittest.main()
