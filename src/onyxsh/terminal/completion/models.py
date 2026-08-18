# onyxsh/terminal/completion/models.py
"""
Data models and enumerations for the intelligent autocomplete system.
"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Dict, List, Optional


class CompletionType(str, Enum):
    """Types of completion items."""
    COMMAND = "command"
    SUBCOMMAND = "subcommand"
    FLAG = "flag"
    ARGUMENT = "argument"
    HISTORY = "history"
    SNIPPET = "snippet"
    FILE = "file"


class CompletionSource(str, Enum):
    """Source provenance of a completion suggestion."""
    SPEC = "spec"         # From built-in curated Linux CLI specs
    HISTORY = "history"   # From SQLite CommandHistoryManager
    SNIPPET = "snippet"   # From SnippetManager
    AI = "ai"             # From AI Assistant Copilot


@dataclass
class CompletionItem:
    """A single autocomplete suggestion item."""
    text: str                                  # The text to insert into terminal
    display_text: Optional[str] = None         # Optional display text in UI (defaults to text)
    description: str = ""                      # Brief explanation (e.g. "Install packages")
    completion_type: CompletionType = CompletionType.COMMAND
    source: CompletionSource = CompletionSource.SPEC
    score: float = 1.0                         # Ranking score (higher = top priority)
    icon_name: str = "utilities-terminal-symbolic"
    prefix_to_replace: str = ""                # The token portion being replaced/completed
    suffix_to_insert: str = ""                 # Suffix to append if only ghost text is used
    metadata: Dict[str, Any] = field(default_factory=dict)

    def get_display_text(self) -> str:
        """Returns display text for UI row."""
        return self.display_text or self.text

    def get_icon(self) -> str:
        """Returns relevant symbolic icon name."""
        if self.source == CompletionSource.HISTORY:
            return "document-open-recent-symbolic"
        elif self.source == CompletionSource.SNIPPET:
            return "system-run-symbolic"
        elif self.completion_type == CompletionType.FLAG:
            return "emblem-symbolic-link"
        elif self.completion_type == CompletionType.SUBCOMMAND:
            return "media-playback-start-symbolic"
        return self.icon_name or "utilities-terminal-symbolic"


@dataclass
class CompletionContext:
    """Contextual information at the cursor position."""
    full_line: str = ""                        # Entire text on the current prompt line
    cursor_position: int = 0                   # Character index within full_line
    line_before_cursor: str = ""               # Text before the cursor
    tokens: List[str] = field(default_factory=list) # Parsed command tokens
    current_word: str = ""                     # Token currently being typed at cursor
    token_index: int = 0                       # Index of current_word in tokens
    command_root: str = ""                     # Base command (e.g. "apt" or "docker", unwrapping sudo)
    is_sudo: bool = False                      # Whether sudo prefixes the command
    cwd: str = ""                              # Current working directory
    host: str = "localhost"                    # Host / session name
