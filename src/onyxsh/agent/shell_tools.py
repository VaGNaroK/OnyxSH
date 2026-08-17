"""Shell execution tools for running argument vectors safely and asynchronously."""

from __future__ import annotations

import asyncio
import os
import shutil
from pathlib import Path
from typing import Optional

from .models import ToolResult
from .redactor import redact_secrets


MAX_OUTPUT_CHARS = 10000

# Environment variable prefixes or suffixes to strip for security
SENSITIVE_ENV_KEYWORDS = (
    "TOKEN",
    "KEY",
    "SECRET",
    "PASS",
    "AUTH",
    "CREDENTIAL",
    "AWS_",
    "GITHUB_",
    "OPENAI_",
    "GEMINI_",
    "GROQ_",
)


def get_sanitized_environment() -> dict[str, str]:
    """Return a clean copy of os.environ without API keys and tokens."""
    clean_env: dict[str, str] = {}

    for k, v in os.environ.items():
        k_upper = k.upper()
        # Filter out sensitive environment variables
        if any(keyword in k_upper for keyword in SENSITIVE_ENV_KEYWORDS):
            continue
        clean_env[k] = v

    # Ensure baseline locale and terminal vars
    clean_env.setdefault("TERM", "xterm-256color")
    clean_env.setdefault("LANG", "C.UTF-8")
    return clean_env


async def run_argv(
    argv: list[str],
    working_directory: Optional[str] = None,
    timeout_seconds: int = 30,
    custom_env: Optional[dict[str, str]] = None,
) -> ToolResult:
    """
    Execute a process directly from an argument vector without shell interpolation.

    Args:
        argv: Command and arguments (e.g. ['ls', '-la']).
        working_directory: Directory to execute the command in.
        timeout_seconds: Maximum time in seconds to wait for execution.
        custom_env: Optional environment dictionary override.

    Returns:
        ToolResult with stdout, stderr, exit_code, truncation flag, and redacted count.
    """
    if not argv or not isinstance(argv, list):
        return ToolResult(
            status="error",
            stderr="Invalid or empty argv list provided.",
            exit_code=1,
        )

    # Defense-in-depth: check against direct shell -c execution
    if len(argv) >= 2 and argv[0] in {"sh", "bash", "zsh", "dash", "ksh"} and argv[1] == "-c":
        return ToolResult(
            status="denied",
            stderr="Invocações de shell direto com -c não são permitidas.",
            exit_code=1,
        )

    # Validate that executable exists
    executable = argv[0]
    exec_path = shutil.which(executable)
    if not exec_path:
        return ToolResult(
            status="error",
            stderr=f"Executable not found in PATH: {executable}",
            exit_code=127,
        )

    cwd = None
    if working_directory:
        cwd_path = Path(os.path.expanduser(working_directory)).resolve()
        if cwd_path.exists() and cwd_path.is_dir():
            cwd = str(cwd_path)
        else:
            return ToolResult(
                status="error",
                stderr=f"Working directory does not exist: {working_directory}",
                exit_code=1,
            )

    env = custom_env if custom_env is not None else get_sanitized_environment()

    try:
        process = await asyncio.create_subprocess_exec(
            exec_path,
            *argv[1:],
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
            cwd=cwd,
            env=env,
        )

        try:
            stdout_bytes, stderr_bytes = await asyncio.wait_for(
                process.communicate(),
                timeout=timeout_seconds,
            )
        except asyncio.TimeoutError:
            try:
                process.terminate()
                await asyncio.sleep(0.2)
                if process.returncode is None:
                    process.kill()
            except Exception:
                pass

            return ToolResult(
                status="timeout",
                stderr=f"Execution timed out after {timeout_seconds} seconds.",
                exit_code=-1,
            )

        stdout_str = stdout_bytes.decode("utf-8", errors="replace")
        stderr_str = stderr_bytes.decode("utf-8", errors="replace")
        exit_code = process.returncode if process.returncode is not None else 0

        # Truncate if output exceeds limit
        truncated = False
        if len(stdout_str) > MAX_OUTPUT_CHARS:
            stdout_str = (
                stdout_str[:MAX_OUTPUT_CHARS]
                + f"\n\n[... Saída truncada em {MAX_OUTPUT_CHARS} caracteres ...]"
            )
            truncated = True

        if len(stderr_str) > MAX_OUTPUT_CHARS:
            stderr_str = (
                stderr_str[:MAX_OUTPUT_CHARS]
                + f"\n\n[... Erro truncado em {MAX_OUTPUT_CHARS} caracteres ...]"
            )
            truncated = True

        # Redact secrets
        stdout_redacted, count_stdout = redact_secrets(stdout_str)
        stderr_redacted, count_stderr = redact_secrets(stderr_str)
        total_redacted = count_stdout + count_stderr

        status = "ok" if exit_code == 0 else "error"

        return ToolResult(
            status=status,
            stdout=stdout_redacted,
            stderr=stderr_redacted,
            exit_code=exit_code,
            truncated=truncated,
            secrets_redacted=total_redacted,
        )

    except Exception as e:
        return ToolResult(
            status="error",
            stderr=f"Execution failure: {e}",
            exit_code=1,
        )
