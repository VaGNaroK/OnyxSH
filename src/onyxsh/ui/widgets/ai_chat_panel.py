"""AI Chat Panel Widget - Persistent overlay for AI conversations."""

from __future__ import annotations

import json
import random
import re
import uuid
from datetime import datetime
from pathlib import Path
from typing import TYPE_CHECKING

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")

from gi.repository import Adw, Gdk, Gio, GLib, GObject, Gtk, Pango

from ...utils.icons import icon_image
from ...utils.logger import get_logger
from ...utils.tooltip_helper import get_tooltip_helper
from ...utils.translation_utils import _
from .conversation_history import ConversationHistoryPanel


if TYPE_CHECKING:
    from ...terminal.ai_assistant import AIAssistant

logger = get_logger(__name__)

# Path to CSS styles directory
_STYLES_DIR = Path(__file__).parent.parent.parent / "data" / "styles"

# Pre-compiled regex patterns for markdown formatting (performance optimization)
_CODE_BLOCK_PATTERN = re.compile(r'```(\w*)\n?(.*?)```', re.DOTALL)
_INLINE_CODE_PATTERN = re.compile(r'`([^`]+)`')
_BOLD_PATTERN = re.compile(r'\*\*([^*]+)\*\*')
_ITALIC_PATTERN = re.compile(r'\*([^*]+)\*')
_HEADER3_PATTERN = re.compile(r'^### (.+)$', re.MULTILINE)
_HEADER2_PATTERN = re.compile(r'^## (.+)$', re.MULTILINE)
_HEADER1_PATTERN = re.compile(r'^# (.+)$', re.MULTILINE)

# Lazy-loaded pygments module (optional dependency)
_pygments_module = None
_pygments_available = None  # None = not checked yet, True/False = result


def _get_pygments():
    """Lazy load pygments module. Returns None if not installed."""
    global _pygments_module, _pygments_available

    if _pygments_available is None:
        try:
            import pygments
            from pygments.lexers import TextLexer, get_lexer_by_name
            from pygments.util import ClassNotFound

            _pygments_module = {
                "pygments": pygments,
                "get_lexer_by_name": get_lexer_by_name,
                "TextLexer": TextLexer,
                "ClassNotFound": ClassNotFound,
            }
            _pygments_available = True
            logger.debug("Pygments loaded successfully for syntax highlighting")
        except ImportError:
            _pygments_module = None
            _pygments_available = False
            logger.debug("Pygments not available, using fallback highlighting")

    return _pygments_module


def _extract_reply_from_json(text: str) -> str:
    """Extract 'reply' field from JSON response or return text safely.

    Handles complete and partial responses without truncating code/quotes.
    """
    if not text:
        return text

    stripped = text.strip()

    # 1. If it's valid JSON
    try:
        data = json.loads(stripped)
        if isinstance(data, dict):
            if "reply" in data and isinstance(data["reply"], str):
                return data["reply"]
            if "content" in data and isinstance(data["content"], str):
                return data["content"]
            if "message" in data and isinstance(data["message"], str):
                return data["message"]
    except Exception:
        pass

    # 2. Check if wrapped in markdown code fence like ```json { ... } ```
    if stripped.startswith("```"):
        first_newline = stripped.find("\n")
        if first_newline != -1:
            inner = stripped[first_newline + 1 :]
            if inner.endswith("```"):
                inner = inner[:-3].strip()
            try:
                data = json.loads(inner)
                if isinstance(data, dict) and "reply" in data and isinstance(data["reply"], str):
                    return data["reply"]
            except Exception:
                pass

    # 3. Robust regex extraction for JSON reply
    reply_match = re.search(
        r'"reply"\s*:\s*"(.*?)(?:"\s*,\s*"(?:commands|cmd|tools)"\s*:|"\s*\}\s*$)',
        stripped,
        re.DOTALL,
    )
    if reply_match:
        val = reply_match.group(1)
        val = (
            val.replace('\\"', '"')
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\\\", "\\")
        )
        return val

    # 4. Partial streaming case:
    if stripped.startswith("{"):
        match_start = re.search(r'["\']reply["\']\s*:\s*["\']', stripped)
        if match_start:
            content_start = match_start.end()
            partial = stripped[content_start:]
            trailing_commands = re.search(
                r'["\']\s*,\s*["\']commands["\'].*$', partial, re.DOTALL
            )
            if trailing_commands:
                partial = partial[: trailing_commands.start()]
            elif partial.endswith('"}') or partial.endswith('"]'):
                partial = partial.rstrip('"} ]')
            partial = (
                partial.replace('\\"', '"')
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\\\", "\\")
            )
            return partial

    return text


def _normalize_commands(commands: list | None) -> list:
    """Normalize commands to strings or structured step dictionaries."""
    if not commands:
        return []

    result = []
    for cmd in commands:
        if isinstance(cmd, str):
            result.append(cmd)
        elif isinstance(cmd, dict):
            if "step_id" in cmd or "risk" in cmd or "tool" in cmd or "approval" in cmd:
                result.append(cmd)
            else:
                command_str = cmd.get("command", "") or cmd.get("cmd", "")
                if command_str:
                    result.append(command_str)
        else:
            result.append(cmd)
    return result


# Extended list of quick prompts (200+ items) - random selection shown per new conversation
ALL_QUICK_PROMPTS = [
    # Basic Terminal Help
    ("📁", _("How do I navigate directories?")),
    ("🔍", _("How do I find files by name?")),
    ("📝", _("How do I edit files in terminal?")),
    ("📊", _("How do I view disk usage?")),
    ("🔐", _("How do I change file permissions?")),
    ("📦", _("How do I compress files?")),
    ("🌐", _("How do I check my IP address?")),
    ("⚙️", _("How do I view running processes?")),
    ("💾", _("How do I check memory usage?")),
    ("🔄", _("How do I restart a service?")),
    # Git Commands
    ("🌿", _("How do I create a new branch?")),
    ("🔀", _("How do I merge branches?")),
    ("📤", _("How do I push to remote?")),
    ("📥", _("How do I pull changes?")),
    ("↩️", _("How do I undo last commit?")),
    ("📜", _("How do I view commit history?")),
    ("🏷️", _("How do I create a tag?")),
    ("🔎", _("How do I find who changed a line?")),
    ("🗑️", _("How do I delete a branch?")),
    ("📋", _("How do I stash changes?")),
    # Docker Commands
    ("🐳", _("How do I list Docker containers?")),
    ("🚀", _("How do I run a Docker container?")),
    ("🛑", _("How do I stop a container?")),
    ("🖼️", _("How do I list Docker images?")),
    ("🧹", _("How do I clean Docker resources?")),
    ("📊", _("How do I view container logs?")),
    ("🔗", _("How do I create Docker network?")),
    ("💽", _("How do I manage Docker volumes?")),
    ("🏗️", _("How do I build a Docker image?")),
    ("🔄", _("How do I restart a container?")),
    # SSH and Networking
    ("🔑", _("How do I generate SSH keys?")),
    ("🔐", _("How do I copy SSH key to server?")),
    ("📡", _("How do I check open ports?")),
    ("🌐", _("How do I test network connectivity?")),
    ("🔍", _("How do I DNS lookup?")),
    ("📊", _("How do I monitor network traffic?")),
    ("🧱", _("How do I configure firewall?")),
    ("🔄", _("How do I create SSH tunnel?")),
    ("📋", _("How do I copy files via SSH?")),
    ("⚡", _("How do I speed up SSH connections?")),
    # File Operations
    ("📄", _("How do I create empty file?")),
    ("📂", _("How do I create directory?")),
    ("🗑️", _("How do I delete files safely?")),
    ("📋", _("How do I copy files?")),
    ("✂️", _("How do I move files?")),
    ("🔗", _("How do I create symbolic link?")),
    ("🔍", _("How do I search file contents?")),
    ("📊", _("How do I compare files?")),
    ("🔄", _("How do I sync directories?")),
    ("📝", _("How do I append to file?")),
    # Text Processing
    ("🔎", _("How do I use grep?")),
    ("✂️", _("How do I use awk?")),
    ("📝", _("How do I use sed?")),
    ("📊", _("How do I count lines?")),
    ("🔀", _("How do I sort text?")),
    ("🔗", _("How do I join files?")),
    ("🎯", _("How do I extract columns?")),
    ("🔄", _("How do I remove duplicates?")),
    ("📋", _("How do I format JSON?")),
    ("🔍", _("How do I search and replace?")),
    # System Administration
    ("👤", _("How do I add a user?")),
    ("👥", _("How do I manage groups?")),
    ("🔐", _("How do I change password?")),
    ("📊", _("How do I check system load?")),
    ("💽", _("How do I mount a drive?")),
    ("📦", _("How do I install packages?")),
    ("🔄", _("How do I update system?")),
    ("⚙️", _("How do I configure cron jobs?")),
    ("📜", _("How do I view system logs?")),
    ("🔍", _("How do I find large files?")),
    # Python Development
    ("🐍", _("How do I create virtualenv?")),
    ("📦", _("How do I install pip packages?")),
    ("🔍", _("How do I find Python package?")),
    ("📋", _("How do I list installed packages?")),
    ("🧪", _("How do I run Python tests?")),
    ("📊", _("How do I profile Python code?")),
    ("🔧", _("How do I format Python code?")),
    ("📝", _("How do I create requirements.txt?")),
    ("🚀", _("How do I run Python script?")),
    ("🔍", _("How do I debug Python?")),
    # Node.js Development
    ("📦", _("How do I initialize npm project?")),
    ("🔧", _("How do I install npm packages?")),
    ("🚀", _("How do I run npm scripts?")),
    ("📋", _("How do I list npm packages?")),
    ("🔄", _("How do I update npm packages?")),
    ("🧹", _("How do I clean npm cache?")),
    ("🔗", _("How do I link npm package?")),
    ("📊", _("How do I audit npm packages?")),
    ("🔍", _("How do I find npm package?")),
    ("⚡", _("How do I use npx?")),
    # Shell Scripting
    ("📝", _("How do I write a bash script?")),
    ("🔄", _("How do I use loops in bash?")),
    ("❓", _("How do I use conditionals?")),
    ("📊", _("How do I read user input?")),
    ("📁", _("How do I read from file?")),
    ("✍️", _("How do I write to file?")),
    ("🔧", _("How do I use functions?")),
    ("📋", _("How do I parse arguments?")),
    ("⚠️", _("How do I handle errors?")),
    ("🔍", _("How do I debug bash script?")),
    # Kubernetes
    ("☸️", _("How do I get pods?")),
    ("📊", _("How do I view pod logs?")),
    ("🚀", _("How do I deploy to k8s?")),
    ("🔄", _("How do I scale deployment?")),
    ("🔍", _("How do I describe pod?")),
    ("📋", _("How do I get services?")),
    ("⚙️", _("How do I create configmap?")),
    ("🔐", _("How do I create secret?")),
    ("🖥️", _("How do I exec into pod?")),
    ("📤", _("How do I port forward?")),
    # Database Operations
    ("🗄️", _("How do I connect to PostgreSQL?")),
    ("📊", _("How do I backup database?")),
    ("🔄", _("How do I restore database?")),
    ("📋", _("How do I list databases?")),
    ("🔍", _("How do I query from terminal?")),
    ("📤", _("How do I export to CSV?")),
    ("📥", _("How do I import from CSV?")),
    ("👤", _("How do I create database user?")),
    ("🔐", _("How do I grant permissions?")),
    ("📊", _("How do I check database size?")),
    # Performance and Monitoring
    ("📊", _("How do I monitor CPU usage?")),
    ("💾", _("How do I monitor memory?")),
    ("💽", _("How do I monitor disk I/O?")),
    ("🌐", _("How do I monitor network?")),
    ("⏱️", _("How do I benchmark command?")),
    ("🔍", _("How do I trace system calls?")),
    ("📈", _("How do I view process tree?")),
    ("🔥", _("How do I find bottlenecks?")),
    ("📋", _("How do I list open files?")),
    ("🧵", _("How do I view thread info?")),
    # Archives and Compression
    ("📦", _("How do I create tar archive?")),
    ("📂", _("How do I extract tar.gz?")),
    ("🗜️", _("How do I use gzip?")),
    ("📋", _("How do I list archive contents?")),
    ("➕", _("How do I add to archive?")),
    ("📤", _("How do I create zip file?")),
    ("📥", _("How do I extract zip?")),
    ("🔐", _("How do I encrypt archive?")),
    ("✂️", _("How do I split archive?")),
    ("🔗", _("How do I merge archives?")),
    # Security
    ("🔐", _("How do I encrypt file?")),
    ("🔓", _("How do I decrypt file?")),
    ("🔑", _("How do I generate password?")),
    ("✅", _("How do I verify checksum?")),
    ("📋", _("How do I list certificates?")),
    ("🔏", _("How do I sign file?")),
    ("🔍", _("How do I scan for vulnerabilities?")),
    ("🧹", _("How do I secure file permissions?")),
    ("📊", _("How do I audit system?")),
    ("🔐", _("How do I use GPG?")),
    # Tmux and Screen
    ("🖥️", _("How do I start tmux session?")),
    ("📋", _("How do I list tmux sessions?")),
    ("🔗", _("How do I attach to session?")),
    ("✂️", _("How do I split tmux pane?")),
    ("🔄", _("How do I switch panes?")),
    ("📝", _("How do I rename window?")),
    ("❌", _("How do I kill session?")),
    ("📤", _("How do I detach from session?")),
    ("📋", _("How do I copy in tmux?")),
    ("⚙️", _("How do I configure tmux?")),
    # Vim/Neovim
    ("📝", _("How do I save in vim?")),
    ("❌", _("How do I quit vim?")),
    ("🔍", _("How do I search in vim?")),
    ("🔄", _("How do I replace in vim?")),
    ("📋", _("How do I copy line in vim?")),
    ("✂️", _("How do I delete line in vim?")),
    ("↩️", _("How do I undo in vim?")),
    ("📊", _("How do I go to line in vim?")),
    ("🔀", _("How do I split in vim?")),
    ("📁", _("How do I open file in vim?")),
    # Environment and Config
    ("🔧", _("How do I set environment variable?")),
    ("📋", _("How do I list env variables?")),
    ("📝", _("How do I edit bashrc?")),
    ("🔄", _("How do I reload bashrc?")),
    ("📊", _("How do I view PATH?")),
    ("➕", _("How do I add to PATH?")),
    ("🔍", _("How do I find config file?")),
    ("📋", _("How do I export variable?")),
    ("🔐", _("How do I use .env file?")),
    ("⚙️", _("How do I set alias?")),
    # Advanced Commands
    ("🔗", _("How do I use xargs?")),
    ("📊", _("How do I use find with exec?")),
    ("🔄", _("How do I use parallel?")),
    ("📋", _("How do I use tee?")),
    ("⏱️", _("How do I use watch?")),
    ("📊", _("How do I use htop?")),
    ("🔍", _("How do I use fzf?")),
    ("📝", _("How do I use heredoc?")),
    ("🔄", _("How do I use subshell?")),
    ("📋", _("How do I use command substitution?")),
    # Misc
    ("📅", _("How do I format date?")),
    ("🧮", _("How do I calculate in terminal?")),
    ("🎨", _("How do I use colors in terminal?")),
    ("📊", _("How do I create histogram?")),
    ("🔔", _("How do I send notification?")),
    ("📋", _("How do I use clipboard?")),
    ("🖼️", _("How do I view image in terminal?")),
    ("📊", _("How do I plot in terminal?")),
    ("🔊", _("How do I play sound?")),
    ("⏰", _("How do I schedule task?")),
]


