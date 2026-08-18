# onyxsh/terminal/completion/engine.py
"""
Multi-source Intelligent Autocomplete and Suggestion Engine.
Orchestrates Command Specs, SQLite Command History, and Snippet Manager.
"""

import os
import re
import shlex
from typing import Any, Dict, List, Optional, Tuple

from ...settings.manager import SettingsManager
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .models import (
    CompletionContext,
    CompletionItem,
    CompletionSource,
    CompletionType,
)
from .specs.registry import SpecRegistry, get_spec_registry


class CompletionEngine:
    """
    Core engine responsible for parsing line context and aggregating
    intelligent completions from specs, history, and snippets.
    """

    def __init__(
        self,
        settings_manager: Optional[SettingsManager] = None,
        spec_registry: Optional[SpecRegistry] = None,
    ) -> None:
        self.logger = get_logger("onyxsh.terminal.completion.engine")
        self.settings_manager = settings_manager
        self.spec_registry = spec_registry or get_spec_registry()

    def parse_context(
        self,
        full_line: str,
        cursor_pos: Optional[int] = None,
        cwd: str = "",
        host: str = "localhost",
    ) -> CompletionContext:
        """
        Parses the active prompt line into structured CompletionContext.
        """
        if cursor_pos is None:
            cursor_pos = len(full_line)
        cursor_pos = max(0, min(len(full_line), cursor_pos))

        line_before_cursor = full_line[:cursor_pos]

        # Isolate the current command segment in compound lines (e.g. `cd /etc && sudo apt ` or `ls | grep `)
        # Split by command separators (;, &&, ||, |)
        delimiters = [";", "&&", "||", "|"]
        last_delim_pos = -1
        for delim in delimiters:
            pos = line_before_cursor.rfind(delim)
            if pos != -1:
                last_delim_pos = max(last_delim_pos, pos + len(delim))

        if last_delim_pos != -1:
            active_segment = line_before_cursor[last_delim_pos:]
        else:
            active_segment = line_before_cursor

        # Determine current word being typed at cursor
        # Look backwards from end of active_segment until whitespace or delimiter
        m = re.search(r"(\S+)$", active_segment)
        current_word = m.group(1) if m else ""

        # Parse tokens
        tokens = active_segment.strip().split()
        # If segment ends with whitespace, we are starting a new word
        if active_segment.endswith(" ") or not active_segment:
            current_word = ""

        is_sudo = False
        command_root = ""
        effective_tokens = list(tokens)

        if effective_tokens:
            if effective_tokens[0].lower() == "sudo":
                is_sudo = True
                if len(effective_tokens) > 1:
                    command_root = effective_tokens[1].lower()
                else:
                    command_root = "sudo"
            else:
                command_root = effective_tokens[0].lower()

        token_index = len(tokens) - (0 if current_word else -1)

        return CompletionContext(
            full_line=full_line,
            cursor_position=cursor_pos,
            line_before_cursor=line_before_cursor,
            tokens=tokens,
            current_word=current_word,
            token_index=max(0, token_index),
            command_root=command_root,
            is_sudo=is_sudo,
            cwd=cwd,
            host=host,
        )

    def get_completions(
        self,
        full_line: str,
        cursor_pos: Optional[int] = None,
        cwd: str = "",
        host: str = "localhost",
        limit: int = 6,
    ) -> List[CompletionItem]:
        """
        Calculates and ranks autocomplete items for the current line state.
        """
        # Check settings
        if self.settings_manager:
            if not self.settings_manager.get("autocomplete_enabled", True):
                return []

        context = self.parse_context(full_line, cursor_pos, cwd, host)
        line_clean = context.line_before_cursor.strip()

        # Do not suggest on empty prompt line unless explicitly requested
        if not line_clean and not context.tokens:
            return []

        all_items: List[CompletionItem] = []

        # 1. Query Command Specs
        specs_enabled = (
            self.settings_manager.get("autocomplete_specs_enabled", True)
            if self.settings_manager
            else True
        )
        if specs_enabled:
            all_items.extend(self._get_spec_completions(context))

        # 2. Query SQLite History
        history_enabled = (
            self.settings_manager.get("autocomplete_history_enabled", True)
            if self.settings_manager
            else True
        )
        if history_enabled:
            all_items.extend(self._get_history_completions(context))

        # 3. Query Snippets
        snippets_enabled = (
            self.settings_manager.get("autocomplete_snippets_enabled", True)
            if self.settings_manager
            else True
        )
        if snippets_enabled:
            all_items.extend(self._get_snippet_completions(context))

        # Deduplicate and sort by score descending
        seen_texts = set()
        ranked_items: List[CompletionItem] = []

        for item in sorted(all_items, key=lambda x: x.score, reverse=True):
            clean_text = item.text.strip()
            if clean_text not in seen_texts:
                seen_texts.add(clean_text)
                # Compute suffix to insert for ghost text
                if not item.suffix_to_insert:
                    prefix = context.current_word
                    if prefix and item.text.startswith(prefix):
                        item.suffix_to_insert = item.text[len(prefix):]
                    elif not prefix:
                        item.suffix_to_insert = item.text

                ranked_items.append(item)
                if len(ranked_items) >= limit:
                    break

        return ranked_items

    def _get_spec_completions(
        self, context: CompletionContext
    ) -> List[CompletionItem]:
        """Generates completions from the declarative command specification registry."""
        items: List[CompletionItem] = []
        tokens = context.tokens

        # Case A: Typing the first command (e.g. `ap` -> `apt`, `doc` -> `docker`, `git`)
        if (len(tokens) == 0) or (len(tokens) == 1 and not context.line_before_cursor.endswith(" ")):
            prefix = context.current_word.lower()
            for cmd_name in self.spec_registry.get_all_command_names():
                if not prefix or cmd_name.startswith(prefix):
                    spec = self.spec_registry.get_spec(cmd_name)
                    desc = spec.description if spec else ""
                    score = 3.0 if cmd_name == prefix else 2.5
                    items.append(
                        CompletionItem(
                            text=cmd_name,
                            description=desc,
                            completion_type=CompletionType.COMMAND,
                            source=CompletionSource.SPEC,
                            score=score,
                            prefix_to_replace=context.current_word,
                        )
                    )
            return items

        # Case B: Typing `sudo <subcommand>` (e.g. `sudo a` -> `sudo apt`, `sudo systemctl`)
        if context.is_sudo and (len(tokens) == 1 or (len(tokens) == 2 and not context.line_before_cursor.endswith(" "))):
            prefix = context.current_word.lower() if context.tokens and len(context.tokens) >= 2 else ""
            if prefix != "sudo":
                for cmd_name in self.spec_registry.get_all_command_names():
                    if cmd_name == "sudo":
                        continue
                    if not prefix or cmd_name.startswith(prefix):
                        spec = self.spec_registry.get_spec(cmd_name)
                        desc = spec.description if spec else ""
                        items.append(
                            CompletionItem(
                                text=cmd_name,
                                description=desc,
                                completion_type=CompletionType.COMMAND,
                                source=CompletionSource.SPEC,
                                score=2.8 if cmd_name == prefix else 2.4,
                                prefix_to_replace=context.current_word,
                            )
                        )
                return items

        # Case C: Delegating to the specific command's spec (e.g. `apt install`, `docker ps -a`)
        target_cmd = context.command_root
        if target_cmd:
            spec = self.spec_registry.get_spec(target_cmd)
            if spec:
                spec_items = spec.get_completions(context)
                items.extend(spec_items)

        return items

    def _get_history_completions(
        self, context: CompletionContext
    ) -> List[CompletionItem]:
        """Queries the SQLite command history for matching command suggestions."""
        items: List[CompletionItem] = []
        try:
            from ...terminal.command_history import get_command_history_manager

            history_mgr = get_command_history_manager()
            line_prefix = context.line_before_cursor.strip()
            if not line_prefix:
                return []

            # Search history with prefix
            matches = history_mgr.search_commands(
                query=line_prefix,
                limit=5,
                cwd_filter=context.cwd or None,
            )

            for entry in matches:
                cmd_text = entry.command_text.strip()
                if cmd_text.startswith(line_prefix) and cmd_text != line_prefix:
                    # Calculate suffix
                    suffix = cmd_text[len(line_prefix):]
                    desc = f"{_('History')} • {entry.execution_count}x"
                    if entry.exit_code != 0:
                        desc += f" (exit {entry.exit_code})"

                    # Boost score if in the exact same directory or pinned
                    score = 2.0
                    if entry.is_pinned:
                        score += 1.0
                    if entry.cwd == context.cwd:
                        score += 0.8
                    if entry.exit_code == 0:
                        score += 0.5

                    items.append(
                        CompletionItem(
                            text=cmd_text,
                            display_text=cmd_text,
                            description=desc,
                            completion_type=CompletionType.HISTORY,
                            source=CompletionSource.HISTORY,
                            score=score,
                            prefix_to_replace=context.line_before_cursor,
                            suffix_to_insert=suffix,
                            metadata={"entry": entry},
                        )
                    )
        except Exception as e:
            self.logger.debug(f"History completion query error: {e}")

        return items

    def _get_snippet_completions(
        self, context: CompletionContext
    ) -> List[CompletionItem]:
        """Queries the SnippetManager for matching snippet templates."""
        items: List[CompletionItem] = []
        try:
            from ...terminal.snippet_manager import get_snippet_manager

            snippet_mgr = get_snippet_manager()
            line_prefix = context.line_before_cursor.strip().lower()
            if not line_prefix:
                return []

            snippets = snippet_mgr.get_all_snippets()
            for snip in snippets:
                name_match = snip.name.lower().startswith(line_prefix)
                cmd_match = snip.command.lower().startswith(line_prefix)
                if name_match or cmd_match:
                    items.append(
                        CompletionItem(
                            text=snip.command,
                            display_text=f"{snip.name} ({snip.command})",
                            description=snip.description or _("Snippet template"),
                            completion_type=CompletionType.SNIPPET,
                            source=CompletionSource.SNIPPET,
                            score=2.2,
                            prefix_to_replace=context.line_before_cursor,
                        )
                    )
        except Exception as e:
            self.logger.debug(f"Snippet completion query error: {e}")

        return items


_engine_instance: Optional[CompletionEngine] = None


def get_completion_engine(
    settings_manager: Optional[SettingsManager] = None,
) -> CompletionEngine:
    """Returns singleton CompletionEngine instance."""
    global _engine_instance
    if _engine_instance is None:
        _engine_instance = CompletionEngine(settings_manager)
    elif settings_manager and _engine_instance.settings_manager is None:
        _engine_instance.settings_manager = settings_manager
    return _engine_instance
