# onyxsh/utils/git_utils.py

"""
Git utilities and security auditor for OnyxSH.
Provides robust local Git repository inspection, staging helpers,
diff extraction, and pre-commit secret leak detection.
Supports both native execution and Flatpak sandbox host spawning.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple
from urllib.parse import unquote, urlparse

from ..agent.redactor import SECRET_PATTERNS
from .logger import get_logger
from .platform import is_flatpak_sandbox

logger = get_logger("onyxsh.utils.git_utils")


def clean_file_uri_to_path(uri: Optional[str | Path]) -> str:
    """Converts a file:// URI or raw path with potential query strings to a clean filesystem path."""
    if not uri:
        return str(Path.cwd())

    raw_str = str(uri).strip()
    if not raw_str:
        return str(Path.cwd())

    try:
        if raw_str.startswith("file://"):
            parsed = urlparse(raw_str)
            raw_path = unquote(parsed.path)
            if parsed.netloc and parsed.netloc not in ("localhost", "127.0.0.1", ""):
                # If host prefix remains
                raw_path = f"/{parsed.netloc}{raw_path}"
            clean = raw_path
        else:
            clean = raw_str

        # Strip any remaining query string attached to URI (e.g. ?__zt_sem__=...)
        if "?" in clean:
            clean = clean.split("?", 1)[0]
        # Strip any localhost prefix if remaining
        if clean.startswith("/localhost/"):
            clean = clean[10:]
        elif clean.startswith("localhost/"):
            clean = clean[9:]

        return str(Path(clean).resolve())
    except Exception:
        return str(Path.cwd())


def run_git_command(
    args: List[str],
    cwd: Path | str,
    timeout: float = 10.0,
) -> subprocess.CompletedProcess:
    """
    Executes a Git command handling both native host and Flatpak sandbox environments.
    Inside Flatpak sandbox, dispatches to the host via flatpak-spawn or host-spawn.
    """
    clean_cwd = clean_file_uri_to_path(cwd)
    path_str = str(clean_cwd)

    # 1. Direct git in PATH (native Linux environment)
    if shutil.which("git") and not is_flatpak_sandbox():
        try:
            return subprocess.run(
                ["git"] + args,
                cwd=path_str,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False,
            )
        except Exception as e:
            logger.debug(f"Direct git execution failed: {e}")

    # 2. Inside Flatpak sandbox or container (run on host via flatpak-spawn / host-spawn)
    if is_flatpak_sandbox() or shutil.which("flatpak-spawn") or shutil.which("host-spawn"):
        if shutil.which("flatpak-spawn"):
            try:
                return subprocess.run(
                    ["flatpak-spawn", "--host", f"--directory={path_str}", "git"] + args,
                    capture_output=True,
                    text=True,
                    timeout=timeout,
                    check=False,
                )
            except Exception as e:
                logger.debug(f"flatpak-spawn git execution failed: {e}")

        if shutil.which("host-spawn"):
            try:
                return subprocess.run(
                    ["host-spawn", f"--directory={path_str}", "git"] + args,
                    capture_output=True,
                    text=True,
                    timeout=timeout,
                    check=False,
                )
            except Exception as e:
                logger.debug(f"host-spawn git execution failed: {e}")

    # 3. Fallback to direct subprocess.run
    return subprocess.run(
        ["git"] + args,
        cwd=path_str,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )


def is_git_repository(cwd: Path | str) -> bool:
    """Checks if the given directory is inside a Git repository work tree."""
    try:
        clean_path = clean_file_uri_to_path(cwd)
        path = Path(clean_path).resolve()
        if not path.exists() or not path.is_dir():
            return False
        res = run_git_command(["rev-parse", "--is-inside-work-tree"], cwd=path, timeout=3.0)
        return res.returncode == 0 and "true" in res.stdout.strip().lower()
    except Exception as e:
        logger.debug(f"Git check failed for {cwd}: {e}")
        return False


