"""Policy Engine for evaluating and enforcing execution safety in OnyxSH."""

from __future__ import annotations

import json
import re
from pathlib import Path
from typing import Any, Optional

from ..utils.platform import get_package_manager
from .models import ActionPlan, ActionStep, RiskLevel


READ_ONLY_COMMANDS: frozenset[str] = frozenset({
    "ls", "cat", "df", "du", "head", "tail", "grep", "rg", "find",
    "pwd", "whoami", "uname", "uptime", "free", "ps", "top", "htop",
    "ip", "ifconfig", "ping", "netstat", "ss", "date", "cal", "echo",
    "which", "whereis", "type", "file", "wc", "stat", "tree", "diff",
    "git status", "git log", "git diff", "git show", "git branch",
    "journalctl", "dmesg", "lsblk", "lscpu", "lspci", "lsusb",
})

USER_WRITE_COMMANDS: frozenset[str] = frozenset({
    "touch", "mkdir", "cp", "mv", "rm", "rmdir", "chmod", "chown",
    "tar", "zip", "unzip", "gzip", "gunzip", "sed", "awk", "nano",
    "git add", "git commit", "git checkout", "git switch", "git merge",
    "git stash", "git pull", "git push", "git clone", "python", "python3",
    "pip", "npm", "cargo", "go", "make", "gcc", "clang",
})


