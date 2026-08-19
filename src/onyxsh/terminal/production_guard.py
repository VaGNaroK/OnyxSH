# onyxsh/terminal/production_guard.py
"""
Production Guard Engine for OnyxSH.
Analyzes terminal command lines in real time to detect potentially destructive
or high-risk actions executed against production environments.
"""

from __future__ import annotations

import re
import shlex
from dataclasses import dataclass
from typing import List, Optional, Tuple

from ..utils.logger import get_logger
from ..utils.translation_utils import _


@dataclass
class GuardViolation:
    """Represents a dangerous command violation in a production environment."""

    command: str
    category: str
    risk_summary: str
    severity: str = "critical"  # "critical", "high", "warning"


class ProductionGuard:
    """
    Evaluates commands typed in production terminals against destructive rulesets.
    """

    def __init__(self) -> None:
        self.logger = get_logger("onyxsh.terminal.production_guard")
        self._init_rules()

    def _init_rules(self) -> None:
        """Compiles regex rulesets for destructive actions."""
        # 1. File deletion rules
        self._file_delete_patterns = [
            (
                re.compile(r"\brm\s+-[a-zA-Z]*[rR][a-zA-Z]*[fF]|\brm\s+-[a-zA-Z]*[fF][a-zA-Z]*[rR]|\brm\s+.*--recursive.*--force|\brm\s+.*--force.*--recursive", re.IGNORECASE),
                "Recursive force deletion (rm -rf)",
            ),
            (
                re.compile(r"\brm\s+-[a-zA-Z]*r[a-zA-Z]*|\brm\s+--recursive", re.IGNORECASE),
                "Recursive directory deletion (rm -r)",
            ),
            (
                re.compile(r"\bshred\s+-[a-zA-Z]*u|\bwipefs\b", re.IGNORECASE),
                "Permanent file or filesystem wipe (shred/wipefs)",
            ),
        ]

        # 2. Disk & filesystem formatting / raw overwrite
        self._disk_patterns = [
            (
                re.compile(r"\bmkfs(?:\.[a-zA-Z0-9_-]+)?\b", re.IGNORECASE),
                "Filesystem creation / format (mkfs)",
            ),
            (
                re.compile(r"\bdd\s+.*(?:of=/dev/|if=/dev/zero|if=/dev/urandom)", re.IGNORECASE),
                "Raw disk block overwrite (dd of=/dev/...)",
            ),
            (
                re.compile(r"\b(?:fdisk|gdisk|parted|sfdisk)\s+/dev/", re.IGNORECASE),
                "Partition table modification (fdisk/parted)",
            ),
        ]

        # 3. System reboot, halt and shutdown
        self._system_power_patterns = [
            (
                re.compile(r"\b(?:shutdown|reboot|poweroff|halt)\b", re.IGNORECASE),
                "Host shutdown or reboot operation",
            ),
            (
                re.compile(r"\b(?:init|telinit)\s+[06]\b"),
                "Runlevel switch to halt or reboot (init 0/6)",
            ),
        ]

        # 4. Service manipulation
        self._service_patterns = [
            (
                re.compile(r"\bsystemctl\s+(?:stop|disable|mask|isolate)\b", re.IGNORECASE),
                "Critical systemd service stop or disable",
            ),
            (
                re.compile(r"\bservice\s+[\w.-]+\s+(?:stop|restart|reload)\b", re.IGNORECASE),
                "System service stoppage or restart",
            ),
        ]

        # 5. Dangerous Git operations
        self._git_patterns = [
            (
                re.compile(r"\bgit\s+reset\s+--hard\b", re.IGNORECASE),
                "Hard git reset discarding uncommitted changes",
            ),
            (
                re.compile(r"\bgit\s+clean\s+-[a-zA-Z]*[fdx]", re.IGNORECASE),
                "Untracked files deletion (git clean)",
            ),
            (
                re.compile(r"\bgit\s+push\s+.*(?:--force|-f\b|\+)", re.IGNORECASE),
                "Force push overwriting remote Git history",
            ),
        ]

        # 6. Database destruction
        self._db_patterns = [
            (
                re.compile(r"\b(?:DROP|TRUNCATE)\s+(?:DATABASE|SCHEMA|TABLE)\b", re.IGNORECASE),
                "Database drop or truncate operation",
            ),
        ]

    def _strip_wrapper_commands(self, cmd_line: str) -> str:
        """Strips sudo, doas, pkexec, env and common command prefixes."""
        stripped = cmd_line.strip()
        wrappers = ["sudo", "doas", "pkexec", "nohup", "env", "time"]

        changed = True
        while changed:
            changed = False
            for wrapper in wrappers:
                pattern = rf"^{wrapper}\s+(-[a-zA-Z0-9_-]+\s+)*"
                match = re.match(pattern, stripped, re.IGNORECASE)
                if match:
                    stripped = stripped[match.end():].strip()
                    changed = True
                    break

        return stripped

    def _split_subcommands(self, cmd_line: str) -> List[str]:
        """Splits composite command line into individual subcommands (newlines, semicolons, &&, ||, pipes)."""
        parts = re.split(r"[\n;]|&&|\|\||\|", cmd_line)
        return [p.strip() for p in parts if p.strip()]

    def evaluate_command(self, raw_command: str) -> Optional[GuardViolation]:
        """
        Evaluates a shell command string to determine if it is a destructive
        action in a production environment.

        Handles multiline scripts, chained commands (;, &&, ||), pipelines,
        and wrapper commands (sudo, doas, pkexec, etc.).

        Returns a GuardViolation if any part of the command is destructive, otherwise None.
        """
        clean_cmd = raw_command.strip()
        if not clean_cmd:
            return None

        # Build list of candidate expressions to test:
        # 1. Full raw command
        # 2. Wrapper-stripped full command
        # 3. Each individual subcommand (split by newline, semicolon, &&, ||, |)
        # 4. Wrapper-stripped subcommands
        candidates = [clean_cmd, self._strip_wrapper_commands(clean_cmd)]
        for subcmd in self._split_subcommands(clean_cmd):
            if subcmd not in candidates:
                candidates.append(subcmd)
            stripped_sub = self._strip_wrapper_commands(subcmd)
            if stripped_sub not in candidates:
                candidates.append(stripped_sub)

        all_rules: List[Tuple[List[Tuple[re.Pattern, str]], str, str]] = [
            (self._file_delete_patterns, "File System", "critical"),
            (self._disk_patterns, "Storage & Partitions", "critical"),
            (self._system_power_patterns, "System Power", "critical"),
            (self._service_patterns, "Services & Daemons", "high"),
            (self._git_patterns, "Version Control", "high"),
            (self._db_patterns, "Database", "critical"),
        ]

        for pattern_list, category, severity in all_rules:
            for pattern, description in pattern_list:
                for candidate in candidates:
                    if pattern.search(candidate):
                        translated_category = _(category)
                        translated_desc = _(description)
                        self.logger.warning(
                            f"Production Guard intercepted dangerous command: '{candidate}' in '{clean_cmd}' -> {translated_desc}"
                        )
                        return GuardViolation(
                            command=clean_cmd,
                            category=translated_category,
                            risk_summary=translated_desc,
                            severity=severity,
                        )

        return None


_global_production_guard: Optional[ProductionGuard] = None


def get_production_guard() -> ProductionGuard:
    """Returns singleton instance of ProductionGuard."""
    global _global_production_guard
    if _global_production_guard is None:
        _global_production_guard = ProductionGuard()
    return _global_production_guard