def get_repo_root(cwd: Path | str) -> Optional[Path]:
    """Returns the root directory of the Git repository, or None."""
    try:
        clean_path = clean_file_uri_to_path(cwd)
        path = Path(clean_path).resolve()
        if not path.exists() or not path.is_dir():
            return None
        res = run_git_command(["rev-parse", "--show-toplevel"], cwd=path, timeout=3.0)
        if res.returncode == 0 and res.stdout.strip():
            return Path(res.stdout.strip())
    except Exception as e:
        logger.debug(f"Failed to get repo root for {cwd}: {e}")
    return None


def get_git_status(cwd: Path | str) -> Dict[str, Any]:
    """
    Returns structured Git status for the repository.
    Dictionary contains:
      - is_repo: bool
      - repo_root: str
      - branch: str
      - staged: list of dicts {"status": str, "path": str}
      - unstaged: list of dicts {"status": str, "path": str}
      - untracked: list of str
    """
    result: Dict[str, Any] = {
        "is_repo": False,
        "repo_root": "",
        "branch": "",
        "staged": [],
        "unstaged": [],
        "untracked": [],
    }

    repo_root = get_repo_root(cwd)
    if not repo_root:
        return result

    result["is_repo"] = True
    result["repo_root"] = str(repo_root)

    # 1. Get branch name
    try:
        branch_res = run_git_command(["branch", "--show-current"], cwd=repo_root, timeout=3.0)
        branch = branch_res.stdout.strip()
        if not branch:
            # Fallback for detached HEAD
            head_res = run_git_command(["rev-parse", "--short", "HEAD"], cwd=repo_root, timeout=3.0)
            branch = f"HEAD ({head_res.stdout.strip()})" if head_res.returncode == 0 else "main"
        result["branch"] = branch
    except Exception:
        result["branch"] = "main"

    # 2. Get porcelain status
    try:
        status_res = run_git_command(["status", "--porcelain=v1", "-uall"], cwd=repo_root, timeout=5.0)
        if status_res.returncode == 0:
            for line in status_res.stdout.splitlines():
                if len(line) < 3:
                    continue
                index_status = line[0]
                worktree_status = line[1]
                filepath = line[3:].strip()
                if " -> " in filepath:
                    filepath = filepath.split(" -> ")[-1].strip()

                # Untracked
                if index_status == "?" and worktree_status == "?":
                    result["untracked"].append(filepath)
                else:
                    # Staged changes (Index status != space and != '?')
                    if index_status not in (" ", "?"):
                        result["staged"].append(
                            {"status": index_status, "path": filepath}
                        )
                    # Unstaged changes (Worktree status != space and != '?')
                    if worktree_status not in (" ", "?"):
                        result["unstaged"].append(
                            {"status": worktree_status, "path": filepath}
                        )
    except Exception as e:
        logger.error(f"Error parsing git status: {e}")

    return result


def get_git_diff(
    cwd: Path | str, staged: bool = True, max_chars: int = 30000
) -> str:
    """
    Returns the Git diff text.
    If staged is True, returns 'git diff --cached'.
    If staged is False, returns 'git diff' (unstaged).
    """
    repo_root = get_repo_root(cwd)
    if not repo_root:
        return ""

    cmd = ["diff", "--cached"] if staged else ["diff"]
    try:
        diff_res = run_git_command(cmd, cwd=repo_root, timeout=8.0)
        if diff_res.returncode == 0:
            text = diff_res.stdout
            if len(text) > max_chars:
                return text[:max_chars] + f"\n\n[... Diff truncated at {max_chars} characters ...]"
            return text
    except Exception as e:
        logger.error(f"Error fetching git diff (staged={staged}): {e}")
    return ""


LINE_SECRET_PATTERNS: list[tuple[re.Pattern, str]] = [
    (re.compile(r"-----BEGIN [A-Z0-9_\-\s]*PRIVATE KEY"), "REDACTED_PRIVATE_KEY"),
    (re.compile(r"-----BEGIN PGP PRIVATE KEY"), "REDACTED_PGP_PRIVATE_KEY"),
    (re.compile(r"-----BEGIN [A-Z0-9_\-\s]*CERTIFICATE"), "REDACTED_CERTIFICATE"),
]


