# onyxsh/data/snippet_resolver.py
"""
Snippet Context Resolver: Parses parameterized command templates with {{variable}}
and {variable} placeholders, resolving system and contextual dynamic variables
(CWD, SSH host, user, git branch, date, time, clipboard, selection).
"""

import datetime
import getpass
import re
import subprocess
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Set

from ..utils.logger import get_logger
from ..utils.translation_utils import _

# Matches {{var}}, {{var=default_value}}, and legacy {var} or {var=default_value}
TEMPLATE_VAR_PATTERN = re.compile(r"\{\{([^}]+)\}\}|\{([a-zA-Z0-9_-]+(?:=[^}]*)?)\}")

# Built-in system variables that are resolved automatically without user input
SYSTEM_VARIABLE_NAMES: Set[str] = {
    "cwd",
    "pwd",
    "host",
    "hostname",
    "user",
    "username",
    "session",
    "date",
    "time",
    "datetime",
    "git_branch",
    "branch_git",
    "clipboard",
    "selection",
    "random_uuid",
}


@dataclass
class SnippetVariable:
    """Represents a variable extracted from a command snippet template."""

    name: str
    default_value: str = ""
    is_system: bool = False
    display_label: str = ""
    raw_token: str = ""

    @property
    def key(self) -> str:
        return self.name.strip().lower()


class SnippetContextResolver:
    """
    Extracts and resolves dynamic context variables and custom parameters in command snippets.
    """

    def __init__(self):
        self.logger = get_logger("onyxsh.data.snippet_resolver")

    def extract_variables(self, template: str) -> List[SnippetVariable]:
        """
        Parses all variable placeholders in the given command template.
        Supports: {{container}}, {{lines=100}}, {host}, {dest=/tmp}
        """
        if not template:
            return []

        variables: List[SnippetVariable] = []
        seen_names: Set[str] = set()

        for match in TEMPLATE_VAR_PATTERN.finditer(template):
            raw_token = match.group(0)
            inner = match.group(1) if match.group(1) is not None else match.group(2)
            if not inner:
                continue

            if "=" in inner:
                var_name, default_val = inner.split("=", 1)
            else:
                var_name, default_val = inner, ""

            var_name = var_name.strip()
            default_val = default_val.strip()
            normalized_key = var_name.lower()

            if normalized_key in seen_names:
                continue

            seen_names.add(normalized_key)
            is_system = normalized_key in SYSTEM_VARIABLE_NAMES

            # Human-readable label
            label = var_name.replace("_", " ").title()

            variables.append(
                SnippetVariable(
                    name=var_name,
                    default_value=default_val,
                    is_system=is_system,
                    display_label=label,
                    raw_token=raw_token,
                )
            )

        return variables

    def get_custom_variables(self, template: str) -> List[SnippetVariable]:
        """Returns only user-customizable variables (excluding system dynamic variables)."""
        return [v for v in self.extract_variables(template) if not v.is_system]

    def resolve_system_context(
        self,
        terminal: Optional[Any] = None,
        clipboard_text: str = "",
        selection_text: str = "",
    ) -> Dict[str, str]:
        """
        Resolves current contextual values (CWD, SSH host, user, git branch, date, etc.).
        """
        context: Dict[str, str] = {}

        # 1. Date & Time
        now = datetime.datetime.now()
        context["date"] = now.strftime("%Y-%m-%d")
        context["time"] = now.strftime("%H:%M:%S")
        context["datetime"] = now.strftime("%Y-%m-%d %H:%M:%S")
        context["random_uuid"] = uuid.uuid4().hex[:8]

        # 2. Local User & Host defaults
        local_user = getpass.getuser()
        context["user"] = local_user
        context["username"] = local_user
        context["host"] = "localhost"
        context["hostname"] = "localhost"
        context["session"] = ""

        # 3. Terminal specific context (CWD & SSH Session)
        cwd_path = str(Path.home())
        if terminal:
            if hasattr(terminal, "get_current_directory_uri"):
                uri = terminal.get_current_directory_uri()
                if uri and uri.startswith("file://"):
                    path = uri[7:]
                    if path.startswith("localhost/"):
                        path = path[9:]
                    elif path.startswith("localhost"):
                        path = path[len("localhost") :]
                    if path:
                        cwd_path = path

            if hasattr(terminal, "onyxsh_session") or hasattr(
                terminal, "zashterminal_session"
            ):
                sess = getattr(
                    terminal,
                    "onyxsh_session",
                    getattr(terminal, "zashterminal_session", None),
                )
                if sess:
                    if getattr(sess, "host", None):
                        context["host"] = sess.host
                        context["hostname"] = sess.host
                    if getattr(sess, "username", None):
                        context["user"] = sess.username
                        context["username"] = sess.username
                    if getattr(sess, "name", None):
                        context["session"] = sess.name

        context["cwd"] = cwd_path
        context["pwd"] = cwd_path

        # 4. Git Branch detection in current CWD
        context["git_branch"] = self._detect_git_branch(cwd_path)
        context["branch_git"] = context["git_branch"]

        # 5. Clipboard & Selection
        context["clipboard"] = clipboard_text or ""
        context["selection"] = selection_text or ""

        return context

    def resolve_template(
        self,
        template: str,
        user_values: Optional[Dict[str, str]] = None,
        terminal: Optional[Any] = None,
        clipboard_text: str = "",
        selection_text: str = "",
    ) -> str:
        """
        Interpolates all placeholders in the template with system context and user provided values.
        """
        if not template:
            return ""

        user_values = user_values or {}
        system_context = self.resolve_system_context(
            terminal=terminal,
            clipboard_text=clipboard_text,
            selection_text=selection_text,
        )

        def _replacer(match: re.Match) -> str:
            inner = match.group(1) if match.group(1) is not None else match.group(2)
            if not inner:
                return match.group(0)

            if "=" in inner:
                var_name, default_val = inner.split("=", 1)
            else:
                var_name, default_val = inner, ""

            var_name = var_name.strip()
            default_val = default_val.strip()
            norm_key = var_name.lower()

            # 1. Check user provided values first
            if var_name in user_values and user_values[var_name] is not None:
                return str(user_values[var_name])
            if norm_key in user_values and user_values[norm_key] is not None:
                return str(user_values[norm_key])

            # 2. Check system dynamic context
            if norm_key in system_context and system_context[norm_key]:
                return str(system_context[norm_key])

            # 3. Fallback to default value or empty
            return default_val

        result = TEMPLATE_VAR_PATTERN.sub(_replacer, template)
        # Normalize repeated whitespaces
        return " ".join(result.split())

    def _detect_git_branch(self, cwd: str) -> str:
        """Safely detects current git branch in CWD."""
        if not cwd or not Path(cwd).exists():
            return "main"

        try:
            head_file = Path(cwd) / ".git" / "HEAD"
            if head_file.exists():
                content = head_file.read_text(encoding="utf-8").strip()
                if content.startswith("ref: refs/heads/"):
                    return content[len("ref: refs/heads/") :]
                return content[:8]

            # Try git rev-parse with fast timeout
            res = subprocess.run(
                ["git", "branch", "--show-current"],
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=0.3,
            )
            if res.returncode == 0 and res.stdout.strip():
                return res.stdout.strip()
        except Exception:
            pass

        return "main"


_global_resolver: Optional[SnippetContextResolver] = None


def get_snippet_resolver() -> SnippetContextResolver:
    """Returns the singleton instance of SnippetContextResolver."""
    global _global_resolver
    if _global_resolver is None:
        _global_resolver = SnippetContextResolver()
    return _global_resolver