def get_random_quick_prompts(count: int = 6) -> list[tuple[str, str]]:
    """Get a random selection of quick prompts."""
    return random.sample(ALL_QUICK_PROMPTS, min(count, len(ALL_QUICK_PROMPTS)))


class LoadingIndicator(Gtk.Box):
    """Loading indicator with animated dots."""

    def __init__(self):
        super().__init__(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.add_css_class("ai-loading-indicator")

        self._spinner = Gtk.Spinner()
        self._spinner.set_size_request(16, 16)
        self.append(self._spinner)

        self._label = Gtk.Label(label=_("AI is thinking..."))
        self._label.add_css_class("dim-label")
        self.append(self._label)

    def start(self):
        """Start the loading animation."""
        self._spinner.start()
        self.set_visible(True)

    def stop(self):
        """Stop the loading animation."""
        self._spinner.stop()
        self.set_visible(False)


class MessageBubble(Gtk.Box):
    """A chat message bubble widget with role indicator."""

    def __init__(
        self,
        role: str,
        content: str,
        commands: list[str] | None = None,
        settings_manager=None,
    ):
        super().__init__(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        self._role = role
        self._content = content
        self._commands = commands or []
        self._settings_manager = settings_manager
        self._palette = None

        # Get terminal palette if using terminal theme
        if settings_manager and settings_manager.get("gtk_theme", "") == "terminal":
            scheme = settings_manager.get_color_scheme_data()
            self._palette = scheme.get("palette", [])

        self._setup_ui()

    def _add_tooltip(self, widget: Gtk.Widget, text: str):
        """Add tooltip to widget using custom helper or fallback to standard."""
        helper = get_tooltip_helper()
        if helper:
            helper.add_tooltip(widget, text)
        else:
            widget.set_tooltip_text(text)

    def _setup_ui(self):
        # Role indicator header
        header_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        header_box.set_margin_start(8)
        header_box.set_margin_end(8)
        header_box.set_margin_top(4)

        if self._role == "user":
            self.set_halign(Gtk.Align.END)
            # User icon and label
            user_icon = Gtk.Image.new_from_icon_name("avatar-default-symbolic")
            user_icon.add_css_class("dim-label")
            header_box.append(user_icon)

            role_label = Gtk.Label(label=_("You"))
            role_label.add_css_class("caption")
            role_label.add_css_class("dim-label")
            header_box.append(role_label)
        else:
            self.set_halign(Gtk.Align.START)
            # AI icon and label
            ai_icon = Gtk.Image.new_from_icon_name("dialog-information-symbolic")
            ai_icon.add_css_class("accent")
            header_box.append(ai_icon)

            role_label = Gtk.Label(label=_("AI Assistant"))
            role_label.add_css_class("caption")
            role_label.add_css_class("accent")
            header_box.append(role_label)

            # Spacer and copy full response button
            spacer = Gtk.Box(hexpand=True)
            header_box.append(spacer)

            copy_all_btn = Gtk.Button()
            copy_all_btn.set_icon_name("edit-copy-symbolic")
            copy_all_btn.add_css_class("flat")
            copy_all_btn.add_css_class("circular")
            copy_all_btn.add_css_class("ai-cmd-btn")
            copy_all_btn.connect("clicked", self._on_copy_full_message)
            self._add_tooltip(copy_all_btn, _("Copiar resposta completa"))
            header_box.append(copy_all_btn)

        self.append(header_box)

        # Main content box
        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)

        if self._role == "user":
            content_box.add_css_class("ai-message-user")
        else:
            content_box.add_css_class("ai-message-assistant")

        content_box.set_margin_start(8)
        content_box.set_margin_end(8)
        content_box.set_margin_bottom(4)

        # Message label with markdown-like formatting
        self._label = Gtk.Label()
        self._label.set_wrap(True)
        self._label.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
        if hasattr(Gtk, "NaturalWrapMode") and hasattr(self._label, "set_natural_wrap_mode"):
            self._label.set_natural_wrap_mode(Gtk.NaturalWrapMode.WORD)
        self._label.set_xalign(0)
        self._label.set_selectable(True)
        self._label.set_max_width_chars(60)

        # Convert markdown to Pango markup with fallback
        formatted_content = self._format_content(self._content)
        try:
            self._label.set_markup(formatted_content)
        except Exception:
            # Markup parsing failed, fallback to plain text
            self._label.set_text(self._content)

        content_box.append(self._label)
        self._add_code_block_actions(content_box)
        self.append(content_box)

        # Add command buttons for assistant messages
        if self._role == "assistant" and self._commands:
            self._add_command_buttons()

    def _get_code_block_colors(self) -> dict:
        """Get colors for code blocks and inline code based on theme."""
        style_manager = Adw.StyleManager.get_default()
        is_dark = style_manager.get_dark()

        if is_dark:
            return {
                "block_bg": "#2d2d2d",
                "block_fg": "#e6e6e6",
                "inline_bg": "#3d3d3d",
                "inline_fg": "#ff79c6",  # Pink for inline code
            }
        else:
            return {
                "block_bg": "#f0f0f0",  # Light gray background
                "block_fg": "#24292e",  # Dark text
                "inline_bg": "#eff1f3",  # Subtle gray for inline
                "inline_fg": "#d63384",  # Magenta for inline code
            }

    def _format_content(self, text: str) -> str:
        """Convert basic markdown to Pango markup with syntax highlighting."""
        # Get theme-adaptive colors
        colors = self._get_code_block_colors()
        block_bg = colors["block_bg"]
        block_fg = colors["block_fg"]
        inline_bg = colors["inline_bg"]
        inline_fg = colors["inline_fg"]

        # Step 1: Extract and preserve code blocks and inline code
        # Store them with placeholders to prevent markdown transformations inside code
        # Use Unicode private use area characters as markers (safe from normal text)
        code_blocks = []
        inline_codes = []

        def store_code_block(match):
            lang = match.group(1).lower() if match.group(1) else ""
            code = match.group(2)
            highlighted = self._highlight_code_for_label(code, lang)
            idx = len(code_blocks)
            code_blocks.append(f'<span background="{block_bg}" foreground="{block_fg}"><tt>{highlighted}</tt></span>')
            return f'\ue000CODEBLOCK{idx}\ue001'

        def store_inline_code(match):
            code = match.group(1)
            escaped_code = GLib.markup_escape_text(code)
            idx = len(inline_codes)
            inline_codes.append(f'<span background="{inline_bg}" foreground="{inline_fg}"><tt>{escaped_code}</tt></span>')
            return f'\ue000INLINE{idx}\ue001'

        # Replace code blocks with placeholders (using pre-compiled patterns)
        text = _CODE_BLOCK_PATTERN.sub(store_code_block, text)

        # Replace inline code with placeholders
        text = _INLINE_CODE_PATTERN.sub(store_inline_code, text)

        # Step 2: Escape remaining text for Pango markup
        text = GLib.markup_escape_text(text)

        # Step 3: Apply markdown transformations (safe now - no code content)
        # Bold (**...**)
        text = _BOLD_PATTERN.sub(r'<b>\1</b>', text)

        # Italic (*...*)
        text = _ITALIC_PATTERN.sub(r'<i>\1</i>', text)

        # Headers (# ...)
        text = _HEADER3_PATTERN.sub(r'<b>\1</b>', text)
        text = _HEADER2_PATTERN.sub(r'<b><big>\1</big></b>', text)
        text = _HEADER1_PATTERN.sub(r'<b><big><big>\1</big></big></b>', text)

        # Step 4: Restore code blocks and inline codes
        for i, block in enumerate(code_blocks):
            text = text.replace(f'\ue000CODEBLOCK{i}\ue001', block)

        for i, inline in enumerate(inline_codes):
            text = text.replace(f'\ue000INLINE{i}\ue001', inline)

        return text

    def _highlight_with_pygments(self, code: str, lang: str, pygments_mod: dict) -> str:
        """Highlight code using Pygments with Pango markup output."""
        get_lexer_by_name = pygments_mod["get_lexer_by_name"]
        TextLexer = pygments_mod["TextLexer"]
        ClassNotFound = pygments_mod["ClassNotFound"]

        # Map common language aliases
        lang_map = {
            "sh": "bash",
            "shell": "bash",
            "zsh": "bash",
            "": "bash",  # Default to bash for terminal
            "py": "python",
        }
        lang = lang_map.get(lang.lower(), lang.lower())

        try:
            lexer = get_lexer_by_name(lang)
        except ClassNotFound:
            lexer = TextLexer()

        # Use terminal palette colors if available, otherwise use Dracula
        if self._palette and len(self._palette) >= 8:
            # Map terminal palette to Pygments tokens
            # 0=black, 1=red, 2=green, 3=yellow, 4=blue, 5=magenta, 6=cyan, 7=white
            # 8-15 are bright variants
            colors = {
                "Token.Keyword": self._palette[5]
                if len(self._palette) > 5
                else "#ff79c6",  # Magenta
                "Token.Keyword.Namespace": self._palette[5]
                if len(self._palette) > 5
                else "#ff79c6",
                "Token.Keyword.Constant": self._palette[5]
                if len(self._palette) > 5
                else "#ff79c6",
                "Token.Keyword.Declaration": self._palette[5]
                if len(self._palette) > 5
                else "#ff79c6",
                "Token.Keyword.Pseudo": self._palette[5]
                if len(self._palette) > 5
                else "#ff79c6",
                "Token.Keyword.Reserved": self._palette[5]
                if len(self._palette) > 5
                else "#ff79c6",
                "Token.Keyword.Type": self._palette[6]
                if len(self._palette) > 6
                else "#8be9fd",  # Cyan
                "Token.Name.Builtin": self._palette[2]
                if len(self._palette) > 2
                else "#50fa7b",  # Green
                "Token.Name.Function": self._palette[2]
                if len(self._palette) > 2
                else "#50fa7b",
                "Token.Name.Class": self._palette[2]
                if len(self._palette) > 2
                else "#50fa7b",
                "Token.Name.Decorator": self._palette[2]
                if len(self._palette) > 2
                else "#50fa7b",
                "Token.Name.Variable": self._palette[6]
                if len(self._palette) > 6
                else "#8be9fd",  # Cyan
                "Token.Name.Variable.Global": self._palette[6]
                if len(self._palette) > 6
                else "#8be9fd",
                "Token.Name.Variable.Instance": self._palette[6]
                if len(self._palette) > 6
                else "#8be9fd",
                # String tokens
                "Token.String": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",  # Yellow
                "Token.String.Doc": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.String.Double": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.String.Single": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.String.Backtick": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.String.Interpol": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.String.Escape": self._palette[11]
                if len(self._palette) > 11
                else "#ffb86c",  # Bright yellow
                # Literal tokens
                "Token.Literal": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.Literal.String": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.Literal.String.Double": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.Literal.String.Single": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.Literal.String.Backtick": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.Literal.String.Doc": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.Literal.String.Escape": self._palette[11]
                if len(self._palette) > 11
                else "#ffb86c",
                "Token.Literal.String.Interpol": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.Literal.String.Heredoc": self._palette[3]
                if len(self._palette) > 3
                else "#f1fa8c",
                "Token.Literal.Number": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",  # Magenta
                "Token.Literal.Number.Integer": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",
                "Token.Literal.Number.Float": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",
                "Token.Literal.Number.Hex": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",
                "Token.Literal.Number.Oct": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",
                "Token.Literal.Number.Bin": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",
                # Number tokens
                "Token.Number": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",
                "Token.Number.Integer": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",
                "Token.Number.Float": self._palette[5]
                if len(self._palette) > 5
                else "#bd93f9",
                # Comment tokens
                "Token.Comment": self._palette[8]
                if len(self._palette) > 8
                else "#6272a4",  # Bright black (gray)
                "Token.Comment.Single": self._palette[8]
                if len(self._palette) > 8
                else "#6272a4",
                "Token.Comment.Multiline": self._palette[8]
                if len(self._palette) > 8
                else "#6272a4",
                "Token.Comment.Hashbang": self._palette[8]
                if len(self._palette) > 8
                else "#6272a4",
                "Token.Comment.Preproc": self._palette[8]
                if len(self._palette) > 8
                else "#6272a4",
                # Operator tokens
                "Token.Operator": self._palette[5]
                if len(self._palette) > 5
                else "#ff79c6",  # Magenta
                "Token.Operator.Word": self._palette[5]
                if len(self._palette) > 5
                else "#ff79c6",
                "Token.Punctuation": self._palette[7]
                if len(self._palette) > 7
                else "#f8f8f2",  # White
            }
        else:
            style_manager = Adw.StyleManager.get_default()
            is_dark = style_manager.get_dark()
            if is_dark:
                # Default Dracula color scheme for dark theme
                colors = {
                    "Token.Keyword": "#ff79c6",
                    "Token.Keyword.Namespace": "#ff79c6",
                    "Token.Keyword.Constant": "#ff79c6",
                    "Token.Keyword.Declaration": "#ff79c6",
                    "Token.Keyword.Pseudo": "#ff79c6",
                    "Token.Keyword.Reserved": "#ff79c6",
                    "Token.Keyword.Type": "#8be9fd",
                    "Token.Name.Builtin": "#50fa7b",
                    "Token.Name.Function": "#50fa7b",
                    "Token.Name.Class": "#50fa7b",
                    "Token.Name.Decorator": "#50fa7b",
                    "Token.Name.Variable": "#8be9fd",
                    "Token.Name.Variable.Global": "#8be9fd",
                    "Token.Name.Variable.Instance": "#8be9fd",
                    # String tokens (various pygments token paths)
                    "Token.String": "#f1fa8c",
                    "Token.String.Doc": "#f1fa8c",
                    "Token.String.Double": "#f1fa8c",
                    "Token.String.Single": "#f1fa8c",
                    "Token.String.Backtick": "#f1fa8c",
                    "Token.String.Interpol": "#f1fa8c",
                    "Token.String.Escape": "#ffb86c",
                    # Literal tokens
                    "Token.Literal": "#f1fa8c",
                    "Token.Literal.String": "#f1fa8c",
                    "Token.Literal.String.Double": "#f1fa8c",
                    "Token.Literal.String.Single": "#f1fa8c",
                    "Token.Literal.String.Backtick": "#f1fa8c",
                    "Token.Literal.String.Doc": "#f1fa8c",
                    "Token.Literal.String.Escape": "#ffb86c",
                    "Token.Literal.String.Interpol": "#f1fa8c",
                    "Token.Literal.String.Heredoc": "#f1fa8c",
                    "Token.Literal.Number": "#bd93f9",
                    "Token.Literal.Number.Integer": "#bd93f9",
                    "Token.Literal.Number.Float": "#bd93f9",
                    "Token.Literal.Number.Hex": "#bd93f9",
                    "Token.Literal.Number.Oct": "#bd93f9",
                    "Token.Literal.Number.Bin": "#bd93f9",
                    # Number tokens
                    "Token.Number": "#bd93f9",
                    "Token.Number.Integer": "#bd93f9",
                    "Token.Number.Float": "#bd93f9",
                    # Comment tokens
                    "Token.Comment": "#6272a4",
                    "Token.Comment.Single": "#6272a4",
                    "Token.Comment.Multiline": "#6272a4",
                    "Token.Comment.Hashbang": "#6272a4",
                    "Token.Comment.Preproc": "#6272a4",
                    # Operator tokens
                    "Token.Operator": "#ff79c6",
                    "Token.Operator.Word": "#ff79c6",
                    "Token.Punctuation": "#f8f8f2",
                }
            else:
                # High contrast color scheme for light theme
                colors = {
                    "Token.Keyword": "#cf222e",
                    "Token.Keyword.Namespace": "#cf222e",
                    "Token.Keyword.Constant": "#0550ae",
                    "Token.Keyword.Declaration": "#cf222e",
                    "Token.Keyword.Pseudo": "#cf222e",
                    "Token.Keyword.Reserved": "#cf222e",
                    "Token.Keyword.Type": "#953800",
                    "Token.Name.Builtin": "#8250df",
                    "Token.Name.Function": "#116d3d",
                    "Token.Name.Class": "#953800",
                    "Token.Name.Decorator": "#8250df",
                    "Token.Name.Variable": "#0550ae",
                    "Token.Name.Variable.Global": "#0550ae",
                    "Token.Name.Variable.Instance": "#0550ae",
                    # String tokens
                    "Token.String": "#0a3069",
                    "Token.String.Doc": "#0a3069",
                    "Token.String.Double": "#0a3069",
                    "Token.String.Single": "#0a3069",
                    "Token.String.Backtick": "#0a3069",
                    "Token.String.Interpol": "#0a3069",
                    "Token.String.Escape": "#953800",
                    # Literal tokens
                    "Token.Literal": "#0a3069",
                    "Token.Literal.String": "#0a3069",
                    "Token.Literal.String.Double": "#0a3069",
                    "Token.Literal.String.Single": "#0a3069",
                    "Token.Literal.String.Backtick": "#0a3069",
                    "Token.Literal.String.Doc": "#0a3069",
                    "Token.Literal.String.Escape": "#953800",
                    "Token.Literal.String.Interpol": "#0a3069",
                    "Token.Literal.String.Heredoc": "#0a3069",
                    "Token.Literal.Number": "#0550ae",
                    "Token.Literal.Number.Integer": "#0550ae",
                    "Token.Literal.Number.Float": "#0550ae",
                    "Token.Literal.Number.Hex": "#0550ae",
                    "Token.Literal.Number.Oct": "#0550ae",
                    "Token.Literal.Number.Bin": "#0550ae",
                    # Number tokens
                    "Token.Number": "#0550ae",
                    "Token.Number.Integer": "#0550ae",
                    "Token.Number.Float": "#0550ae",
                    # Comment tokens
                    "Token.Comment": "#57606a",
                    "Token.Comment.Single": "#57606a",
                    "Token.Comment.Multiline": "#57606a",
                    "Token.Comment.Hashbang": "#57606a",
                    "Token.Comment.Preproc": "#57606a",
                    # Operator tokens
                    "Token.Operator": "#cf222e",
                    "Token.Operator.Word": "#cf222e",
                    "Token.Punctuation": "#24292f",
                }


        # Tokenize and build Pango markup
        # Use the already lazy-loaded pygments module
        pygments = pygments_mod["pygments"]
        result = []
        for token_type, token_value in pygments.lex(code, lexer):
            # Escape for Pango markup
            escaped = GLib.markup_escape_text(token_value)

            # Find matching color (check parent token types too)
            color = None
            token_str = str(token_type)

            # Try exact match first, then progressively shorter prefixes
            while token_str and not color:
                if token_str in colors:
                    color = colors[token_str]
                else:
                    # Try parent token type
                    if "." in token_str:
                        token_str = token_str.rsplit(".", 1)[0]
                    else:
                        break

            if color:
                result.append(f'<span foreground="{color}">{escaped}</span>')
            else:
                result.append(escaped)

        return "".join(result)

    def _get_syntax_colors(self) -> dict:
        """Get syntax highlighting colors based on current theme (light/dark)."""
        # Check if we're in light or dark mode
        style_manager = Adw.StyleManager.get_default()
        is_dark = style_manager.get_dark()

        if is_dark:
            # Dracula-inspired colors for dark theme
            return {
                "keyword": "#ff79c6",      # Pink for keywords
                "string": "#f1fa8c",       # Yellow for strings
                "comment": "#6272a4",      # Blue-gray for comments
                "number": "#bd93f9",       # Purple for numbers
                "function": "#50fa7b",     # Green for functions/commands
                "variable": "#8be9fd",     # Cyan for variables
                "flag": "#ffb86c",         # Orange for flags
            }
        else:
            # Light theme colors - darker, high contrast for light backgrounds
            return {
                "keyword": "#ab296a",      # Darker magenta for keywords
                "string": "#0a3069",       # Dark navy for strings
                "comment": "#57606a",      # Dark gray for comments
                "number": "#0550ae",       # Dark blue for numbers
                "function": "#116d3d",     # Dark green for functions/commands
                "variable": "#0550ae",     # Dark blue for variables
                "flag": "#953800",         # Dark orange/brown for flags
            }


    def _highlight_fallback(self, code: str, lang: str) -> str:
        """Fallback regex-based syntax highlighting.

        This method handles raw (unescaped) code and produces valid Pango markup.
        Uses a token-based approach to properly handle escaping.
        Adapts colors for light/dark themes.
        """
        # Get colors based on current theme
        colors = self._get_syntax_colors()

        # Define token patterns for shell/bash (most common for terminal commands)
        if lang in ("bash", "sh", "shell", "zsh", ""):
            patterns = [
                # Comments - must be first
                (r'#[^\n]*', 'comment'),
                # Double-quoted strings
                (r'"(?:[^"\\]|\\.)*"', 'string'),
                # Single-quoted strings
                (r"'(?:[^'\\]|\\.)*'", 'string'),
                # Variables $VAR and ${VAR}
                (r'\$\{?[\w]+\}?', 'variable'),
                # Flags/options (--flag or -f)
                (r'(?<!\w)--?[\w-]+', 'flag'),
                # Shell keywords
                (r'\b(?:if|then|else|elif|fi|for|while|do|done|case|esac|in|function|return|exit|export|source|alias|unset|local|readonly)\b', 'keyword'),
                # Common commands (expanded list)
                (r'\b(?:sudo|cd|ls|cat|echo|grep|awk|sed|find|xargs|chmod|chown|cp|mv|rm|mkdir|touch|head|tail|sort|uniq|wc|cut|tr|tee|man|which|whereis|apt|apt-get|apt-cache|dpkg|pacman|yay|paru|pip|pip3|npm|npx|yarn|pnpm|git|docker|docker-compose|podman|kubectl|systemctl|journalctl|curl|wget|tar|gzip|gunzip|zip|unzip|ssh|scp|rsync|kill|killall|pkill|ps|top|htop|btop|df|du|free|mount|umount|ln|pwd|date|cal|whoami|hostname|uname|clear|history|alias|export|env|set|bash|zsh|sh|fish|python|python3|node|ruby|perl|make|cmake|gcc|g\+\+|clang|cargo|rustc|go|java|javac|nano|vim|nvim|vi|emacs|code|less|more|diff|patch|install|update|upgrade|remove|purge|autoremove|search|info|show|list|status|start|stop|restart|enable|disable|reload|reboot|shutdown|poweroff|suspend|hibernate|chroot|exec|nohup|screen|tmux|watch|time|timeout|sleep|true|false|test|read|printf|pushd|popd|dirs|fg|bg|jobs|disown|wait|trap|break|continue|shift|getopts|eval|source|type|command|builtin|hash|help|logout|exit|return|declare|typeset|let|readonly|local|global|unset|shopt|complete|compgen|compopt|mapfile|readarray|coproc|select|until|ulimit|umask|fc|bind|caller|enable|mapfile|readarray|times)\b', 'function'),
                # Numbers
                (r'\b\d+\b', 'number'),
            ]
        elif lang in ("python", "py"):
            patterns = [
                # Comments
                (r'#[^\n]*', 'comment'),
                # Triple-quoted strings
                (r'"""[\s\S]*?"""|\'\'\'[\s\S]*?\'\'\'', 'string'),
                # Double-quoted strings
                (r'"(?:[^"\\]|\\.)*"', 'string'),
                # Single-quoted strings
                (r"'(?:[^'\\]|\\.)*'", 'string'),
                # Decorators
                (r'@[\w.]+', 'function'),
                # Keywords
                (r'\b(?:def|class|if|elif|else|for|while|try|except|finally|with|as|import|from|return|yield|raise|pass|break|continue|and|or|not|in|is|lambda|True|False|None|async|await|global|nonlocal)\b', 'keyword'),
                # Built-in functions
                (r'\b(?:print|len|range|str|int|float|list|dict|set|tuple|open|type|isinstance|hasattr|getattr|setattr|delattr|repr|abs|all|any|bin|bool|bytes|callable|chr|complex|dir|divmod|enumerate|eval|exec|filter|format|frozenset|globals|hash|hex|id|input|iter|locals|map|max|min|next|object|oct|ord|pow|property|reversed|round|slice|sorted|staticmethod|sum|super|vars|zip)\b', 'function'),
                # Numbers
                (r'\b\d+\.?\d*\b', 'number'),
            ]
        elif lang == "json":
            patterns = [
                # Keys
                (r'"[\w_-]+"(?=\s*:)', 'variable'),
                # String values
                (r'"(?:[^"\\]|\\.)*"', 'string'),
                # Booleans and null
                (r'\b(?:true|false|null)\b', 'keyword'),
                # Numbers
                (r'\b\d+\.?\d*\b', 'number'),
            ]
        else:
            # No highlighting for unknown languages
            return GLib.markup_escape_text(code)

        # Build a combined pattern with named groups
        combined_parts = []
        for i, (pattern, token_type) in enumerate(patterns):
            combined_parts.append(f'(?P<t{i}>{pattern})')
        combined_pattern = '|'.join(combined_parts)

        # Process the code and build highlighted output
        result = []
        last_end = 0

        for match in re.finditer(combined_pattern, code):
            # Add non-matched text before this match (escaped)
            if match.start() > last_end:
                result.append(GLib.markup_escape_text(code[last_end:match.start()]))

            # Find which group matched and get its token type
            matched_text = match.group(0)
            token_type = None
            for i, (_pattern, ttype) in enumerate(patterns):
                if match.group(f't{i}') is not None:
                    token_type = ttype
                    break

            # Add highlighted text (escaped)
            escaped_text = GLib.markup_escape_text(matched_text)
            if token_type and token_type in colors:
                result.append(f'<span foreground="{colors[token_type]}">{escaped_text}</span>')
            else:
                result.append(escaped_text)

            last_end = match.end()

        # Add any remaining text after the last match
        if last_end < len(code):
            result.append(GLib.markup_escape_text(code[last_end:]))

        return ''.join(result)

    def _highlight_code_for_label(self, code: str, lang: str) -> str:
        """Highlight code for use in labels (handles escaping).

        Both pygments and fallback handle escaping internally.
        For shell/bash languages, prefer the fallback as it has better
        recognition of common terminal commands.
        """
        # Normalize language
        lang_lower = lang.lower() if lang else ""

        # For shell/bash, prefer fallback highlighting as it recognizes
        # common terminal commands better than Pygments' BashLexer
        if lang_lower in ("bash", "sh", "shell", "zsh", ""):
            return self._highlight_fallback(code, lang)

        # For other languages, use pygments if available
        pygments_mod = _get_pygments()
        if pygments_mod:
            return self._highlight_with_pygments(code, lang, pygments_mod)

        # Fallback for all other cases
        return self._highlight_fallback(code, lang)

    def _add_command_buttons(self):
        """Add buttons for each detected command or ActionStep with visual risk cards."""
        if not self._commands:
            return

        # Commands section container
        commands_section = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        commands_section.set_margin_start(8)
        commands_section.set_margin_end(8)
        commands_section.set_margin_top(12)

        # Section header
        section_header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        terminal_icon = icon_image("utilities-terminal-symbolic")
        terminal_icon.add_css_class("ai-section-icon")
        section_header.append(terminal_icon)

        section_label = Gtk.Label(label=_("Plano de Ações do Agente"))
        section_label.add_css_class("ai-section-title")
        section_header.append(section_label)
        commands_section.append(section_header)

        for item in self._commands[:10]:
            if isinstance(item, str):
                argv = item.split()
                try:
                    from ...agent.policy_engine import PolicyEngine
                    pe = PolicyEngine()
                    risk = int(pe.classify(argv))
                except Exception:
                    risk = 0
                step_data = {
                    "tool": "shell.run",
                    "argv": argv,
                    "command_str": item,
                    "description": "",
                    "risk": risk,
                    "approval": "blocked" if risk >= 4 else ("polkit" if risk == 2 else "click"),
                }
            elif hasattr(item, "to_dict"):
                step_data = item.to_dict()
                step_data["command_str"] = " ".join(item.argv) if item.argv else item.tool
            elif isinstance(item, dict):
                step_data = item
                step_data["command_str"] = " ".join(item.get("argv", [])) if item.get("argv") else (item.get("command", "") or item.get("tool", ""))
            else:
                continue

            cmd_str = step_data.get("command_str", "")
            risk_val = int(step_data.get("risk", 0))
            approval_val = step_data.get("approval", "click")
            tool_name = step_data.get("tool", "shell.run")
            description = step_data.get("description", "")

            # Step card container
            step_card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
            step_card.add_css_class("ai-step-card")

            # Header row: Risk Badge + Tool name
            header_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)

            risk_badge = Gtk.Label()
            risk_badge_texts = {
                0: _("🟢 Nível 0 (Leitura)"),
                1: _("🔵 Nível 1 (Escrita)"),
                2: _("🟠 Nível 2 (Admin Polkit)"),
                3: _("🔴 Nível 3 (Crítico)"),
                4: _("⛔ Nível 4 (Bloqueado)"),
            }
            risk_badge.set_label(risk_badge_texts.get(risk_val, f"Nível {risk_val}"))
            risk_badge.add_css_class("ai-risk-badge")
            risk_badge.add_css_class(f"ai-risk-badge-{min(4, max(0, risk_val))}")
            header_row.append(risk_badge)

            tool_label = Gtk.Label(label=f"[{tool_name}]")
            tool_label.add_css_class("dim-label")
            header_row.append(tool_label)

            step_card.append(header_row)

            # Command text row
            cmd_row = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            cmd_label = Gtk.Label()
            cmd_label.set_xalign(0)
            cmd_label.set_hexpand(True)
            cmd_label.set_wrap(True)
            cmd_label.set_wrap_mode(Pango.WrapMode.WORD_CHAR)
            if hasattr(Gtk, "NaturalWrapMode") and hasattr(cmd_label, "set_natural_wrap_mode"):
                cmd_label.set_natural_wrap_mode(Gtk.NaturalWrapMode.WORD)
            cmd_label.add_css_class("ai-command-text")
            cmd_label.set_selectable(True)
            cmd_label.set_markup(self._highlight_code_for_label(cmd_str, "bash"))
            cmd_row.append(cmd_label)

            # Action buttons
            buttons_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
            buttons_box.set_valign(Gtk.Align.CENTER)
            buttons_box.add_css_class("ai-cmd-buttons")

            if approval_val == "blocked" or risk_val >= 4:
                blocked_btn = Gtk.Button(label=_("⛔ Bloqueado"))
                blocked_btn.set_sensitive(False)
                blocked_btn.add_css_class("flat")
                self._add_tooltip(blocked_btn, _("Comando bloqueado pelas políticas de segurança."))
                buttons_box.append(blocked_btn)
            elif approval_val == "diff" or tool_name in {"fs.propose_edit", "fs.write_staged_file"}:
                diff_btn = Gtk.Button()
                diff_btn.set_label(_("👁 Ver Alterações"))
                diff_btn.add_css_class("suggested-action")
                diff_btn.connect("clicked", self._on_diff_clicked, step_data)
                self._add_tooltip(diff_btn, _("Revisar diff unificado antes de aplicar com backup"))
                buttons_box.append(diff_btn)
            elif approval_val == "polkit" or risk_val == 2:
                admin_btn = Gtk.Button()
                admin_btn.set_label(_("🛡 Executar como Admin"))
                admin_btn.add_css_class("destructive-action")
                admin_btn.connect("clicked", self._on_run_clicked, cmd_str)
                self._add_tooltip(admin_btn, _("Executar ação administrativa autenticada via Polkit"))
                buttons_box.append(admin_btn)
            else:
                run_btn = Gtk.Button()
                run_btn.set_icon_name("media-playback-start-symbolic")
                run_btn.add_css_class("flat")
                run_btn.add_css_class("circular")
                run_btn.add_css_class("ai-cmd-btn-run")
                run_btn.connect("clicked", self._on_run_clicked, cmd_str)
                self._add_tooltip(run_btn, _("Executar comando"))
                buttons_box.append(run_btn)

                dry_run_argv = step_data.get("dry_run_argv")
                if dry_run_argv:
                    dry_run_str = " ".join(dry_run_argv)
                    dry_btn = Gtk.Button()
                    dry_btn.set_icon_name("media-playlist-repeat-symbolic")
                    dry_btn.add_css_class("flat")
                    dry_btn.add_css_class("circular")
                    dry_btn.add_css_class("ai-cmd-btn")
                    dry_btn.connect("clicked", self._on_run_clicked, dry_run_str)
                    self._add_tooltip(dry_btn, _("Simular execução (Dry-run)"))
                    buttons_box.append(dry_btn)

                insert_btn = Gtk.Button()
                insert_btn.set_icon_name("edit-paste-symbolic")
                insert_btn.add_css_class("flat")
                insert_btn.add_css_class("circular")
                insert_btn.add_css_class("ai-cmd-btn")
                insert_btn.connect("clicked", self._on_execute_clicked, cmd_str)
                self._add_tooltip(insert_btn, _("Inserir no terminal"))
                buttons_box.append(insert_btn)

            copy_btn = Gtk.Button()
            copy_btn.set_icon_name("edit-copy-symbolic")
            copy_btn.add_css_class("flat")
            copy_btn.add_css_class("circular")
            copy_btn.add_css_class("ai-cmd-btn")
            copy_btn.connect("clicked", self._on_copy_clicked, cmd_str)
            self._add_tooltip(copy_btn, _("Copiar comando"))
            buttons_box.append(copy_btn)

            cmd_row.append(buttons_box)
            step_card.append(cmd_row)

            if description:
                expander = Gtk.Expander(label=_("O que esta etapa faz?"))
                desc_label = Gtk.Label(label=description)
                desc_label.set_wrap(True)
                desc_label.set_xalign(0)
                desc_label.set_margin_top(4)
                desc_label.set_margin_bottom(4)
                desc_label.add_css_class("dim-label")
                expander.set_child(desc_label)
                step_card.append(expander)

            commands_section.append(step_card)

        self.append(commands_section)

    def _on_diff_clicked(self, _button: Gtk.Button, step_data: dict) -> None:
        """Open diff review dialog for proposed edit."""
        from ..dialogs.diff_review_dialog import DiffReviewDialog
        target_path = step_data.get("path") or step_data.get("working_directory") or "arquivo"
        diff_text = step_data.get("diff") or "--- a/config\n+++ b/config\n@@ -1 +1 @@\n-old\n+new"

        def apply_callback(create_backup: bool):
            cmd = f"# Aplicado com backup={create_backup}\n" + step_data.get("command_str", "")
            self.emit("run-command", cmd)

        root = self.get_root()
        dialog = DiffReviewDialog(root, target_path=str(target_path), diff_text=diff_text, on_apply=apply_callback)
        dialog.present()

    def _add_code_block_actions(self, container: Gtk.Box) -> None:
        """Add dedicated copy/insert buttons for code blocks in the message."""
        if self._role != "assistant":
            return
        matches = _CODE_BLOCK_PATTERN.findall(self._content)
        if not matches:
            return

        for i, (lang, code) in enumerate(matches):
            code_clean = code.strip()
            if not code_clean:
                continue
            lang_display = lang.lower() if lang else "code"
            card = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
            card.add_css_class("ai-code-block-action")
            card.set_margin_top(4)
            card.set_margin_bottom(2)

            badge = Gtk.Label(label=f"📜 {lang_display}")
            badge.add_css_class("dim-label")
            badge.add_css_class("caption")
            card.append(badge)

            btn_spacer = Gtk.Box(hexpand=True)
            card.append(btn_spacer)

            copy_code_btn = Gtk.Button()
            copy_code_btn.set_label(_("📋 Copiar Código"))
            copy_code_btn.add_css_class("flat")
            copy_code_btn.add_css_class("suggested-action")
            copy_code_btn.connect("clicked", self._on_copy_clicked, code_clean)
            self._add_tooltip(copy_code_btn, _("Copiar bloco de código/script para a área de transferência"))
            card.append(copy_code_btn)

            if lang_display in {"bash", "sh", "zsh", "shell"} and len(code_clean.splitlines()) == 1:
                insert_btn = Gtk.Button()
                insert_btn.set_icon_name("edit-paste-symbolic")
                insert_btn.add_css_class("flat")
                insert_btn.connect("clicked", self._on_execute_clicked, code_clean)
                self._add_tooltip(insert_btn, _("Inserir no terminal"))
                card.append(insert_btn)

            container.append(card)

    def _on_run_clicked(self, button: Gtk.Button, command: str):
        """Emit signal to run command directly."""
        self.emit("run-command", command)

    def _on_execute_clicked(self, button: Gtk.Button, command: str):
        """Emit signal to execute command."""
        self.emit("execute-command", command)

    def _on_copy_clicked(self, button: Gtk.Button, command: str):
        """Copy command or code to clipboard with visual confirmation."""
        clipboard = button.get_clipboard()
        clipboard.set(command)
        old_icon = button.get_icon_name()
        old_label = button.get_label()
        if old_icon:
            button.set_icon_name("emblem-ok-symbolic")
            GLib.timeout_add(1500, lambda: button.set_icon_name(old_icon))
        elif old_label:
            button.set_label(_("✓ Copiado!"))
            GLib.timeout_add(1500, lambda: button.set_label(old_label))

    def _on_copy_full_message(self, button: Gtk.Button):
        """Copy the entire message text to clipboard."""
        self._on_copy_clicked(button, self._content)

    def update_content(self, content: str, commands: list[str] | None = None):
        """Update the message content (for streaming)."""
        self._content = content
        formatted_content = self._format_content(content)

        # Try to set markup, fallback to plain text if markup parsing fails
        try:
            self._label.set_markup(formatted_content)
        except Exception:
            # Markup parsing failed, fallback to plain text
            self._label.set_text(content)

        # Update commands if provided
        if commands and commands != self._commands:
            self._commands = commands
            # Remove old command buttons if any (skip header and content box)
            children = list(self)
            for child in children[2:]:  # Skip header box and content box
                self.remove(child)
            self._add_command_buttons()


