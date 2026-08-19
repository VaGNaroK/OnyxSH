# onyxsh/terminal/git_assistant.py

"""
AI-powered Git Assistant for OnyxSH.
Generates Conventional Commits messages from repository diffs and status.
"""

from __future__ import annotations

import json
import threading
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

from ..agent.redactor import redact_secrets
from ..utils.git_utils import get_git_diff, get_git_status, get_repo_root
from ..utils.logger import get_logger
from ..utils.translation_utils import _

logger = get_logger("onyxsh.terminal.git_assistant")


class GitCommitAssistant:
    """Generates commit messages using the configured AI provider."""

    def __init__(self, ai_assistant) -> None:
        self._ai_assistant = ai_assistant

    def generate_commit_message_async(
        self,
        cwd: Path | str,
        style: str = "conventional",  # "conventional", "short", "detailed"
        language: str = "pt-BR",  # "pt-BR", "en-US"
        callback: Optional[Callable[[Optional[str], Optional[str]], None]] = None,
    ) -> None:
        """
        Generates a commit message in a background worker thread.
        Callback signature: callback(commit_message: Optional[str], error: Optional[str])
        """
        worker = threading.Thread(
            target=self._worker,
            args=(cwd, style, language, callback),
            daemon=True,
        )
        worker.start()

    def _worker(
        self,
        cwd: Path | str,
        style: str,
        language: str,
        callback: Optional[Callable[[Optional[str], Optional[str]], None]],
    ) -> None:
        try:
            repo_root = get_repo_root(cwd)
            if not repo_root:
                if callback:
                    callback(None, _("O diretório atual não é um repositório Git."))
                return

            status_info = get_git_status(repo_root)
            staged_files = status_info.get("staged", [])
            unstaged_files = status_info.get("unstaged", [])

            # Prefer staged diff; fallback to unstaged diff if nothing is staged
            diff_text = get_git_diff(repo_root, staged=True)
            is_staged_used = bool(diff_text.strip())

            if not is_staged_used:
                diff_text = get_git_diff(repo_root, staged=False)

            if not diff_text.strip() and not status_info.get("untracked"):
                if callback:
                    callback(
                        None,
                        _(
                            "Nenhuma modificação detectada no repositório para gerar commit."
                        ),
                    )
                return

            # Redact secrets before sending to AI model for absolute safety
            safe_diff, _ = redact_secrets(diff_text)

            # Build file summaries
            staged_summary = (
                "\n".join(f"- [{f['status']}] {f['path']}" for f in staged_files)
                or "(Nenhum arquivo estagiado)"
            )
            unstaged_summary = (
                "\n".join(f"- [{f['status']}] {f['path']}" for f in unstaged_files)
                or "(Nenhum)"
            )

            # Construct system and user prompt
            lang_label = "Portuguese (pt-BR)" if "pt" in language.lower() else "English (en-US)"
            prompt = self._build_prompt(
                branch=status_info.get("branch", "main"),
                staged_summary=staged_summary,
                unstaged_summary=unstaged_summary,
                diff=safe_diff,
                style=style,
                language=lang_label,
            )

            # Execute via TerminalAiAssistant
            config = self._ai_assistant._load_configuration()
            messages = [
                {
                    "role": "system",
                    "content": (
                        "You are an expert software engineer specialized in Git version control and writing clean, "
                        "concise, and accurate commit messages following Conventional Commits v1.0.0 standards."
                    ),
                },
                {"role": "user", "content": prompt},
            ]

            raw_response = self._ai_assistant._perform_request(config, messages)
            cleaned_message = self._clean_commit_output(raw_response)

            if callback:
                callback(cleaned_message, None)

        except Exception as e:
            logger.error(f"Error generating commit message: {e}", exc_info=True)
            if callback:
                callback(None, str(e))

    def _build_prompt(
        self,
        branch: str,
        staged_summary: str,
        unstaged_summary: str,
        diff: str,
        style: str,
        language: str,
    ) -> str:
        instructions = ""
        if style == "short":
            instructions = (
                "Provide ONLY a single concise line in Conventional Commits format: `<type>(<scope>): <subject>`. "
                "Do NOT add bullet points or body text."
            )
        elif style == "detailed":
            instructions = (
                "Provide a structured commit message in Conventional Commits format:\n"
                "1. Header: `<type>(<scope>): <concise subject>` (max 72 chars, imperative mood)\n"
                "2. Blank line\n"
                "3. Detailed bullet points explaining what and why changes were made.\n"
            )
        else:  # conventional default
            instructions = (
                "Provide a standard commit message following Conventional Commits format:\n"
                "- Header: `<type>(<scope>): <subject in imperative mood>`\n"
                "- Body: 2 to 5 bullet points with concise summaries of the main changes.\n"
            )

        return (
            f"Please generate a Git commit message in {language} for the changes below.\n\n"
            f"**Branch:** {branch}\n\n"
            f"**Staged Files:**\n{staged_summary}\n\n"
            f"**Unstaged Files:**\n{unstaged_summary}\n\n"
            f"**Git Diff:**\n```diff\n{diff}\n```\n\n"
            f"**Rules & Guidelines:**\n"
            f"{instructions}\n"
            f"- Valid types: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert.\n"
            f"- Do NOT wrap the entire commit message in markdown ```code blocks```. Return RAW commit text only.\n"
            f"- Do NOT add introductory or conversational phrases like 'Here is the commit message:'."
        )

    def _clean_commit_output(self, text: str) -> str:
        """Removes markdown code fences and conversational noise."""
        if not text:
            return ""

        # Handle JSON wrapper if model returned JSON
        try:
            parsed = json.loads(text)
            if isinstance(parsed, dict) and "reply" in parsed:
                text = parsed["reply"]
        except Exception:
            pass

        cleaned = text.strip()
        # Strip ```markdown or ```git or ``` fences
        if cleaned.startswith("```"):
            lines = cleaned.splitlines()
            if len(lines) >= 2 and lines[0].startswith("```") and lines[-1].startswith("```"):
                cleaned = "\n".join(lines[1:-1]).strip()

        # Remove common chat greetings
        noise_prefixes = [
            "here is the commit message:",
            "here's the commit message:",
            "aqui está a mensagem de commit:",
            "mensagem de commit sugerida:",
        ]
        for prefix in noise_prefixes:
            if cleaned.lower().startswith(prefix):
                cleaned = cleaned[len(prefix) :].strip()

        return cleaned
