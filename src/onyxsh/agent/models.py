"""Data models and schemas for OnyxSH Secure Agent Mode."""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from enum import IntEnum
from typing import Any, Optional


class RiskLevel(IntEnum):
    """Risk stratification levels for agent actions."""

    READ_ONLY = 0   # ls, df, du, journalctl --disk-usage
    USER_WRITE = 1  # Create/edit files in user home
    ADMIN = 2       # Named administrative actions (Polkit)
    CRITICAL = 3    # Scoped deletions, service changes
    BLOCKED = 4     # Prohibited / dangerous actions (never execute)


VALID_APPROVALS = {"click", "diff", "polkit", "blocked"}
VALID_TOOL_STATUSES = {"ok", "error", "denied", "timeout"}
VALID_USER_DECISIONS = {"approved", "denied", "dry_run"}


def _check_allowed_keys(data: dict[str, Any], allowed_keys: set[str], class_name: str) -> None:
    unknown = set(data.keys()) - allowed_keys
    if unknown:
        raise ValueError(f"Unknown fields in {class_name}: {sorted(unknown)}")


@dataclass
class ActionStep:
    """A single executable step within an ActionPlan."""

    step_id: str
    tool: str
    argv: list[str]
    description: str
    risk: int
    requires_admin: bool = False
    dry_run_argv: Optional[list[str]] = None
    working_directory: Optional[str] = None
    timeout_seconds: int = 30
    approval: str = "click"

    def __post_init__(self) -> None:
        self._validate()

    def _validate(self) -> None:
        if not isinstance(self.step_id, str) or not self.step_id.strip():
            raise ValueError("step_id must be a non-empty string")
        if not isinstance(self.tool, str) or not self.tool.strip():
            raise ValueError("tool must be a non-empty string")
        if not isinstance(self.argv, list) or not all(isinstance(a, str) for a in self.argv):
            raise ValueError("argv must be a list of strings")
        if not isinstance(self.description, str):
            raise ValueError("description must be a string")

        if not isinstance(self.risk, int) or isinstance(self.risk, bool) or self.risk < 0 or self.risk > 4:
            raise ValueError(f"risk must be an integer between 0 and 4, got {self.risk!r}")

        if not isinstance(self.requires_admin, bool):
            raise ValueError("requires_admin must be a boolean")

        if self.dry_run_argv is not None:
            if not isinstance(self.dry_run_argv, list) or not all(isinstance(a, str) for a in self.dry_run_argv):
                raise ValueError("dry_run_argv must be None or a list of strings")

        if self.working_directory is not None and not isinstance(self.working_directory, str):
            raise ValueError("working_directory must be None or a string")

        if not isinstance(self.timeout_seconds, int) or isinstance(self.timeout_seconds, bool) or self.timeout_seconds <= 0:
            raise ValueError("timeout_seconds must be a positive integer")

        if self.approval not in VALID_APPROVALS:
            raise ValueError(f"approval must be one of {VALID_APPROVALS}, got {self.approval!r}")

    def to_dict(self) -> dict[str, Any]:
        """Convert step to dictionary format."""
        return {
            "step_id": self.step_id,
            "tool": self.tool,
            "argv": list(self.argv),
            "description": self.description,
            "risk": int(self.risk),
            "requires_admin": self.requires_admin,
            "dry_run_argv": list(self.dry_run_argv) if self.dry_run_argv is not None else None,
            "working_directory": self.working_directory,
            "timeout_seconds": self.timeout_seconds,
            "approval": self.approval,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ActionStep:
        """Create ActionStep from dictionary with strict validation."""
        if not isinstance(data, dict):
            raise TypeError(f"Expected dict for ActionStep, got {type(data).__name__}")

        allowed = {
            "step_id",
            "tool",
            "argv",
            "description",
            "risk",
            "requires_admin",
            "dry_run_argv",
            "working_directory",
            "timeout_seconds",
            "approval",
        }
        _check_allowed_keys(data, allowed, "ActionStep")

        required = {"step_id", "tool", "argv", "description", "risk"}
        missing = required - set(data.keys())
        if missing:
            raise ValueError(f"Missing required fields in ActionStep: {sorted(missing)}")

        return cls(
            step_id=data["step_id"],
            tool=data["tool"],
            argv=data["argv"],
            description=data["description"],
            risk=data["risk"],
            requires_admin=data.get("requires_admin", False),
            dry_run_argv=data.get("dry_run_argv"),
            working_directory=data.get("working_directory"),
            timeout_seconds=data.get("timeout_seconds", 30),
            approval=data.get("approval", "click"),
        )


@dataclass
class ActionPlan:
    """Structured plan produced by LLM representing an action sequence."""

    plan_id: str
    intent: str
    summary: str
    steps: list[ActionStep] = field(default_factory=list)
    provider: str = ""

    def __post_init__(self) -> None:
        self._validate()

    def _validate(self) -> None:
        if not isinstance(self.plan_id, str) or not self.plan_id.strip():
            raise ValueError("plan_id must be a non-empty string")
        if not isinstance(self.intent, str):
            raise ValueError("intent must be a string")
        if not isinstance(self.summary, str):
            raise ValueError("summary must be a string")
        if not isinstance(self.steps, list) or not all(isinstance(s, ActionStep) for s in self.steps):
            raise ValueError("steps must be a list of ActionStep instances")
        if not isinstance(self.provider, str):
            raise ValueError("provider must be a string")

    def to_dict(self) -> dict[str, Any]:
        """Convert plan to dictionary format."""
        return {
            "plan_id": self.plan_id,
            "intent": self.intent,
            "summary": self.summary,
            "steps": [s.to_dict() for s in self.steps],
            "provider": self.provider,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ActionPlan:
        """Create ActionPlan from dictionary with strict validation."""
        if not isinstance(data, dict):
            raise TypeError(f"Expected dict for ActionPlan, got {type(data).__name__}")

        allowed = {"plan_id", "intent", "summary", "steps", "provider"}
        _check_allowed_keys(data, allowed, "ActionPlan")

        required = {"plan_id", "intent", "summary"}
        missing = required - set(data.keys())
        if missing:
            raise ValueError(f"Missing required fields in ActionPlan: {sorted(missing)}")

        raw_steps = data.get("steps", [])
        if not isinstance(raw_steps, list):
            raise ValueError("steps must be a list")

        parsed_steps = []
        for i, step_data in enumerate(raw_steps):
            if isinstance(step_data, ActionStep):
                parsed_steps.append(step_data)
            elif isinstance(step_data, dict):
                parsed_steps.append(ActionStep.from_dict(step_data))
            else:
                raise TypeError(f"Step {i} must be dict or ActionStep, got {type(step_data).__name__}")

        return cls(
            plan_id=data["plan_id"],
            intent=data["intent"],
            summary=data["summary"],
            steps=parsed_steps,
            provider=data.get("provider", ""),
        )


@dataclass
class ToolResult:
    """Outcome of invoking a tool action."""

    status: str
    stdout: str = ""
    stderr: str = ""
    exit_code: int = 0
    truncated: bool = False
    secrets_redacted: int = 0

    def __post_init__(self) -> None:
        self._validate()

    def _validate(self) -> None:
        if self.status not in VALID_TOOL_STATUSES:
            raise ValueError(f"status must be one of {VALID_TOOL_STATUSES}, got {self.status!r}")
        if not isinstance(self.stdout, str):
            raise ValueError("stdout must be a string")
        if not isinstance(self.stderr, str):
            raise ValueError("stderr must be a string")
        if not isinstance(self.exit_code, int) or isinstance(self.exit_code, bool):
            raise ValueError("exit_code must be an integer")
        if not isinstance(self.truncated, bool):
            raise ValueError("truncated must be a boolean")
        if not isinstance(self.secrets_redacted, int) or isinstance(self.secrets_redacted, bool) or self.secrets_redacted < 0:
            raise ValueError("secrets_redacted must be a non-negative integer")

    @property
    def success(self) -> bool:
        """Convenience property indicating success status."""
        return self.status in {"ok", "success"}

    @property
    def error(self) -> str:
        """Convenience property alias for stderr."""
        return self.stderr

    @property
    def returncode(self) -> int:
        """Convenience property alias for exit_code."""
        return self.exit_code

    def to_dict(self) -> dict[str, Any]:
        """Convert ToolResult to dictionary format."""
        return {
            "status": self.status,
            "stdout": self.stdout,
            "stderr": self.stderr,
            "exit_code": self.exit_code,
            "truncated": self.truncated,
            "secrets_redacted": self.secrets_redacted,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> ToolResult:
        """Create ToolResult from dictionary with validation."""
        if not isinstance(data, dict):
            raise TypeError(f"Expected dict for ToolResult, got {type(data).__name__}")

        allowed = {"status", "stdout", "stderr", "exit_code", "truncated", "secrets_redacted"}
        _check_allowed_keys(data, allowed, "ToolResult")

        if "status" not in data:
            raise ValueError("Missing required field 'status' in ToolResult")

        return cls(
            status=data["status"],
            stdout=data.get("stdout", ""),
            stderr=data.get("stderr", ""),
            exit_code=data.get("exit_code", 0),
            truncated=data.get("truncated", False),
            secrets_redacted=data.get("secrets_redacted", 0),
        )


@dataclass
class AuditRecord:
    """Audit log entry recorded for every action step decision and execution."""

    plan_id: str
    step_id: str
    tool: str
    argv: list[str]
    risk: int
    user_decision: str
    result_status: str
    timestamp: str = ""

    def __post_init__(self) -> None:
        if not self.timestamp:
            from datetime import datetime, timezone
            self.timestamp = datetime.now(timezone.utc).isoformat()
        self._validate()

    def _validate(self) -> None:
        if not isinstance(self.timestamp, str) or not self.timestamp.strip():
            raise ValueError("timestamp must be a non-empty string")
        if not isinstance(self.plan_id, str) or not self.plan_id.strip():
            raise ValueError("plan_id must be a non-empty string")
        if not isinstance(self.step_id, str) or not self.step_id.strip():
            raise ValueError("step_id must be a non-empty string")
        if not isinstance(self.tool, str) or not self.tool.strip():
            raise ValueError("tool must be a non-empty string")
        if not isinstance(self.argv, list) or not all(isinstance(a, str) for a in self.argv):
            raise ValueError("argv must be a list of strings")
        if not isinstance(self.risk, int) or isinstance(self.risk, bool) or self.risk < 0 or self.risk > 4:
            raise ValueError(f"risk must be an integer between 0 and 4, got {self.risk!r}")
        if self.user_decision not in VALID_USER_DECISIONS:
            raise ValueError(f"user_decision must be one of {VALID_USER_DECISIONS}, got {self.user_decision!r}")
        if not isinstance(self.result_status, str):
            raise ValueError("result_status must be a string")

    def to_dict(self) -> dict[str, Any]:
        """Convert AuditRecord to dictionary format."""
        return {
            "timestamp": self.timestamp,
            "plan_id": self.plan_id,
            "step_id": self.step_id,
            "tool": self.tool,
            "argv": list(self.argv),
            "risk": int(self.risk),
            "user_decision": self.user_decision,
            "result_status": self.result_status,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> AuditRecord:
        """Create AuditRecord from dictionary with validation."""
        if not isinstance(data, dict):
            raise TypeError(f"Expected dict for AuditRecord, got {type(data).__name__}")

        allowed = {
            "timestamp",
            "plan_id",
            "step_id",
            "tool",
            "argv",
            "risk",
            "user_decision",
            "result_status",
        }
        _check_allowed_keys(data, allowed, "AuditRecord")

        required = {
            "timestamp",
            "plan_id",
            "step_id",
            "tool",
            "argv",
            "risk",
            "user_decision",
            "result_status",
        }
        missing = required - set(data.keys())
        if missing:
            raise ValueError(f"Missing required fields in AuditRecord: {sorted(missing)}")

        return cls(
            timestamp=data["timestamp"],
            plan_id=data["plan_id"],
            step_id=data["step_id"],
            tool=data["tool"],
            argv=data["argv"],
            risk=data["risk"],
            user_decision=data["user_decision"],
            result_status=data["result_status"],
        )