# Register signals for MessageBubble
GObject.signal_new(
    "execute-command",
    MessageBubble,
    GObject.SignalFlags.RUN_LAST,
    GObject.TYPE_NONE,
    (GObject.TYPE_STRING,)
)

GObject.signal_new(
    "run-command",
    MessageBubble,
    GObject.SignalFlags.RUN_LAST,
    GObject.TYPE_NONE,
    (GObject.TYPE_STRING,)
)


class AIChatPanel(Gtk.Box):
    """Persistent AI chat panel overlay."""

    __gsignals__ = {
        "execute-command": (GObject.SignalFlags.RUN_LAST, None, (str,)),
        "run-command": (GObject.SignalFlags.RUN_LAST, None, (str,)),
        "close-requested": (GObject.SignalFlags.RUN_LAST, None, ()),
    }

    def __init__(
        self, ai_assistant: AIAssistant, tooltip_helper=None, settings_manager=None
    ):
        super().__init__(orientation=Gtk.Orientation.VERTICAL)
        self._ai_assistant = ai_assistant
        self._history_manager = ai_assistant._history_manager
        self._settings_manager = settings_manager
        self._current_assistant_bubble: MessageBubble | None = None
        self._quick_prompts = get_random_quick_prompts(6)

        # Retry support state
        self._last_request_message: str | None = None
        self._raw_streaming_content: str = ""

        # Minimum height for the panel, Paned handles resize
        self.set_size_request(-1, 200)
        self.set_vexpand(True)  # Expand in paned
        self.add_css_class("ai-chat-panel")

        self._setup_ui()
        self._connect_signals()
        self._apply_css()
        self._apply_transparency()

        # Load existing conversation if any
        self._load_conversation()

    def _add_tooltip(self, widget: Gtk.Widget, text: str):
        """Add tooltip to widget using custom helper or fallback to standard."""
        # Ensure tooltip is enabled (may have been disabled to force-close popup)
        widget.set_has_tooltip(True)
        helper = get_tooltip_helper()
        if helper:
            helper.add_tooltip(widget, text)
        else:
            widget.set_tooltip_text(text)

    def _setup_ui(self):
        """Build the chat panel UI."""
        # Header bar
        header = Adw.HeaderBar()
        header.add_css_class("ai-panel-header")
        header.set_show_end_title_buttons(False)
        header.set_show_start_title_buttons(False)

        # Title
        title_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        title_label = Gtk.Label(label=_("AI Assistant"))
        title_label.add_css_class("title")
        title_box.append(title_label)

        header.set_title_widget(title_box)

        # New chat button (document-new-symbolic not in bundled icons, use system)
        new_chat_btn = Gtk.Button()
        new_chat_btn.set_icon_name("document-new-symbolic")
        new_chat_btn.add_css_class("flat")
        new_chat_btn.connect("clicked", self._on_new_chat)
        self._add_tooltip(new_chat_btn, _("New conversation"))
        header.pack_start(new_chat_btn)

        # History button (document-open-recent-symbolic not in bundled icons, use system)
        history_btn = Gtk.Button()
        history_btn.set_icon_name("document-open-recent-symbolic")
        history_btn.add_css_class("flat")
        history_btn.connect("clicked", self._on_show_history)
        self._add_tooltip(history_btn, _("View history"))
        header.pack_start(history_btn)

        # Scope / Security Policies button
        scope_btn = Gtk.Button()
        scope_btn.set_icon_name("security-high-symbolic")
        scope_btn.add_css_class("flat")
        scope_btn.connect("clicked", self._on_show_scope)
        self._add_tooltip(scope_btn, _("Políticas de Segurança e Escopo do Agente"))
        header.pack_start(scope_btn)

        # Audit log button
        audit_btn = Gtk.Button()
        audit_btn.set_icon_name("document-properties-symbolic")
        audit_btn.add_css_class("flat")
        audit_btn.connect("clicked", self._on_show_audit_log)
        self._add_tooltip(audit_btn, _("Registro de Auditoria de Ações"))
        header.pack_start(audit_btn)

        # Export conversation menu button
        export_btn = Gtk.MenuButton()
        export_btn.set_icon_name("document-save-symbolic")
        export_btn.add_css_class("flat")
        self._add_tooltip(export_btn, _("Export conversation"))

        export_popover = Gtk.Popover()
        export_menu_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        export_menu_box.set_margin_top(6)
        export_menu_box.set_margin_bottom(6)
        export_menu_box.set_margin_start(6)
        export_menu_box.set_margin_end(6)

        # 1. Export as Markdown
        md_btn = Gtk.Button()
        md_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        md_icon = icon_image("text-x-generic-symbolic") or Gtk.Image.new_from_icon_name("text-x-generic-symbolic")
        md_label = Gtk.Label(label=_("Export as Markdown (.md)"))
        md_box.append(md_icon)
        md_box.append(md_label)
        md_btn.set_child(md_box)
        md_btn.add_css_class("flat")
        md_btn.connect("clicked", lambda b: (export_popover.popdown(), self._export_conversation_as("md")))
        export_menu_box.append(md_btn)

        # 2. Export as JSON
        json_btn = Gtk.Button()
        json_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        json_icon = icon_image("text-x-script-symbolic") or Gtk.Image.new_from_icon_name("text-x-script-symbolic")
        json_label = Gtk.Label(label=_("Export as JSON (.json)"))
        json_box.append(json_icon)
        json_box.append(json_label)
        json_btn.set_child(json_box)
        json_btn.add_css_class("flat")
        json_btn.connect("clicked", lambda b: (export_popover.popdown(), self._export_conversation_as("json")))
        export_menu_box.append(json_btn)

        # Separator
        export_menu_box.append(Gtk.Separator(orientation=Gtk.Orientation.HORIZONTAL))

        # 3. Copy to Clipboard
        copy_btn = Gtk.Button()
        copy_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        copy_icon = icon_image("edit-copy-symbolic") or Gtk.Image.new_from_icon_name("edit-copy-symbolic")
        copy_label = Gtk.Label(label=_("Copy to Clipboard"))
        copy_box.append(copy_icon)
        copy_box.append(copy_label)
        copy_btn.set_child(copy_box)
        copy_btn.add_css_class("flat")
        copy_btn.connect("clicked", lambda b: (export_popover.popdown(), self._copy_conversation_to_clipboard()))
        export_menu_box.append(copy_btn)

        export_popover.set_child(export_menu_box)
        export_btn.set_popover(export_popover)
        header.pack_start(export_btn)


        # Close button (uses bundled icon)
        close_btn = Gtk.Button()
        close_btn.set_child(icon_image("window-close-symbolic"))
        close_btn.add_css_class("flat")
        close_btn.connect("clicked", lambda b: self.emit("close-requested"))
        self._add_tooltip(close_btn, _("Close panel"))
        header.pack_end(close_btn)

        self.append(header)

        # Chat content area with scrolling
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scrolled.set_min_content_height(100)  # Minimum height to prevent layout issues

        self._messages_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self._messages_box.set_margin_start(8)
        self._messages_box.set_margin_end(8)
        self._messages_box.set_margin_top(8)
        self._messages_box.set_margin_bottom(8)

        scrolled.set_child(self._messages_box)
        self._scrolled = scrolled
        self.append(scrolled)

        # Loading indicator
        self._loading = LoadingIndicator()
        self._loading.set_visible(False)
        self._loading.set_margin_start(16)
        self._loading.set_margin_end(16)
        self._loading.set_margin_bottom(8)
        self.append(self._loading)

        # Quick prompts container with header
        quick_prompts_container = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)

        # Header with title and customize button
        prompts_header = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        prompts_header.set_margin_start(12)
        prompts_header.set_margin_end(8)

        prompts_title = Gtk.Label(label=_("Quick Prompts"))
        prompts_title.add_css_class("dim-label")
        prompts_title.set_xalign(0)
        prompts_title.set_hexpand(True)
        prompts_header.append(prompts_title)

        customize_btn = Gtk.Button()
        customize_btn.set_icon_name("emblem-system-symbolic")
        customize_btn.add_css_class("flat")
        customize_btn.add_css_class("circular")
        customize_btn.connect("clicked", self._on_customize_prompts)
        self._add_tooltip(customize_btn, _("Customize quick prompts"))
        prompts_header.append(customize_btn)

        quick_prompts_container.append(prompts_header)

        # Quick prompts area (shown when no messages)
        self._quick_prompts_box = Gtk.FlowBox()
        self._quick_prompts_box.set_selection_mode(Gtk.SelectionMode.NONE)
        self._quick_prompts_box.set_max_children_per_line(3)
        self._quick_prompts_box.set_min_children_per_line(4)
        self._quick_prompts_box.set_margin_start(8)
        self._quick_prompts_box.set_margin_end(8)
        self._quick_prompts_box.set_margin_bottom(8)
        self._populate_quick_prompts()
        quick_prompts_container.append(self._quick_prompts_box)

        self._quick_prompts_container = quick_prompts_container
        self.append(quick_prompts_container)

        # Input area with multi-line text view
        input_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        input_box.set_margin_start(8)
        input_box.set_margin_end(8)
        input_box.set_margin_bottom(8)
        input_box.set_size_request(-1, 30)  # Minimum height to prevent negative allocation
        input_box.add_css_class("ai-input-box")

        # Create a scrolled window for the text view
        text_scroll = Gtk.ScrolledWindow()
        text_scroll.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        text_scroll.set_min_content_height(24)  # Start as single line
        text_scroll.set_max_content_height(120)  # Max height before scrolling
        text_scroll.set_propagate_natural_height(True)
        text_scroll.set_hexpand(True)

        # Multi-line text view
        self._text_view = Gtk.TextView()
        self._text_view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self._text_view.set_accepts_tab(False)  # Tab should not insert tab character
        self._text_view.add_css_class("ai-input-textview")

        # Get the buffer for text operations
        self._text_buffer = self._text_view.get_buffer()

        # Handle key press for Enter to send (Shift+Enter for newline)
        key_controller = Gtk.EventControllerKey()
        key_controller.connect("key-pressed", self._on_key_pressed)
        self._text_view.add_controller(key_controller)

        # Auto-resize based on content
        self._text_buffer.connect("changed", self._on_text_changed)

        text_scroll.set_child(self._text_view)
        input_box.append(text_scroll)

        # Keep reference for text scroll widget
        self._text_scroll = text_scroll

        self._send_btn = Gtk.Button()
        self._send_btn.set_child(icon_image("go-up-symbolic"))
        self._send_btn.add_css_class("suggested-action")
        self._send_btn.add_css_class("circular")
        self._send_btn.set_valign(Gtk.Align.CENTER)  # Vertically center aligned
        self._send_btn.connect("clicked", self._on_send)
        self._add_tooltip(self._send_btn, _("Send message"))
        input_box.append(self._send_btn)

        self.append(input_box)

    def _on_text_changed(self, buffer):
        """Handle text buffer changes for auto-resize."""
        # Just trigger a queue_resize to allow natural height propagation
        self._text_view.queue_resize()

    def _on_key_pressed(self, controller, keyval, _keycode, state):
        """Handle key press events for the text view."""
        # Escape key closes the panel
        if keyval == Gdk.KEY_Escape:
            self.emit("close-requested")
            return True  # Event handled

        # Check for Enter key without Shift
        if keyval == Gdk.KEY_Return or keyval == Gdk.KEY_KP_Enter:
            # Shift+Enter = newline, Enter alone = send
            if not (state & Gdk.ModifierType.SHIFT_MASK):
                self._on_send(self._text_view)
                return True  # Event handled
        return False  # Let the event propagate

    def _populate_quick_prompts(self):
        """Fill the quick prompts area with buttons."""
        for child in list(self._quick_prompts_box):
            self._quick_prompts_box.remove(child)

        # Check for custom prompts in settings
        prompts_to_use = self._quick_prompts
        if self._settings_manager:
            custom_prompts = self._settings_manager.get("ai_custom_quick_prompts", [])
            if custom_prompts:
                prompts_to_use = [
                    (p.get("emoji", "💬"), p.get("text", ""))
                    for p in custom_prompts
                    if p.get("text")
                ]

        for icon, text in prompts_to_use:
            btn = Gtk.Button()
            btn_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)

            icon_label = Gtk.Label(label=icon)
            btn_box.append(icon_label)

            text_label = Gtk.Label(label=text)
            text_label.set_ellipsize(Pango.EllipsizeMode.END)
            text_label.set_max_width_chars(20)
            btn_box.append(text_label)

            btn.set_child(btn_box)
            btn.add_css_class("flat")
            btn.connect("clicked", self._on_quick_prompt_clicked, text)
            self._add_tooltip(btn, text)
            self._quick_prompts_box.append(btn)

    def _connect_signals(self):
        """Connect to AI assistant signals and theme changes."""
        self._ai_assistant.connect("streaming-chunk", self._on_streaming_chunk)
        self._ai_assistant.connect("response-ready", self._on_response_ready)
        self._ai_assistant.connect("error", self._on_error)

        # Listen for theme changes to update styles
        style_manager = Adw.StyleManager.get_default()
        style_manager.connect("notify::dark", self._on_theme_changed)

    def _on_theme_changed(self, style_manager, param):
        """Handle theme change (light/dark) to update styles."""
        logger.debug("Theme changed, reapplying AI chat panel styles")
        self._apply_transparency()

    def _apply_css(self):
        """Apply custom CSS for the chat panel from external file."""
        css_provider = Gtk.CssProvider()
        css_file = _STYLES_DIR / "ai_chat_panel.css"

        if css_file.exists():
            css_provider.load_from_path(str(css_file))
            logger.debug(f"Loaded AI chat panel CSS from {css_file}")
        else:
            logger.warning(f"AI chat panel CSS file not found: {css_file}")

        Gtk.StyleContext.add_provider_for_display(
            self.get_display(),
            css_provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    def _apply_transparency(self):
        """Apply background transparency to the AI chat panel.

        This method ensures:
        1. The panel background is transparent (respecting user settings)
        2. Chat content (bubbles, input) have solid opaque backgrounds for readability
        3. Colors adapt to light/dark theme
        """
        try:
            if not self._settings_manager:
                return

            # Detect theme
            style_manager = Adw.StyleManager.get_default()
            is_dark = style_manager.get_dark()

            transparency = self._settings_manager.get("headerbar_transparency", 0)

            # Determine base color for panel background
            gtk_theme = self._settings_manager.get("gtk_theme", "")
            if gtk_theme == "terminal":
                scheme = self._settings_manager.get_color_scheme_data()
                base_color_hex = scheme.get("background", "#000000" if is_dark else "#ffffff")
                fg_color_hex = scheme.get(
                    "foreground", "#ffffff" if is_dark else "#000000"
                )
                header_bg = scheme.get("headerbar_background", base_color_hex)
                # Get accent color from palette (typically blue at index 4)
                palette = scheme.get("palette", [])
                accent_color = palette[4] if len(palette) > 4 else "#3584e4"
            else:
                base_color_hex = "#1e1e1e" if is_dark else "#f6f5f4"
                fg_color_hex = "#ffffff" if is_dark else "#000000"
                header_bg = base_color_hex
                accent_color = "#3584e4"

            # Parse hex color for panel background
            r = int(base_color_hex[1:3], 16)
            g = int(base_color_hex[3:5], 16)
            b = int(base_color_hex[5:7], 16)

            # Calculate alpha for panel background transparency
            if transparency > 0:
                alpha = max(0.0, min(1.0, 1.0 - (transparency / 100.0) ** 1.6))
                rgba_bg = f"rgba({r}, {g}, {b}, {alpha})"
            else:
                rgba_bg = f"rgb({r}, {g}, {b})"

            # Define solid opaque colors for content areas based on theme
            if gtk_theme == "terminal":
                command_bg = "#1e1e1e" if is_dark else "#f6f8fa"
                command_fg = "#e0e0e0" if is_dark else "#1f2328"
                command_border = "rgba(255, 255, 255, 0.1)" if is_dark else "rgba(0, 0, 0, 0.12)"
                command_hover_bg = "#2d2d2d" if is_dark else "#eef1f5"

                # Terminal theme - use colors from terminal scheme
                bubble_user_bg = accent_color
                # For user bubble text, check if accent is dark enough for white text
                ar = int(accent_color[1:3], 16)
                ag = int(accent_color[3:5], 16)
                ab = int(accent_color[5:7], 16)
                accent_luminance = (0.299 * ar + 0.587 * ag + 0.114 * ab) / 255
                bubble_user_fg = "#ffffff" if accent_luminance < 0.5 else "#000000"
                bubble_assistant_bg = header_bg
                bubble_assistant_border = (
                    f"color-mix(in srgb, {fg_color_hex} 10%, transparent)"
                )
                input_bg = header_bg
                input_border = f"color-mix(in srgb, {fg_color_hex} 10%, transparent)"
                scroll_bg = (
                    f"rgba({r}, {g}, {b}, 0.3)" if transparency > 0 else "transparent"
                )
                content_fg = fg_color_hex
            elif is_dark:
                command_bg = "#1e1e1e"
                command_fg = "#e0e0e0"
                command_border = "rgba(255, 255, 255, 0.1)"
                command_hover_bg = "#2d2d2d"

                # Dark theme colors - Modern dark palette
                bubble_user_bg = "#3584e4"  # Accent blue for user
                bubble_user_fg = "#ffffff"
                bubble_assistant_bg = "#2d2d2d"  # Dark card background
                bubble_assistant_border = "rgba(255, 255, 255, 0.1)"
                input_bg = "#2d2d2d"
                input_border = "rgba(255, 255, 255, 0.1)"
                scroll_bg = f"rgba({r}, {g}, {b}, 0.3)" if transparency > 0 else "transparent"
                content_fg = "#ffffff"
            else:
                command_bg = "#f6f8fa"
                command_fg = "#1f2328"
                command_border = "rgba(0, 0, 0, 0.12)"
                command_hover_bg = "#eef1f5"

                # Light theme colors - Clean light palette
                bubble_user_bg = "#3584e4"  # Same accent blue
                bubble_user_fg = "#ffffff"
                bubble_assistant_bg = "#ffffff"  # Pure white for assistant
                bubble_assistant_border = "rgba(0, 0, 0, 0.08)"
                input_bg = "#ffffff"
                input_border = "rgba(0, 0, 0, 0.12)"
                scroll_bg = f"rgba({r}, {g}, {b}, 0.3)" if transparency > 0 else "transparent"
                content_fg = "#000000"

            # Build comprehensive CSS for transparent panel with solid content
            css = f"""
            /* Panel background - transparent or opaque based on setting */
            .ai-chat-panel {{
                background-color: {rgba_bg};
                color: {content_fg};
            }}

            /* Scrolled area - subtle background for depth */
            .ai-chat-panel scrolledwindow {{
                background-color: {scroll_bg};
            }}

            /* User message bubble - always solid and visible */
            .ai-message-user {{
                background-color: {bubble_user_bg};
                background-image: linear-gradient(135deg, {bubble_user_bg}, shade({bubble_user_bg}, 0.92));
                color: {bubble_user_fg};
                border-radius: 16px 16px 4px 16px;
                padding: 10px 14px;
                box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
            }}

            /* Assistant message bubble - always solid */
            .ai-message-assistant {{
                background-color: {bubble_assistant_bg};
                color: {content_fg};
                border: 1px solid {bubble_assistant_border};
                border-radius: 16px 16px 16px 4px;
                padding: 10px 14px;
                box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
            }}

            /* Command block and step cards - responsive to light/dark themes */
            .ai-command-block {{
                background-color: {command_bg};
                color: {command_fg};
                border: 1px solid {command_border};
                border-radius: 10px;
                padding: 12px 14px;
                transition: all 200ms ease;
            }}
            .ai-command-block:hover {{
                background-color: {command_hover_bg};
                border-color: alpha(@accent_color, 0.4);
                box-shadow: 0 2px 8px alpha(@accent_color, 0.1);
            }}
            .ai-command-text {{
                color: {command_fg};
            }}
            .ai-step-card {{
                background-color: {command_bg};
                border: 1px solid {command_border};
                color: {command_fg};
            }}
            .ai-step-card:hover {{
                background-color: {command_hover_bg};
                border-color: alpha(@accent_color, 0.5);
            }}


            /* Input area - solid background for visibility */
            .ai-input-box {{
                background-color: {input_bg};
                color: {content_fg};
                border: 1px solid {input_border};
                border-radius: 14px;
                padding: 6px 10px;
                transition: border-color 200ms ease, box-shadow 200ms ease;
            }}
            .ai-input-box:focus-within {{
                border-color: @accent_color;
                box-shadow: 0 0 0 2px alpha(@accent_color, 0.2);
            }}
            .ai-input-textview {{
                background-color: transparent;
                color: {content_fg};
                padding: 4px;
                min-height: 24px;
            }}
            .ai-input-textview text {{
                background-color: transparent;
                color: {content_fg};
            }}
            
            /* AI Panel HeaderBar */
            .ai-panel-header {{
                background-color: {input_bg};
                color: {content_fg};
            }}
            .ai-panel-header .title {{
                color: {content_fg};
            }}
            .ai-panel-header button {{
                color: {content_fg};
            }}
            .ai-panel-header button image {{
                color: {content_fg};
            }}
            """

            # Remove existing provider if any
            if hasattr(self, "_transparency_provider"):
                try:
                    Gtk.StyleContext.remove_provider_for_display(
                        Gdk.Display.get_default(), self._transparency_provider
                    )
                except Exception:
                    pass

            provider = Gtk.CssProvider()
            provider.load_from_data(css.encode("utf-8"))
            Gtk.StyleContext.add_provider_for_display(
                Gdk.Display.get_default(),
                provider,
                Gtk.STYLE_PROVIDER_PRIORITY_USER,  # Higher priority to override base CSS
            )
            self._transparency_provider = provider
            theme_type = "dark" if is_dark else "light"
            logger.info(f"AI chat panel styles applied: {theme_type} theme, transparency={transparency}%")
        except Exception as e:
            logger.warning(f"Failed to apply transparency to AI chat panel: {e}")

    def update_transparency(self):
        """Public method to update transparency when settings change."""
        self._apply_transparency()

    def _load_conversation(self):
        """Load existing conversation from history."""
        conversation = self._history_manager.get_current_conversation()
        if not conversation:
            return

        messages = conversation.get("messages", [])
        if messages:
            self._quick_prompts_container.set_visible(False)
            for msg in messages:
                # Normalize commands from history (may be list of dicts or strings)
                commands = _normalize_commands(msg.get("commands"))
                self._add_message_bubble(msg["role"], msg["content"], commands)

    def _add_message_bubble(self, role: str, content: str, commands: list | None = None) -> MessageBubble:
        """Add a message bubble to the chat."""
        # Normalize commands to list of strings
        normalized_commands = _normalize_commands(commands)
        bubble = MessageBubble(
            role, content, normalized_commands, settings_manager=self._settings_manager
        )
        bubble.connect("execute-command", self._on_bubble_execute)
        bubble.connect("run-command", self._on_bubble_run)
        self._messages_box.append(bubble)

        # Scroll to bottom
        GLib.idle_add(self._scroll_to_bottom)

        return bubble

    def _scroll_to_bottom(self):
        """Scroll the chat to the bottom."""
        adj = self._scrolled.get_vadjustment()
        adj.set_value(adj.get_upper() - adj.get_page_size())
        return False

    def _scroll_to_bottom_delayed(self):
        """Scroll to bottom with delay to allow layout to settle."""
        # First immediate scroll
        GLib.idle_add(self._scroll_to_bottom)
        # Then delayed scroll to catch layout changes (e.g., when commands appear)
        GLib.timeout_add(50, self._scroll_to_bottom)
        GLib.timeout_add(150, self._scroll_to_bottom)

    def _get_input_text(self) -> str:
        """Get text from the input text view."""
        start = self._text_buffer.get_start_iter()
        end = self._text_buffer.get_end_iter()
        text = self._text_buffer.get_text(start, end, False)
        return text.strip()

    def _set_input_text(self, text: str):
        """Set text in the input text view."""
        self._text_buffer.set_text(text)

    def _on_send(self, widget):
        """Handle send button click or Enter key."""
        text = self._get_input_text()
        if not text:
            return

        # Hide any visible tooltip on the send button immediately
        helper = get_tooltip_helper()
        if helper:
            helper.hide()

        # Store message for retry support
        self._last_request_message = text

        # Automatic redaction of passwords, tokens and secrets
        from ...agent.redactor import redact_secrets
        text_to_send, num_redacted = redact_secrets(text)
        if num_redacted > 0:
            logger.info(f"AI Chat redactor filtered {num_redacted} secrets before sending.")

        self._text_buffer.set_text("")
        self._text_view.set_sensitive(False)
        self._send_btn.set_sensitive(False)
        self._quick_prompts_container.set_visible(False)

        # Initialize raw streaming content tracker
        self._raw_streaming_content = ""

        # Add user message
        self._add_message_bubble("user", text)

        # Start loading indicator
        self._loading.start()

        # Create placeholder for assistant response
        self._current_assistant_bubble = self._add_message_bubble("assistant", "")

        # Send to AI using request_assistance_simple for panel context
        self._ai_assistant.request_assistance_simple(
            text_to_send,
            streaming_callback=self._handle_streaming_chunk
        )

    def _on_quick_prompt_clicked(self, button: Gtk.Button, text: str):
        """Handle quick prompt button click."""
        self._set_input_text(text)
        self._on_send(button)

    def _on_streaming_chunk(self, _assistant, chunk: str, is_done: bool):
        """Handle streaming chunk from AI (GObject signal handler)."""
        GLib.idle_add(self._ui_update_streaming_chunk, chunk, is_done)

    def _handle_streaming_chunk(self, chunk: str, is_done: bool):
        """Handle streaming chunk from AI (thread callback handler)."""
        GLib.idle_add(self._ui_update_streaming_chunk, chunk, is_done)

    def _ui_update_streaming_chunk(self, chunk: str, is_done: bool) -> bool:
        """Safely update UI with streaming chunk on the GTK main thread."""
        try:
            if not is_done and self._current_assistant_bubble:
                self._raw_streaming_content += chunk
                display_content = _extract_reply_from_json(self._raw_streaming_content)
                self._current_assistant_bubble.update_content(display_content)
                self._scroll_to_bottom()
            elif is_done:
                self._raw_streaming_content = ""
        except Exception as e:
            logger = get_logger("onyxsh.ui.chat")
            logger.warning(f"Error updating streaming chunk: {e}")
        return False

    def _on_response_ready(self, _assistant, response: str, commands):
        """Handle complete response from AI."""
        self._loading.stop()
        # Reset raw content tracker
        self._raw_streaming_content = ""

        # Clean up the response - remove any trailing JSON arrays
        clean_response = _extract_reply_from_json(response)
        if not clean_response:
            clean_response = response  # Fallback if extraction returns empty

        # Normalize commands to list of strings
        commands_list = _normalize_commands(list(commands) if commands else [])

        if self._current_assistant_bubble:
            self._current_assistant_bubble.update_content(clean_response, commands_list)
            self._current_assistant_bubble = None

        # Re-enable input AFTER updating content
        self._text_view.set_sensitive(True)
        self._send_btn.set_sensitive(True)
        # Restore tooltip
        self._add_tooltip(self._send_btn, _("Send message"))

        # Scroll to bottom with delay to allow command buttons to render
        self._scroll_to_bottom_delayed()

    def _on_error(self, _assistant, error_msg: str):
        """Handle error from AI with retry option."""
        self._loading.stop()
        # Reset raw content tracker
        self._raw_streaming_content = ""

        if self._current_assistant_bubble:
            # Remove the empty assistant bubble
            self._messages_box.remove(self._current_assistant_bubble)
            self._current_assistant_bubble = None

        # Re-enable input
        self._text_view.set_sensitive(True)
        self._send_btn.set_sensitive(True)
        # Restore tooltip
        self._add_tooltip(self._send_btn, _("Send message"))

        # Create error message box with retry button
        error_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        error_box.add_css_class("ai-message-assistant")
        error_box.set_margin_start(8)
        error_box.set_margin_end(8)

        # Error icon and message
        error_content = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        error_content.set_margin_start(8)
        error_content.set_margin_end(8)
        error_content.set_margin_top(8)

        error_icon = Gtk.Image.new_from_icon_name("dialog-warning-symbolic")
        error_icon.add_css_class("warning")
        error_content.append(error_icon)

        error_label = Gtk.Label(label=error_msg)
        error_label.set_wrap(True)
        error_label.set_xalign(0)
        error_label.set_hexpand(True)
        error_content.append(error_label)

        error_box.append(error_content)

        # Retry button (only if we have a message to retry)
        if self._last_request_message:
            retry_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
            retry_box.set_halign(Gtk.Align.END)
            retry_box.set_margin_end(8)
            retry_box.set_margin_bottom(8)

            retry_btn = Gtk.Button()
            retry_btn_content = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
            retry_icon = icon_image("view-refresh-symbolic")
            retry_btn_content.append(retry_icon)
            retry_label = Gtk.Label(label=_("Retry"))
            retry_btn_content.append(retry_label)
            retry_btn.set_child(retry_btn_content)
            retry_btn.add_css_class("suggested-action")
            retry_btn.connect("clicked", self._on_retry_clicked, error_box)
            self._add_tooltip(retry_btn, _("Retry the last request"))

            retry_box.append(retry_btn)
            error_box.append(retry_box)

        self._messages_box.append(error_box)
        GLib.idle_add(self._scroll_to_bottom)

    def _on_bubble_execute(self, bubble: MessageBubble, command: str):
        """Handle execute command from a bubble (insert into terminal)."""
        self.emit("execute-command", command)

    def _on_bubble_run(self, bubble: MessageBubble, command: str):
        """Handle run command from a bubble (execute in terminal)."""
        self.emit("run-command", command)

    def _on_retry_clicked(self, button: Gtk.Button, error_box: Gtk.Box):
        """Handle retry button click - resend the last request."""
        if not self._last_request_message:
            return

        # Remove the error box
        self._messages_box.remove(error_box)

        # Disable input while processing
        self._text_view.set_sensitive(False)
        self._send_btn.set_sensitive(False)

        # Initialize raw streaming content tracker
        self._raw_streaming_content = ""

        # Start loading indicator
        self._loading.start()

        # Create placeholder for assistant response
        self._current_assistant_bubble = self._add_message_bubble("assistant", "")

        # Resend the same message
        self._ai_assistant.request_assistance_simple(
            self._last_request_message,
            streaming_callback=self._handle_streaming_chunk
        )

    def _on_customize_prompts(self, button: Gtk.Button):
        """Show dialog to customize quick prompts."""
        dialog = Adw.Dialog()
        dialog.set_title(_("Customize Quick Prompts"))
        dialog.set_content_width(500)
        dialog.set_content_height(450)

        # Main content box
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)

        # Header bar for the dialog
        header = Adw.HeaderBar()
        header.set_show_end_title_buttons(False)
        header.set_show_start_title_buttons(False)

        cancel_btn = Gtk.Button(label=_("Cancel"))
        cancel_btn.connect("clicked", lambda b: dialog.close())
        header.pack_start(cancel_btn)

        save_btn = Gtk.Button(label=_("Save"))
        save_btn.add_css_class("suggested-action")
        header.pack_end(save_btn)

        main_box.append(header)

        # Scrolled window for the list
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)

        # List box for prompts
        list_box = Gtk.ListBox()
        list_box.set_selection_mode(Gtk.SelectionMode.NONE)
        list_box.add_css_class("boxed-list")
        list_box.set_margin_start(12)
        list_box.set_margin_end(12)
        list_box.set_margin_top(12)
        list_box.set_margin_bottom(12)

        # Load existing custom prompts or empty list
        custom_prompts = []
        if self._settings_manager:
            custom_prompts = self._settings_manager.get("ai_custom_quick_prompts", [])

        # Store row references for saving
        prompt_rows = []

        def create_prompt_row(emoji: str = "", text: str = "") -> Gtk.ListBoxRow:
            """Create a row for editing a prompt."""
            row = Gtk.ListBoxRow()
            row_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
            row_box.set_margin_start(8)
            row_box.set_margin_end(8)
            row_box.set_margin_top(8)
            row_box.set_margin_bottom(8)

            # Emoji entry (small)
            emoji_entry = Gtk.Entry()
            emoji_entry.set_placeholder_text("🔧")
            emoji_entry.set_text(emoji)
            emoji_entry.set_max_length(4)
            emoji_entry.set_width_chars(4)
            self._add_tooltip(emoji_entry, _("Emoji icon (optional)"))
            row_box.append(emoji_entry)

            # Text entry (expands)
            text_entry = Gtk.Entry()
            text_entry.set_placeholder_text(_("Enter prompt text..."))
            text_entry.set_text(text)
            text_entry.set_hexpand(True)
            row_box.append(text_entry)

            # Delete button (uses bundled icon)
            delete_btn = Gtk.Button()
            delete_btn.set_child(icon_image("user-trash-symbolic"))
            delete_btn.add_css_class("flat")
            delete_btn.add_css_class("destructive-action")
            self._add_tooltip(delete_btn, _("Remove this prompt"))

            def on_delete(btn):
                prompt_rows.remove((row, emoji_entry, text_entry))
                list_box.remove(row)

            delete_btn.connect("clicked", on_delete)
            row_box.append(delete_btn)

            row.set_child(row_box)
            prompt_rows.append((row, emoji_entry, text_entry))
            return row

        # Add existing prompts
        for prompt in custom_prompts:
            row = create_prompt_row(prompt.get("emoji", ""), prompt.get("text", ""))
            list_box.append(row)

        scrolled.set_child(list_box)
        main_box.append(scrolled)

        # Add button at bottom
        add_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        add_box.set_halign(Gtk.Align.CENTER)
        add_box.set_margin_top(8)
        add_box.set_margin_bottom(12)

        add_btn = Gtk.Button()
        add_btn_content = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        add_icon = icon_image("list-add-symbolic")
        add_btn_content.append(add_icon)
        add_label = Gtk.Label(label=_("Add Prompt"))
        add_btn_content.append(add_label)
        add_btn.set_child(add_btn_content)
        add_btn.add_css_class("suggested-action")
        add_btn.connect("clicked", lambda b: list_box.append(create_prompt_row()))
        add_box.append(add_btn)

        # Clear all button
        clear_btn = Gtk.Button()
        clear_btn_content = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        clear_icon = Gtk.Image.new_from_icon_name("edit-clear-all-symbolic")
        clear_btn_content.append(clear_icon)
        clear_label = Gtk.Label(label=_("Use Defaults"))
        clear_btn_content.append(clear_label)
        clear_btn.set_child(clear_btn_content)
        clear_btn.set_margin_start(12)
        self._add_tooltip(clear_btn, _("Clear custom prompts and use random defaults"))

        def on_clear(btn):
            # Remove all rows
            for row, _entry, _label in list(prompt_rows):
                list_box.remove(row)
            prompt_rows.clear()

        clear_btn.connect("clicked", on_clear)
        add_box.append(clear_btn)

        main_box.append(add_box)

        # Save handler
        def on_save(btn):
            # Collect all prompts
            new_prompts = []
            for row, emoji_entry, text_entry in prompt_rows:
                text = text_entry.get_text().strip()
                if text:  # Only save non-empty prompts
                    new_prompts.append({
                        "emoji": emoji_entry.get_text().strip() or "💬",
                        "text": text
                    })

            # Save to settings
            if self._settings_manager:
                self._settings_manager.set("ai_custom_quick_prompts", new_prompts)

            # Refresh the quick prompts display
            self._populate_quick_prompts()

            dialog.close()

        save_btn.connect("clicked", on_save)

        dialog.set_child(main_box)
        dialog.present(self.get_root())

    def _on_new_chat(self, button: Gtk.Button):
        """Start a new conversation."""
        # Clear current messages
        for child in list(self._messages_box):
            self._messages_box.remove(child)

        # Start new conversation in history
        self._history_manager.new_conversation()

        # Refresh quick prompts with new random selection
        self._quick_prompts = get_random_quick_prompts(6)
        self._populate_quick_prompts()
        self._quick_prompts_container.set_visible(True)

        self._current_assistant_bubble = None

    def _on_show_history(self, button: Gtk.Button):
        """Show conversation history panel."""
        # Create a fresh history panel each time (widgets can't be reparented)
        history_panel = ConversationHistoryPanel(self._history_manager)
        history_panel.connect("conversation-selected", self._on_history_conversation_selected)
        history_panel.connect("close-requested", self._on_history_close)
        history_panel.connect("conversation-deleted", self._on_history_conversation_deleted)

        # Create a dialog window for the history panel
        dialog = Adw.Dialog()
        dialog.set_content_width(450)
        dialog.set_content_height(550)
        dialog.set_child(history_panel)

        # Store reference to close it programmatically
        self._history_dialog = dialog

        dialog.present(self.get_root())

    def _on_history_conversation_selected(
        self, _panel: ConversationHistoryPanel, conv_id: str
    ):
        """Handle conversation selection from history panel."""
        self._history_manager.load_conversation(conv_id)
        self._refresh_conversation()

        # Close the history dialog
        if hasattr(self, "_history_dialog") and self._history_dialog:
            self._history_dialog.close()
            self._history_dialog = None

    def _on_history_conversation_deleted(
        self, _panel: ConversationHistoryPanel, conv_id: str
    ):
        """Handle conversation deletion from history panel."""
        # Empty conv_id means all conversations were deleted
        if not conv_id or conv_id == self._history_manager._current_conversation_id:
            # Start a new conversation
            self._history_manager.new_conversation()
            self._refresh_conversation()

    def _on_history_close(self, _panel: ConversationHistoryPanel):
        """Handle close button from history panel."""
        # Close the history dialog
        if hasattr(self, "_history_dialog") and self._history_dialog:
            self._history_dialog.close()
            self._history_dialog = None

    def _refresh_conversation(self):
        """Refresh the display with current conversation."""
        # Clear messages
        for child in list(self._messages_box):
            self._messages_box.remove(child)

        self._quick_prompts_container.set_visible(False)
        self._load_conversation()

    def set_initial_text(self, text: str):
        """Set initial text in the input field."""
        self._set_input_text(text)
        self._text_view.grab_focus()

    def _on_show_scope(self, _button):
        """Open the agent scope and policies dialog."""
        from ..dialogs.agent_scope_dialog import AgentScopeDialog
        root = self.get_root()
        dialog = AgentScopeDialog(root, self._settings_manager)
        dialog.present()

    def _on_show_audit_log(self, _button):
        """Open the agent action audit log dialog."""
        from ..dialogs.audit_log_dialog import AuditLogDialog
        root = self.get_root()
        dialog = AuditLogDialog(root)
        dialog.present()

    def _show_toast(self, message: str) -> None:
        """Show a toast message on the root window's toast overlay."""
        root = self.get_root()
        if root and hasattr(root, "toast_overlay") and root.toast_overlay:
            root.toast_overlay.add_toast(Adw.Toast(title=message))
        else:
            logger.info(f"Toast: {message}")

    def _format_conversation_markdown(self) -> str:
        """Format the current conversation as Markdown."""
        messages = self._history_manager.get_history()
        if not messages:
            return ""

        conv = self._history_manager.get_current_conversation()
        created_at = (
            conv.get("created_at", datetime.now().isoformat())
            if conv
            else datetime.now().isoformat()
        )
        conv_id = conv.get("id", "current") if conv else "current"

        lines = [
            f"# OnyxSH AI Chat Export",
            f"",
            f"- **Date / Timestamp:** {created_at}",
            f"- **Conversation ID:** `{conv_id}`",
            f"- **Total Messages:** {len(messages)}",
            f"",
            f"---",
            f"",
        ]

        for msg in messages:
            role = msg.get("role", "user")
            timestamp = msg.get("timestamp", "")
            content = msg.get("content", "").strip()
            commands = msg.get("commands", [])

            if role == "user":
                lines.append(f"## 👤 User ({timestamp})")
                lines.append("")
                lines.append(content)
                lines.append("")
            else:
                lines.append(f"## 🤖 Assistant ({timestamp})")
                lines.append("")
                lines.append(content)
                lines.append("")
                if commands:
                    lines.append("### ⚡ Commands / Actions:")
                    for cmd in commands:
                        if isinstance(cmd, str):
                            lines.append(f"```bash\n{cmd}\n```")
                        elif isinstance(cmd, dict):
                            cmd_str = (
                                cmd.get("command_str")
                                or cmd.get("command")
                                or " ".join(cmd.get("argv", []))
                                or cmd.get("tool", "")
                            )
                            risk = cmd.get("risk", 0)
                            lines.append(f"- **Risk Level {risk}**:")
                            lines.append(f"```bash\n{cmd_str}\n```")
                    lines.append("")
            lines.append("---")
            lines.append("")

        return "\n".join(lines)

    def _format_conversation_json(self) -> str:
        """Format the current conversation as structured JSON."""
        conv = self._history_manager.get_current_conversation()
        if not conv or not conv.get("messages"):
            messages = self._history_manager.get_history()
            if not messages:
                return ""
            conv_data = {
                "id": str(uuid.uuid4()),
                "created_at": datetime.now().isoformat(),
                "exported_at": datetime.now().isoformat(),
                "messages": messages,
            }
        else:
            conv_data = dict(conv)
            conv_data["exported_at"] = datetime.now().isoformat()

        return json.dumps(conv_data, ensure_ascii=False, indent=2)

    def _copy_conversation_to_clipboard(self) -> None:
        """Copy current conversation transcript to the system clipboard."""
        markdown_text = self._format_conversation_markdown()
        if not markdown_text:
            self._show_toast(_("No messages to export."))
            return

        display = Gdk.Display.get_default()
        if display:
            clipboard = display.get_clipboard()
            clipboard.set(markdown_text)
            self._show_toast(_("Conversation copied to clipboard."))

    def _export_conversation_as(self, fmt: str) -> None:
        """Export conversation to a file via file chooser dialog."""
        if fmt == "json":
            content = self._format_conversation_json()
            ext = ".json"
            filter_name = _("JSON files (*.json)")
            mime_type = "application/json"
            pattern = "*.json"
        else:
            content = self._format_conversation_markdown()
            ext = ".md"
            filter_name = _("Markdown files (*.md)")
            mime_type = "text/markdown"
            pattern = "*.md"

        if not content:
            self._show_toast(_("No messages to export."))
            return

        timestamp_str = datetime.now().strftime("%Y%m%d_%H%M%S")
        default_filename = f"zash_ai_chat_{timestamp_str}{ext}"

        file_dialog = Gtk.FileDialog(
            title=_("Export AI Conversation"),
            modal=True,
        )
        file_dialog.set_initial_name(default_filename)

        # Configure file filters
        filters = Gio.ListStore.new(Gtk.FileFilter)

        target_filter = Gtk.FileFilter()
        target_filter.set_name(filter_name)
        target_filter.add_pattern(pattern)
        target_filter.add_mime_type(mime_type)
        filters.append(target_filter)

        all_filter = Gtk.FileFilter()
        all_filter.set_name(_("All files"))
        all_filter.add_pattern("*")
        filters.append(all_filter)

        file_dialog.set_filters(filters)
        file_dialog.set_default_filter(target_filter)

        root = self.get_root()
        parent_window = root if isinstance(root, Gtk.Window) else None

        def on_save_finish(dialog, result):
            try:
                gfile = dialog.save_finish(result)
                if gfile:
                    filepath = gfile.get_path()
                    if filepath:
                        with open(filepath, "w", encoding="utf-8") as f:
                            f.write(content)
                        self._show_toast(_("Conversation exported successfully."))
            except GLib.Error as e:
                # User cancelled dialog
                if e.matches(Gtk.DialogError.quark(), Gtk.DialogError.DISMISSED):
                    return
                logger.error(f"Failed to export conversation: {e}")
                self._show_toast(_("Failed to export conversation: {}").format(e.message))
            except Exception as e:
                logger.error(f"Error saving exported conversation: {e}")
                self._show_toast(_("Failed to export conversation: {}").format(str(e)))

        file_dialog.save(parent_window, None, on_save_finish)

