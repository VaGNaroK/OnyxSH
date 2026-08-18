# onyxsh/terminal/completion/specs/base.py
"""
Base classes for declarative command specifications and completion providers.
"""

from dataclasses import dataclass, field
from typing import Callable, Dict, List, Optional

from ....utils.translation_utils import _
from ..models import CompletionContext, CompletionItem, CompletionSource, CompletionType


@dataclass
class OptionSpec:
    """Specification for a command flag or option (e.g. -y, --help, -f)."""
    names: List[str]                            # e.g. ["-y", "--yes", "--assume-yes"]
    description: str = ""                       # Human explanation
    takes_value: bool = False                   # True if flag expects a parameter
    value_name: str = ""                        # e.g. "<file>", "<port>"


@dataclass
class SubcommandSpec:
    """Specification for a command sub-action (e.g. apt install, systemctl restart)."""
    name: str                                   # e.g. "install", "status", "restart"
    description: str = ""                       # Human explanation
    aliases: List[str] = field(default_factory=list)
    options: List[OptionSpec] = field(default_factory=list)
    subcommands: List["SubcommandSpec"] = field(default_factory=list)
    examples: List[str] = field(default_factory=list)


@dataclass
class CommandSpec:
    """Top-level command specification (e.g. apt, systemctl, docker, git)."""
    name: str                                   # Primary command executable name (e.g. "apt")
    description: str = ""                       # Top-level description
    aliases: List[str] = field(default_factory=list) # Alternate names (e.g. "apt-get")
    subcommands: List[SubcommandSpec] = field(default_factory=list)
    global_options: List[OptionSpec] = field(default_factory=list)

    def get_completions(self, context: CompletionContext) -> List[CompletionItem]:
        """
        Generates relevant completions for this command given the context.
        """
        items: List[CompletionItem] = []
        tokens = context.tokens
        # Skip the command name token itself (and sudo if present)
        offset = 2 if context.is_sudo else 1
        sub_tokens = tokens[offset:] if len(tokens) > offset else []
        num_cmd_tokens = len(tokens) - (1 if context.is_sudo else 0)
        current_word = context.current_word.lower()

        # If user is at position 1 (typing the first subcommand/flag right after the command)
        if num_cmd_tokens <= 2:
            # 1. Match subcommands
            for sub in self.subcommands:
                if not current_word or sub.name.lower().startswith(current_word):
                    items.append(
                        CompletionItem(
                            text=sub.name,
                            description=sub.description,
                            completion_type=CompletionType.SUBCOMMAND,
                            source=CompletionSource.SPEC,
                            score=2.0 if not current_word else (3.0 if sub.name.lower() == current_word else 2.5),
                            prefix_to_replace=context.current_word,
                        )
                    )

            # 2. Match global options
            for opt in self.global_options:
                for opt_name in opt.names:
                    if not current_word or opt_name.lower().startswith(current_word):
                        items.append(
                            CompletionItem(
                                text=opt_name,
                                description=opt.description,
                                completion_type=CompletionType.FLAG,
                                source=CompletionSource.SPEC,
                                score=1.5,
                                prefix_to_replace=context.current_word,
                            )
                        )
            return items

        # User is further down the token list: find active subcommand
        active_sub: Optional[SubcommandSpec] = None
        for t in sub_tokens:
            for sub in self.subcommands:
                if t.lower() == sub.name.lower() or t.lower() in [a.lower() for a in sub.aliases]:
                    active_sub = sub
                    break
            if active_sub:
                break

        if active_sub:
            # Match sub-options
            for opt in active_sub.options:
                for opt_name in opt.names:
                    if not current_word or opt_name.lower().startswith(current_word):
                        items.append(
                            CompletionItem(
                                text=opt_name,
                                description=opt.description,
                                completion_type=CompletionType.FLAG,
                                source=CompletionSource.SPEC,
                                score=1.8,
                                prefix_to_replace=context.current_word,
                            )
                        )

            # Match nested subcommands if any
            for nested in active_sub.subcommands:
                if not current_word or nested.name.lower().startswith(current_word):
                    items.append(
                        CompletionItem(
                            text=nested.name,
                            description=nested.description,
                            completion_type=CompletionType.SUBCOMMAND,
                            source=CompletionSource.SPEC,
                            score=2.0,
                            prefix_to_replace=context.current_word,
                        )
                    )

        # Also offer global options as fallback
        for opt in self.global_options:
            for opt_name in opt.names:
                if current_word and opt_name.lower().startswith(current_word):
                    items.append(
                        CompletionItem(
                            text=opt_name,
                            description=opt.description,
                            completion_type=CompletionType.FLAG,
                            source=CompletionSource.SPEC,
                            score=1.2,
                            prefix_to_replace=context.current_word,
                        )
                    )

        return items
