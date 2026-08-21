# onyxsh/filemanager/quick_look.py
"""
Quick Look preview dialog for OnyxSH File Manager.

Allows instant file inspection on Space key press or context menu action:
- Syntax highlighted code/scripts (.sh, .py, .json, .yaml, .md, .c, .rs, etc.)
- Log file viewer with error/warning level highlighting
- Image viewer (.png, .jpg, .svg, .webp, .ico, etc.) with dimensions metadata
- Binary/archive metadata card with hex inspection
- Keyboard navigation (Space/Esc to close, Up/Down for next/prev file)
"""

import binascii
import mimetypes
import os
from pathlib import Path
from typing import Callable, List, Optional, Tuple

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk, Pango

from ..ui.dialogs.base_dialog import BaseDialog
from ..utils.logger import get_logger
from ..utils.translation_utils import _
from .models import FileItem

# Max sample size to read for preview (256 KB)
MAX_PREVIEW_BYTES = 256 * 1024
MAX_PREVIEW_LINES = 2000

# Supported image extensions
IMAGE_EXTENSIONS = {
    ".png",
    ".jpg",
    ".jpeg",
    ".svg",
    ".ico",
    ".webp",
    ".bmp",
    ".gif",
    ".tiff",
}

# Supported markdown/docs extensions
MARKDOWN_EXTENSIONS = {".md", ".markdown", ".mdown", ".rst", ".txt"}