def audit_diff_for_secrets(diff_text: str) -> List[Dict[str, Any]]:
    """
    Scans added lines in a Git diff for leaked secrets, API keys, tokens, or private keys.
    Returns a list of findings with file, line, secret type, and masked snippet.
    """
    findings: List[Dict[str, Any]] = []
    if not diff_text:
        return findings

    all_patterns = SECRET_PATTERNS + LINE_SECRET_PATTERNS

    current_file = "unknown"
    for line in diff_text.splitlines():
        # Track active file
        if line.startswith("diff --git "):
            parts = line.split()
            if len(parts) >= 4:
                current_file = parts[3].lstrip("b/")
            continue
        elif line.startswith("+++ b/"):
            current_file = line[6:].strip()
            continue

        # Only audit newly added lines (starts with '+', but not '+++')
        if not line.startswith("+") or line.startswith("+++"):
            continue

        content = line[1:].strip()
        if not content:
            continue

        # Check against secret patterns
        for pattern, replacement in all_patterns:
            matches = pattern.findall(content)
            if matches:
                # Mask matched secret for safe display
                sample = str(matches[0])
                if isinstance(matches[0], tuple):
                    sample = " ".join(str(x) for x in matches[0] if x)
                masked_sample = sample[:4] + "****" + sample[-4:] if len(sample) > 8 else "****"

                findings.append(
                    {
                        "file": current_file,
                        "line": content,
                        "type": replacement.strip("[]"),
                        "preview": masked_sample,
                    }
                )
                break  # Don't duplicate warnings for the same line

    return findings


def stage_all_files(cwd: Path | str) -> bool:
    """Runs 'git add -A' in repository."""
    repo_root = get_repo_root(cwd)
    if not repo_root:
        return False
    try:
        res = run_git_command(["add", "-A"], cwd=repo_root, timeout=10.0)
        return res.returncode == 0
    except Exception as e:
        logger.error(f"Error staging all files: {e}")
        return False


def unstage_all_files(cwd: Path | str) -> bool:
    """Runs 'git reset HEAD' or 'git restore --staged .' to unstage all files."""
    repo_root = get_repo_root(cwd)
    if not repo_root:
        return False
    try:
        res = run_git_command(["reset", "HEAD"], cwd=repo_root, timeout=10.0)
        return res.returncode == 0
    except Exception as e:
        logger.error(f"Error unstaging all files: {e}")
        return False


def stage_file(cwd: Path | str, filepath: str) -> bool:
    """Stages a specific file path ('git add <filepath>')."""
    repo_root = get_repo_root(cwd)
    if not repo_root:
        return False
    try:
        res = run_git_command(["add", filepath], cwd=repo_root, timeout=5.0)
        return res.returncode == 0
    except Exception as e:
        logger.error(f"Error staging file {filepath}: {e}")
        return False


def unstage_file(cwd: Path | str, filepath: str) -> bool:
    """Unstages a specific file path ('git restore --staged <filepath>')."""
    repo_root = get_repo_root(cwd)
    if not repo_root:
        return False
    try:
        res = run_git_command(["restore", "--staged", filepath], cwd=repo_root, timeout=5.0)
        return res.returncode == 0
    except Exception as e:
        logger.error(f"Error unstaging file {filepath}: {e}")
        return False


def commit_changes(cwd: Path | str, message: str) -> Tuple[bool, str]:
    """
    Executes 'git commit -m <message>' in the repository.
    Returns (success: bool, output_or_error_message: str).
    """
    repo_root = get_repo_root(cwd)
    if not repo_root:
        return False, "Not inside a Git repository."

    cleaned_msg = message.strip()
    if not cleaned_msg:
        return False, "Commit message cannot be empty."

    try:
        res = run_git_command(["commit", "-m", cleaned_msg], cwd=repo_root, timeout=15.0)
        if res.returncode == 0:
            return True, res.stdout.strip()
        else:
            err_msg = res.stderr.strip() or res.stdout.strip() or f"Exit code {res.returncode}"
            return False, err_msg
    except Exception as e:
        logger.error(f"Git commit failed: {e}")
        return False, str(e)
