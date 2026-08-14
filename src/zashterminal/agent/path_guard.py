"""Path guard for restricting filesystem access by the AI Agent."""

from __future__ import annotations

import fnmatch
import os
from pathlib import Path
from typing import Optional, Sequence


DEFAULT_READ_DENYLIST: tuple[str, ...] = (
    "~/.ssh",
    "~/.gnupg",
    "~/.aws",
    "~/.kube",
    "~/.docker",
    "~/.netrc",
    "~/.pypirc",
    "~/.npmrc",
    "~/.git-credentials",
    "~/.bash_history",
    "~/.zsh_history",
    "~/.local/share/keyrings",
    "~/.config/gcloud",
    "/etc/shadow",
    "/etc/gshadow",
    "/etc/sudoers",
    "/etc/sudoers.d",
    "/root",
)

DEFAULT_WRITE_DENYLIST: tuple[str, ...] = (
    "~/.bashrc",
    "~/.zshrc",
    "~/.profile",
    "~/.bash_profile",
    "~/.bash_login",
    "~/.bash_logout",
    "~/.config/autostart",
    "~/.config/systemd/user",
    "~/.ssh",
    "~/.gnupg",
    "/etc",
    "/usr",
    "/bin",
    "/sbin",
    "/lib",
    "/lib64",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
    "/root",
    "/var",
)


class PathGuard:
    """Validates and enforces read/write filesystem boundaries for agent actions."""

    def __init__(
        self,
        allowed_roots: Optional[Sequence[str | Path]] = None,
        read_denylist: Optional[Sequence[str]] = None,
        write_denylist: Optional[Sequence[str]] = None,
    ) -> None:
        self.home = Path.home().resolve()

        if allowed_roots is None:
            # Default allowed roots inside user home
            default_roots = [
                self.home / "Documents",
                self.home / "Projects",
                self.home / "Downloads",
                self.home / "scripts",
                self.home / "Desktop",
                self.home / "Workspace",
                self.home,
            ]
            self.allowed_roots = [r.resolve() for r in default_roots if r.exists()]
            if not self.allowed_roots:
                self.allowed_roots = [self.home]
        else:
            self.allowed_roots = [self._resolve_raw(r) for r in allowed_roots]

        self.read_denylist = list(read_denylist or DEFAULT_READ_DENYLIST)
        self.write_denylist = list(write_denylist or DEFAULT_WRITE_DENYLIST)

    def _resolve_raw(self, path: str | Path) -> Path:
        """Expand user and resolve realpath without checking existence."""
        expanded = os.path.expanduser(str(path))
        # Handle symlinks and relative paths
        real = os.path.realpath(expanded)
        return Path(real)

    def _is_env_file(self, filename: str) -> bool:
        """Check if filename matches confidential .env patterns."""
        name = filename.lower()
        if name == ".env" or name.startswith(".env."):
            return True
        if fnmatch.fnmatch(name, "*.env") or fnmatch.fnmatch(name, "*.env.*"):
            return True
        return False

    def _is_under_or_equal(self, target: Path, base: Path) -> bool:
        """Check if target path is base or a descendant of base."""
        try:
            target.relative_to(base)
            return True
        except ValueError:
            return False

    def _matches_denylist(self, real_path: Path, denylist: Sequence[str]) -> bool:
        """Check if resolved path matches any pattern or directory in the denylist."""
        # First check .env pattern
        if self._is_env_file(real_path.name):
            return True

        for entry in denylist:
            resolved_entry = self._resolve_raw(entry)
            # If entry is an exact match or a directory prefix
            if self._is_under_or_equal(real_path, resolved_entry):
                return True

            # Also support glob patterns if specified
            if "*" in entry:
                expanded_entry = os.path.expanduser(entry)
                if fnmatch.fnmatch(str(real_path), expanded_entry):
                    return True

        return False

    def is_within_allowed_roots(self, real_path: Path) -> bool:
        """Check if real_path is within any configured allowed root."""
        return any(self._is_under_or_equal(real_path, root) for root in self.allowed_roots)

    def can_read(self, path: str | Path) -> bool:
        """
        Verify if reading from path is permitted.

        Resolves symlinks to ensure malicious links to sensitive files are blocked.
        """
        if not path:
            return False

        try:
            real_path = self._resolve_raw(path)
        except Exception:
            return False

        # Must not be in read denylist
        if self._matches_denylist(real_path, self.read_denylist):
            return False

        # Allow reading from standard public system paths like /usr/share or /tmp
        # or within allowed roots
        if self.is_within_allowed_roots(real_path):
            return True

        # Common safe read-only system paths (like logs, manuals, /tmp)
        safe_system_read_roots = [
            Path("/tmp").resolve(),
            Path("/var/log").resolve(),
            Path("/usr/share").resolve(),
        ]
        if any(self._is_under_or_equal(real_path, root) for root in safe_system_read_roots):
            return True

        return False

    def can_write(self, path: str | Path) -> bool:
        """
        Verify if writing to path is permitted.

        Resolves symlinks, ensures target is within user workspace and outside denylist.
        """
        if not path:
            return False

        try:
            real_path = self._resolve_raw(path)
        except Exception:
            return False

        # Must not be in write denylist
        if self._matches_denylist(real_path, self.write_denylist):
            return False

        # Must also not be in read denylist (never write into .ssh, .aws, etc.)
        if self._matches_denylist(real_path, self.read_denylist):
            return False

        # Must be strictly within user home or /tmp
        if not self._is_under_or_equal(real_path, self.home) and not self._is_under_or_equal(real_path, Path("/tmp").resolve()):
            return False

        # Must be within allowed roots
        return self.is_within_allowed_roots(real_path)