class QuickLookDialog(BaseDialog):
    """Modern Libadwaita Quick Look preview dialog."""

    def __init__(
        self,
        parent_window: Gtk.Window,
        on_open_editor: Optional[Callable[[FileItem, str], None]] = None,
        on_navigate: Optional[Callable[[int], Optional[Tuple[FileItem, str]]]] = None,
    ) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Quick Look"),
            auto_setup_toolbar=True,
            default_width=780,
            default_height=580,
        )
        self.logger = get_logger("onyxsh.filemanager.quick_look")
        self.on_open_editor = on_open_editor
        self.on_navigate = on_navigate

        self.current_item: Optional[FileItem] = None
        self.current_folder: str = ""
        self._current_text_content: str = ""
        self._is_loading = False

        self._setup_ui()
        self._setup_keyboard_shortcuts()

    def _setup_ui(self) -> None:
        """Construct the headerbar widgets and stack content view."""
        self.add_css_class("quick-look-dialog")

        # Custom headerbar widgets
        self.title_label = Gtk.Label(label="", xalign=0)
        self.title_label.add_css_class("title-4")
        self.title_label.set_ellipsize(Pango.EllipsizeMode.END)

        self.subtitle_label = Gtk.Label(label="", xalign=0)
        self.subtitle_label.add_css_class("dim-label")
        self.subtitle_label.add_css_class("caption")

        title_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        title_box.set_valign(Gtk.Align.CENTER)
        title_box.append(self.title_label)
        title_box.append(self.subtitle_label)

        if self._header_bar:
            self._header_bar.set_title_widget(title_box)

            # Copy Content Button
            self.copy_btn = Gtk.Button(icon_name="edit-copy-symbolic")
            self.copy_btn.set_tooltip_text(_("Copy Content"))
            self.copy_btn.add_css_class("flat")
            self.copy_btn.connect("clicked", self._on_copy_clicked)
            self._header_bar.pack_end(self.copy_btn)

            # Open in Editor Button
            self.open_editor_btn = Gtk.Button(
                label=_("Open in Editor"),
                icon_name="document-edit-symbolic",
            )
            self.open_editor_btn.add_css_class("suggested-action")
            self.open_editor_btn.connect("clicked", self._on_open_editor_clicked)
            self._header_bar.pack_end(self.open_editor_btn)

        # Main View Stack
        self.stack = Gtk.Stack()
        self.stack.set_transition_type(Gtk.StackTransitionType.CROSSFADE)

        # Page 1: Loading
        loading_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=16,
            valign=Gtk.Align.CENTER,
            halign=Gtk.Align.CENTER,
        )
        self.spinner = Gtk.Spinner()
        self.spinner.set_size_request(36, 36)
        loading_label = Gtk.Label(label=_("Loading preview..."))
        loading_label.add_css_class("dim-label")
        loading_box.append(self.spinner)
        loading_box.append(loading_label)
        self.stack.add_named(loading_box, "loading")

        # Page 2: Text / Code Preview
        text_container = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)

        # Truncation banner if needed
        self.truncated_banner = Adw.Banner(
            title=_("Content truncated for preview. Open in editor to view full file."),
            revealed=False,
        )
        text_container.append(self.truncated_banner)

        # Scrolled Text View
        scrolled_text = Gtk.ScrolledWindow(vexpand=True, hexpand=True)
        self.text_view = Gtk.TextView(
            editable=False,
            cursor_visible=True,
            wrap_mode=Gtk.WrapMode.NONE,
            monospace=True,
            left_margin=16,
            right_margin=16,
            top_margin=12,
            bottom_margin=12,
        )
        self.text_view.add_css_class("quick-look-text-view")
        self.text_buffer = self.text_view.get_buffer()
        self._init_syntax_tags(self.text_buffer)

        scrolled_text.set_child(self.text_view)
        text_container.append(scrolled_text)

        # Bottom info bar for text (lines, encoding, size)
        self.text_info_label = Gtk.Label(label="", xalign=0)
        self.text_info_label.add_css_class("caption")
        self.text_info_label.add_css_class("dim-label")
        self.text_info_label.set_margin_start(16)
        self.text_info_label.set_margin_end(16)
        self.text_info_label.set_margin_top(6)
        self.text_info_label.set_margin_bottom(6)
        text_container.append(self.text_info_label)

        self.stack.add_named(text_container, "text")

        # Page 3: Image Preview
        image_container = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        scrolled_image = Gtk.ScrolledWindow(vexpand=True, hexpand=True)
        self.picture = Gtk.Picture()
        self.picture.set_can_shrink(True)
        self.picture.set_valign(Gtk.Align.CENTER)
        self.picture.set_halign(Gtk.Align.CENTER)
        self.picture.set_margin_top(16)
        self.picture.set_margin_bottom(16)
        scrolled_image.set_child(self.picture)
        image_container.append(scrolled_image)

        self.image_info_label = Gtk.Label(label="", xalign=0.5)
        self.image_info_label.add_css_class("caption")
        self.image_info_label.add_css_class("dim-label")
        self.image_info_label.set_margin_bottom(8)
        image_container.append(self.image_info_label)

        self.stack.add_named(image_container, "image")

        # Page 4: Binary / Metadata Card
        self.binary_status = Adw.StatusPage(
            icon_name="package-x-generic-symbolic",
            title=_("Binary File"),
            description="",
        )
        self.binary_status.add_css_class("compact")

        # Hex Preview Box inside binary page
        self.hex_label = Gtk.Label(
            label="",
            xalign=0.5,
            wrap=True,
        )
        self.hex_label.add_css_class("monospace")
        self.hex_label.add_css_class("dim-label")
        self.hex_label.add_css_class("caption")

        bin_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=12,
            valign=Gtk.Align.CENTER,
        )
        bin_box.append(self.binary_status)
        bin_box.append(self.hex_label)

        self.stack.add_named(bin_box, "binary")

        # Page 5: Error / Empty State
        self.error_status = Adw.StatusPage(
            icon_name="dialog-warning-symbolic",
            title=_("Unable to preview file"),
            description="",
        )
        self.stack.add_named(self.error_status, "error")

        self.set_content(self.stack)

    def _setup_keyboard_shortcuts(self) -> None:
        """Handle keyboard navigation: Space/Escape to close, Up/Down for next/prev file."""
        key_controller = Gtk.EventControllerKey.new()
        key_controller.connect("key-pressed", self._on_key_pressed)
        self.add_controller(key_controller)

    def _on_key_pressed(
        self,
        controller: Gtk.EventControllerKey,
        keyval: int,
        keycode: int,
        state: Gdk.ModifierType,
    ) -> bool:
        if keyval in (Gdk.KEY_Escape, Gdk.KEY_space):
            self.close()
            return Gdk.EVENT_STOP

        if keyval in (Gdk.KEY_Up, Gdk.KEY_k):
            if self.on_navigate:
                res = self.on_navigate(-1)
                if res:
                    item, folder = res
                    self.preview_item(item, folder)
                return Gdk.EVENT_STOP

        elif keyval in (Gdk.KEY_Down, Gdk.KEY_j):
            if self.on_navigate:
                res = self.on_navigate(1)
                if res:
                    item, folder = res
                    self.preview_item(item, folder)
                return Gdk.EVENT_STOP

        return Gdk.EVENT_PROPAGATE

    def _init_syntax_tags(self, buffer: Gtk.TextBuffer) -> None:
        """Create standard syntax highlighting text tags for the TextBuffer."""
        # Catppuccin / Adwaita Dark inspired palette
        tags = [
            ("kw", "#c678dd", True, False),       # Keywords (purple, bold)
            ("str", "#98c379", False, False),     # Strings (green)
            ("num", "#d19a66", False, False),     # Numbers (orange)
            ("comment", "#6c7086", False, True),  # Comments (italic dimmed)
            ("fn", "#61afef", True, False),       # Functions/Types (blue)
            ("op", "#56b6c2", False, False),      # Operators (cyan)
            ("err", "#e06c75", True, False),      # Errors (red bold)
            ("warn", "#e5c07b", True, False),     # Warnings (yellow bold)
            ("info", "#61afef", False, False),    # Info/notice (cyan/blue)
            ("date", "#858894", False, False),    # Timestamps (dimmed)
        ]
        for name, color, bold, italic in tags:
            tag = buffer.create_tag(name, foreground=color)
            if bold:
                tag.set_property("weight", Pango.Weight.BOLD)
            if italic:
                tag.set_property("style", Pango.Style.ITALIC)

    def preview_item(
        self,
        item: FileItem,
        current_folder: str,
        operations=None,
    ) -> None:
        """Update and present preview for a given FileItem."""
        self.current_item = item
        self.current_folder = current_folder
        self._current_text_content = ""

        # Update Header Titles
        self.title_label.set_text(item.name)
        full_path = f"{current_folder.rstrip('/')}/{item.name}"
        self.subtitle_label.set_text(f"{item.formatted_size} • {full_path}")

        ext = Path(item.name).suffix.lower()

        # Folders cannot be previewed in quick look
        if item.is_directory:
            self.binary_status.set_title(_("Directory"))
            self.binary_status.set_icon_name("folder-symbolic")
            self.binary_status.set_description(
                f"{_('Permissions')}: {item.permissions}\n"
                f"{_('Owner')}: {item.owner}:{item.group}\n"
                f"{_('Date Modified')}: {item.date_modified}"
            )
            self.hex_label.set_text("")
            self.copy_btn.set_visible(False)
            self.open_editor_btn.set_visible(False)
            self.stack.set_visible_child_name("binary")
            self.present()
            return

        self.copy_btn.set_visible(True)
        self.open_editor_btn.set_visible(True)

        # 1. Image Preview
        if ext in IMAGE_EXTENSIONS:
            self._preview_image(item, full_path, operations)
            self.present()
            return

        # 2. Text / Code / Script / Log Preview
        self._preview_text_or_binary(item, full_path, operations)
        self.present()

    def _preview_image(self, item: FileItem, full_path: str, operations=None) -> None:
        """Load and display image file."""
        self.stack.set_visible_child_name("loading")
        self.spinner.start()

        def load_worker():
            try:
                # If remote session, operations will provide local cached copy or sample
                local_path = full_path
                is_temp = False

                if operations and operations.session_item and operations.session_item.is_ssh():
                    # For remote images, download to temp
                    import tempfile
                    ext = Path(item.name).suffix.lower()
                    with tempfile.NamedTemporaryFile(delete=False, suffix=ext) as tmp:
                        temp_path = tmp.name
                    success, _ = operations.download_file_sync(full_path, temp_path)
                    if success:
                        local_path = temp_path
                        is_temp = True
                    else:
                        raise RuntimeError(_("Failed to fetch remote image."))

                if not Path(local_path).is_file():
                    raise FileNotFoundError(f"File not found: {local_path}")

                file_obj = Gio.File.new_for_path(local_path)

                def on_loaded():
                    self.spinner.stop()
                    self.picture.set_file(file_obj)
                    self.image_info_label.set_text(
                        f"{item.name} • {item.formatted_size} • {item.date_modified}"
                    )
                    self.copy_btn.set_visible(False)
                    self.stack.set_visible_child_name("image")
                    if is_temp and Path(local_path).exists():
                        try:
                            Path(local_path).unlink()
                        except Exception:
                            pass
                    return GLib.SOURCE_REMOVE

                GLib.idle_add(on_loaded)

            except Exception as e:
                self.logger.warning(f"Failed to load image preview for {full_path}: {e}")

                def on_error():
                    self.spinner.stop()
                    self.error_status.set_title(_("Unable to preview image"))
                    self.error_status.set_description(str(e))
                    self.stack.set_visible_child_name("error")
                    return GLib.SOURCE_REMOVE

                GLib.idle_add(on_error)

        import threading
        threading.Thread(target=load_worker, daemon=True).start()

    def _preview_text_or_binary(
        self,
        item: FileItem,
        full_path: str,
        operations=None,
    ) -> None:
        """Asynchronously load text sample and apply syntax highlighting or binary fallback."""
        self.stack.set_visible_child_name("loading")
        self.spinner.start()

        def load_worker():
            try:
                raw_bytes = b""
                is_truncated = False
                err_msg = None

                is_remote = (
                    operations is not None
                    and operations.session_item is not None
                    and operations.session_item.is_ssh()
                )

                if not is_remote:
                    # Local read
                    p = Path(full_path)
                    if not p.is_file():
                        raise FileNotFoundError(f"File not found: {full_path}")
                    file_size = p.stat().st_size
                    with open(p, "rb") as f:
                        raw_bytes = f.read(MAX_PREVIEW_BYTES)
                    is_truncated = file_size > MAX_PREVIEW_BYTES
                else:
                    # Remote read via head -c
                    cmd = ["head", "-c", str(MAX_PREVIEW_BYTES + 1), full_path]
                    success, output = operations.execute_command_on_session(cmd, timeout=10)
                    if not success:
                        raise RuntimeError(output or _("Failed to read remote file"))
                    raw_bytes = output.encode("utf-8", errors="surrogateescape")
                    if len(raw_bytes) > MAX_PREVIEW_BYTES:
                        raw_bytes = raw_bytes[:MAX_PREVIEW_BYTES]
                        is_truncated = True

                # Check if file is binary (contains null bytes)
                is_binary = b"\x00" in raw_bytes[:2048]

                def on_data_ready():
                    self.spinner.stop()
                    if is_binary:
                        self._render_binary_preview(item, raw_bytes)
                    else:
                        self._render_text_preview(item, raw_bytes, is_truncated)
                    return GLib.SOURCE_REMOVE

                GLib.idle_add(on_data_ready)

            except Exception as e:
                self.logger.warning(f"Error reading preview for {full_path}: {e}")

                def on_error():
                    self.spinner.stop()
                    self.error_status.set_title(_("Unable to preview file"))
                    self.error_status.set_description(str(e))
                    self.stack.set_visible_child_name("error")
                    return GLib.SOURCE_REMOVE

                GLib.idle_add(on_error)

        import threading
        threading.Thread(target=load_worker, daemon=True).start()

    def _render_text_preview(
        self,
        item: FileItem,
        raw_bytes: bytes,
        is_truncated: bool,
    ) -> None:
        """Render text content with Pygments syntax highlighting into the TextBuffer."""
        try:
            text = raw_bytes.decode("utf-8")
        except UnicodeDecodeError:
            try:
                text = raw_bytes.decode("latin-1")
            except Exception:
                text = str(raw_bytes)

        self._current_text_content = text
        self.truncated_banner.set_revealed(is_truncated)

        # Truncate lines if excessive for UI smoothness
        lines = text.splitlines(keepends=True)
        if len(lines) > MAX_PREVIEW_LINES:
            text = "".join(lines[:MAX_PREVIEW_LINES])
            self.truncated_banner.set_revealed(True)

        self.text_buffer.set_text("")
        self._apply_syntax_highlighting(item.name, text)

        num_lines = len(lines)
        mime, _ = mimetypes.guess_type(item.name)
        mime_str = mime or "text/plain"
        self.text_info_label.set_text(
            f"{num_lines} {_('lines')} • {item.formatted_size} • {mime_str} • UTF-8"
        )

        self.copy_btn.set_visible(True)
        self.stack.set_visible_child_name("text")

    def _apply_syntax_highlighting(self, filename: str, text: str) -> None:
        """Parse tokens with Pygments and insert tagged text into TextBuffer."""
        try:
            import pygments
            from pygments.lexers import get_lexer_for_filename, TextLexer
            from pygments.token import Token

            try:
                lexer = get_lexer_for_filename(filename, text)
            except Exception:
                lexer = TextLexer()

            iter_end = self.text_buffer.get_end_iter()

            # Mapping of Pygments token types to buffer tag names
            token_map = {
                Token.Keyword: "kw",
                Token.Keyword.Constant: "kw",
                Token.Keyword.Declaration: "kw",
                Token.Keyword.Namespace: "kw",
                Token.String: "str",
                Token.String.Char: "str",
                Token.String.Doc: "str",
                Token.String.Double: "str",
                Token.String.Single: "str",
                Token.Number: "num",
                Token.Number.Integer: "num",
                Token.Number.Float: "num",
                Token.Number.Hex: "num",
                Token.Comment: "comment",
                Token.Comment.Single: "comment",
                Token.Comment.Multiline: "comment",
                Token.Name.Function: "fn",
                Token.Name.Class: "fn",
                Token.Operator: "op",
                Token.Operator.Word: "kw",
                Token.Error: "err",
            }

            is_log_file = filename.endswith(".log") or "log" in filename.lower()

            for token_type, token_val in lexer.get_tokens(text):
                tag_name = None
                for base_type, tag in token_map.items():
                    if token_type in base_type:
                        tag_name = tag
                        break

                if is_log_file:
                    val_upper = token_val.upper()
                    if "ERROR" in val_upper or "FATAL" in val_upper or "FAIL" in val_upper:
                        tag_name = "err"
                    elif "WARN" in val_upper:
                        tag_name = "warn"
                    elif "INFO" in val_upper:
                        tag_name = "info"

                if tag_name:
                    self.text_buffer.insert_with_tags_by_name(
                        iter_end, token_val, tag_name
                    )
                else:
                    self.text_buffer.insert(iter_end, token_val)

        except Exception as e:
            self.logger.debug(f"Pygments formatting fallback: {e}")
            self.text_buffer.set_text(text)

    def _render_binary_preview(self, item: FileItem, raw_bytes: bytes) -> None:
        """Render binary info card with formatted hex dump."""
        mime, _ = mimetypes.guess_type(item.name)
        mime_str = mime or "application/octet-stream"

        self.binary_status.set_title(item.name)
        self.binary_status.set_icon_name("package-x-generic-symbolic")
        self.binary_status.set_description(
            f"{_('Size')}: {item.formatted_size} ({item.size_bytes} bytes)\n"
            f"{_('Type')}: {mime_str}\n"
            f"{_('Permissions')}: {item.permissions}\n"
            f"{_('Owner')}: {item.owner}:{item.group}\n"
            f"{_('Date Modified')}: {item.date_modified}"
        )

        # Generate clean Hex Dump of first 64 bytes
        sample = raw_bytes[:64]
        hex_str = binascii.hexlify(sample).decode("ascii")
        formatted_hex = " ".join(
            hex_str[i : i + 2] for i in range(0, len(hex_str), 2)
        )
        # Format into rows of 16 bytes
        rows = [
            formatted_hex[i : i + 48]
            for i in range(0, len(formatted_hex), 48)
        ]
        self.hex_label.set_text(
            f"--- Hex Header ---\n" + "\n".join(rows) if rows else ""
        )

        self.copy_btn.set_visible(False)
        self.stack.set_visible_child_name("binary")

    def _on_copy_clicked(self, _btn) -> None:
        """Copy current preview text to clipboard."""
        if not self._current_text_content:
            return
        clipboard = self.get_clipboard()
        clipboard.set(self._current_text_content)
        if hasattr(self.parent_window, "toast_overlay"):
            self.parent_window.toast_overlay.add_toast(
                Adw.Toast(title=_("Preview content copied to clipboard"))
            )

    def _on_open_editor_clicked(self, _btn) -> None:
        """Trigger opening in configured editor and close preview."""
        if self.current_item and self.on_open_editor:
            self.on_open_editor(self.current_item, self.current_folder)
            self.close()
