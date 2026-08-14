"""Administrative tools for invoking privileged actions safely via Polkit."""

from __future__ import annotations

import json
import os
import re
import shutil
from pathlib import Path
from typing import Any, Optional

from ..utils.platform import get_package_manager
from .models import ToolResult
from .shell_tools import run_argv


ADMIN_HELPER_PATH = Path("/usr/lib/zashterminal/zashterminal-admin-helper")


def _find_admin_actions_file() -> Path:
    candidates = [
        Path(__file__).resolve().parent.parent / "data" / "policies" / "admin_actions.json",
        Path(__file__).resolve().parent / "data" / "policies" / "admin_actions.json",
        Path("/usr/share/zashterminal/data/policies/admin_actions.json"),
    ]
    for c in candidates:
        if c.exists():
            return c
    raise FileNotFoundError("admin_actions.json policy file not found")


class AdminTools:
    """Invokes validated, audited administrative operations."""

    def __init__(self, admin_actions_path: Optional[str | Path] = None) -> None:
        self.actions_file = Path(admin_actions_path) if admin_actions_path else _find_admin_actions_file()
        with open(self.actions_file, "r", encoding="utf-8") as f:
            self.admin_actions: dict[str, dict[str, Any]] = json.load(f)

    def validate_action(
        self,
        action_id: str,
        params: dict[str, Any],
    ) -> tuple[bool, str, list[str], Optional[list[str]]]:
        """
        Validate an action_id and its parameters against admin_actions policy.

        Returns:
            tuple: (is_valid, error_msg, final_argv, final_dry_run_argv)
        """
        if action_id not in self.admin_actions:
            return False, f"Ação administrativa '{action_id}' não autorizada na política.", [], None

        action_def = self.admin_actions[action_id]
        declared_params = action_def.get("params", {})

        # Validate that all required params match regex constraints
        for param_name, param_pattern in declared_params.items():
            if param_name not in params:
                return False, f"Parâmetro obrigatório ausente: '{param_name}'", [], None
            val = str(params[param_name])
            if not re.match(param_pattern, val):
                return (
                    False,
                    f"Valor inválido para o parâmetro '{param_name}': {val} (não confere com regex {param_pattern})",
                    [],
                    None,
                )

        pkg = get_package_manager()

        # Build final argv
        final_argv: list[str] = []
        for token in action_def.get("argv", []):
            formatted_token = token.replace("{pkg}", pkg)
            for p_name, p_val in params.items():
                formatted_token = formatted_token.replace(f"{{{p_name}}}", str(p_val))
            final_argv.append(formatted_token)

        # Build final dry-run argv if available
        final_dry_run_argv: Optional[list[str]] = None
        if action_def.get("dry_run_argv"):
            final_dry_run_argv = []
            for token in action_def["dry_run_argv"]:
                formatted_token = token.replace("{pkg}", pkg)
                for p_name, p_val in params.items():
                    formatted_token = formatted_token.replace(f"{{{p_name}}}", str(p_val))
                final_dry_run_argv.append(formatted_token)

        return True, "", final_argv, final_dry_run_argv

    async def run_action(
        self,
        action_id: str,
        params: Optional[dict[str, Any]] = None,
        dry_run: bool = False,
        timeout_seconds: int = 60,
    ) -> ToolResult:
        """
        Execute an approved administrative action through Polkit.
        """
        params = params or {}
        is_valid, err, argv, dry_run_argv = self.validate_action(action_id, params)
        if not is_valid:
            return ToolResult(
                status="denied",
                stderr=err,
                exit_code=1,
            )

        target_argv = dry_run_argv if (dry_run and dry_run_argv) else argv

        # Check if the installed helper and pkexec are present
        if ADMIN_HELPER_PATH.exists() and shutil.which("pkexec"):
            helper_argv = [
                "pkexec",
                str(ADMIN_HELPER_PATH),
                "--action",
                action_id,
                "--args",
                json.dumps(params),
            ]
            if dry_run:
                helper_argv.append("--dry-run")
            return await run_argv(helper_argv, timeout_seconds=timeout_seconds)

        # Fallback: if dry run or read-only/status action without helper
        return await run_argv(target_argv, timeout_seconds=timeout_seconds)