def _find_policy_file(filename: str) -> Path:
    """Locate policy JSON files in package data or relative directories."""
    candidates = [
        Path(__file__).resolve().parent.parent / "data" / "policies" / filename,
        Path(__file__).resolve().parent / "data" / "policies" / filename,
        Path("/usr/share/onyxsh/data/policies") / filename,
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    raise FileNotFoundError(f"Policy file '{filename}' not found in candidates: {candidates}")


class PolicyEngine:
    """Validates action plans and commands against security policies and deny patterns."""

    def __init__(
        self,
        deny_patterns_path: Optional[str | Path] = None,
        admin_actions_path: Optional[str | Path] = None,
    ) -> None:
        self.deny_patterns: list[dict[str, str]] = []
        self.admin_actions: dict[str, dict[str, Any]] = {}
        self._compiled_denies: list[tuple[re.Pattern, str]] = []

        self._load_policies(deny_patterns_path, admin_actions_path)

    def _load_policies(
        self,
        deny_patterns_path: Optional[str | Path],
        admin_actions_path: Optional[str | Path],
    ) -> None:
        # 1. Load deny patterns
        if deny_patterns_path is not None:
            p = Path(deny_patterns_path)
        else:
            p = _find_policy_file("deny_patterns.json")

        with open(p, "r", encoding="utf-8") as f:
            self.deny_patterns = json.load(f)

        self._compiled_denies = [
            (re.compile(entry["pattern"], re.IGNORECASE), entry.get("reason", "padrão proibido"))
            for entry in self.deny_patterns
        ]

        # 2. Load admin actions
        if admin_actions_path is not None:
            ap = Path(admin_actions_path)
        else:
            ap = _find_policy_file("admin_actions.json")

        with open(ap, "r", encoding="utf-8") as f:
            self.admin_actions = json.load(f)

    def check_deny_patterns(self, argv: list[str]) -> tuple[bool, Optional[str]]:
        """
        Check if an argv list matches any forbidden pattern.

        Returns (is_denied, reason).
        """
        if not argv:
            return False, None

        cmd_string = " ".join(argv).strip()

        for pattern, reason in self._compiled_denies:
            # Check entire command line string
            if pattern.search(cmd_string):
                return True, reason

            # Also check individual elements
            for element in argv:
                if pattern.search(element):
                    return True, reason

        return False, None

    def match_admin_action(self, argv: list[str]) -> tuple[Optional[str], Optional[dict[str, Any]]]:
        """
        Check if argv matches any registered admin action template.

        Returns (action_id, action_definition) or (None, None).
        """
        if not argv:
            return None, None

        pkg = get_package_manager()
        first_cmd = argv[0]

        for action_id, action_def in self.admin_actions.items():
            template_argv = [a.replace("{pkg}", pkg) for a in action_def.get("argv", [])]
            dry_run_template = (
                [a.replace("{pkg}", pkg) for a in action_def.get("dry_run_argv", [])]
                if action_def.get("dry_run_argv")
                else None
            )

            # Check exact match or base match
            if template_argv and first_cmd == template_argv[0]:
                if argv == template_argv:
                    return action_id, action_def
                if dry_run_template and argv == dry_run_template:
                    return action_id, action_def

                # Parameterized match (e.g. journalctl --vacuum-time=...)
                if len(argv) == len(template_argv):
                    matched = True
                    for actual, expected in zip(argv, template_argv):
                        if "{" in expected and "}" in expected:
                            # Param placeholder check
                            param_name = expected.split("{")[1].split("}")[0]
                            pattern = action_def.get("params", {}).get(param_name, ".*")
                            prefix = expected.split("{")[0]
                            if actual.startswith(prefix):
                                val = actual[len(prefix):]
                                if not re.match(pattern, val):
                                    matched = False
                                    break
                            else:
                                matched = False
                                break
                        elif actual != expected:
                            matched = False
                            break
                    if matched:
                        return action_id, action_def

        return None, None

    def classify(
        self,
        argv: list[str],
        requires_admin: bool = False,
        tool: str = "shell.run",
    ) -> RiskLevel:
        """Classify the risk level of an action."""
        # 1. Deny patterns check (always highest priority)
        is_denied, _ = self.check_deny_patterns(argv)
        if is_denied:
            return RiskLevel.BLOCKED

        # 2. Block direct sudo/su invocations via normal shell tool
        if argv and argv[0] in {"sudo", "su", "pkexec", "doas"}:
            if not requires_admin and tool != "admin.run_action":
                return RiskLevel.BLOCKED

        # 3. Tool specific defaults
        if tool.startswith("fs."):
            if tool in {"fs.list_directory", "fs.metadata", "fs.read_file", "fs.search_text", "fs.disk_usage"}:
                return RiskLevel.READ_ONLY
            if tool in {"fs.write_staged_file", "fs.propose_edit", "fs.create_directory", "fs.move_to_trash"}:
                return RiskLevel.USER_WRITE

        # 4. If admin privilege is requested
        if requires_admin or tool == "admin.run_action":
            if argv and argv[0] in self.admin_actions:
                action_def = self.admin_actions[argv[0]]
                return RiskLevel(action_def.get("risk", RiskLevel.ADMIN))
            action_id, action_def = self.match_admin_action(argv)
            if action_id is not None and action_def is not None:
                return RiskLevel(action_def.get("risk", RiskLevel.ADMIN))
            return RiskLevel.BLOCKED

        if not argv:
            return RiskLevel.READ_ONLY

        # 5. Shell command classification
        base_cmd = argv[0]
        full_2cmd = f"{argv[0]} {argv[1]}" if len(argv) > 1 else base_cmd

        if full_2cmd in READ_ONLY_COMMANDS or base_cmd in READ_ONLY_COMMANDS:
            return RiskLevel.READ_ONLY

        if full_2cmd in USER_WRITE_COMMANDS or base_cmd in USER_WRITE_COMMANDS:
            return RiskLevel.USER_WRITE

        # Default fallback for unknown commands
        return RiskLevel.USER_WRITE

    def evaluate_step(self, step: ActionStep) -> ActionStep:
        """
        Evaluate and sanitize a single ActionStep, setting calculated risk and approval mode.
        """
        is_denied, reason = self.check_deny_patterns(step.argv)
        if is_denied:
            step.risk = int(RiskLevel.BLOCKED)
            step.approval = "blocked"
            step.description = f"[BLOQUEADO: {reason}] {step.description}"
            return step

        computed_risk = self.classify(
            argv=step.argv,
            requires_admin=step.requires_admin,
            tool=step.tool,
        )

        # Overwrite risk with the safest/highest risk
        final_risk = max(int(step.risk), int(computed_risk))
        step.risk = final_risk

        # Determine appropriate approval mode
        if final_risk == int(RiskLevel.BLOCKED):
            step.approval = "blocked"
        elif final_risk == int(RiskLevel.ADMIN):
            step.approval = "polkit"
            step.requires_admin = True
        elif final_risk == int(RiskLevel.USER_WRITE):
            if step.tool in {"fs.propose_edit", "fs.write_staged_file"}:
                step.approval = "diff"
            else:
                step.approval = "click"
        elif final_risk == int(RiskLevel.CRITICAL):
            step.approval = "diff" if step.tool.startswith("fs.") else "click"
        else:  # READ_ONLY
            step.approval = "click"

        return step

    def evaluate_plan(self, plan: ActionPlan) -> ActionPlan:
        """Evaluate each step in an ActionPlan and return the updated plan."""
        evaluated_steps = [self.evaluate_step(step) for step in plan.steps]
        plan.steps = evaluated_steps
        return plan
