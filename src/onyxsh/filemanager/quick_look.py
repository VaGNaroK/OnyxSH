# onyxsh/filemanager/quick_look.py
"""
Quick Look preview dialog and integrated in-place editor for OnyxSH File Manager.

Allows instant file inspection, editing, and saving on Space key press or context menu action:
- Syntax highlighted code/scripts (.sh, .py, .json, .yaml, .md, .c, .rs, etc.)
- In-place text editor with Normal and Superuser (sudo/pkexec) save support
- Full support for both local files and remote SSH server sessions without DE
- Log file viewer with error/warning level highlighting
- Image viewer (.png, .jpg, .svg, .webp, .ico, etc.) with dimensions metadata
- Binary/archive metadata card with hex inspection
- Keyboard navigation (Ctrl+S to save, Ctrl+Shift+S for Root save, Ctrl+E to edit, Esc/Space to close)
"""

import binascii
import mimetypes
import os
import threading
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
    """Modern Libadwaita Quick Look preview and in-place editor dialog."""

    def __init__(
        self,
        parent_window: Gtk.Window,
        on_open_editor: Optional[Callable[[FileItem, str], None]] = None,
        on_navigate: Optional[Callable[[int], Optional[Tuple[FileItem, str]]]] = None,
        on_ai_explain: Optional[Callable[[FileItem, str], None]] = None,
        on_calculate_checksum: Optional[Callable[[FileItem, str], None]] = None,
        on_file_saved: Optional[Callable[[FileItem, str], None]] = None,
    ) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Quick Look"),
            auto_setup_toolbar=True,
            default_width=820,
            default_height=600,
        )
        self.logger = get_logger("onyxsh.filemanager.quick_look")
        self.on_open_editor = on_open_editor
        self.on_navigate = on_navigate
        self.on_ai_explain = on_ai_explain
        self.on_calculate_checksum = on_calculate_checksum
        self.on_file_saved = on_file_saved

        self.current_item: Optional[FileItem] = None
        self.current_folder: str = ""
        self.operations = None
        self._current_text_content: str = ""
        self._original_text_content: str = ""
        self._is_loading = False
        self._is_loading_content = False
        self._is_saving = False
        self.is_editing = False
        self.is_dirty = False
        self.is_truncated = False
        self._full_file_loaded = False
        self._is_binary = False
        self._is_image = False

        self._setup_ui()
        self._setup_keyboard_shortcuts()
        self.connect("close-request", self._on_window_close_request)

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

            # More Actions Menu Button
            self.menu_btn = self._create_more_actions_menu()
            self._header_bar.pack_end(self.menu_btn)

            # Save as Root (Sudo) Button
            self.save_sudo_btn = Gtk.Button(
                label=_("Save as Root"),
                icon_name="security-high-symbolic",
            )
            self.save_sudo_btn.set_tooltip_text(
                _("Save with elevated Superuser / Root privileges (Ctrl+Shift+S)")
            )
            self.save_sudo_btn.add_css_class("flat")
            self.save_sudo_btn.set_visible(False)
            self.save_sudo_btn.connect("clicked", lambda _: self._on_save_clicked(as_sudo=True))
            self._header_bar.pack_end(self.save_sudo_btn)

            # Save Button
            self.save_btn = Gtk.Button(
                label=_("Save"),
                icon_name="document-save-symbolic",
            )
            self.save_btn.set_tooltip_text(_("Save changes (Ctrl+S)"))
            self.save_btn.add_css_class("suggested-action")
            self.save_btn.set_visible(False)
            self.save_btn.set_sensitive(False)
            self.save_btn.connect("clicked", lambda _: self._on_save_clicked(as_sudo=False))
            self._header_bar.pack_end(self.save_btn)

            # Edit Toggle Button
            self.edit_toggle_btn = Gtk.ToggleButton(
                icon_name="document-edit-symbolic",
            )
            self.edit_toggle_btn.set_tooltip_text(_("Toggle Edit Mode (Ctrl+E)"))
            self.edit_toggle_btn.add_css_class("flat")
            self.edit_toggle_btn.connect("toggled", self._on_edit_toggle_toggled)
            self._header_bar.pack_end(self.edit_toggle_btn)

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

        # Page 2: Text / Code Preview & Editor
        text_container = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)

        # Truncation banner if needed
        self.truncated_banner = Adw.Banner(
            title=_("Content truncated for preview. Switching to Edit Mode will load the full file."),
            revealed=False,
        )
        text_container.append(self.truncated_banner)

        # Read-only / Root warning banner
        self.readonly_banner = Adw.Banner(
            title=_("This file is read-only. Edits can be saved with Superuser (Root) privileges."),
            revealed=False,
        )
        text_container.append(self.readonly_banner)

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
        self.text_buffer.connect("changed", self._on_buffer_changed)

        scrolled_text.set_child(self.text_view)
        text_container.append(scrolled_text)

        # Bottom info bar for text (lines, encoding, size, mode)
        bottom_bar = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        bottom_bar.set_margin_start(16)
        bottom_bar.set_margin_end(16)
        bottom_bar.set_margin_top(6)
        bottom_bar.set_margin_bottom(6)

        self.text_info_label = Gtk.Label(label="", xalign=0)
        self.text_info_label.set_hexpand(True)
        self.text_info_label.add_css_class("caption")
        self.text_info_label.add_css_class("dim-label")

        self.mode_badge = Gtk.Label(label=_("Preview Mode"))
        self.mode_badge.add_css_class("badge-pill")
        self.mode_badge.add_css_class("caption")

        bottom_bar.append(self.text_info_label)
        bottom_bar.append(self.mode_badge)
        text_container.append(bottom_bar)

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

        if self._cancel_button:
            self._cancel_button.set_visible(False)

        if self._toolbar_view:
            self._toolbar_view.set_content(self.stack)
        else:
            self.set_content(self.stack)

    def _create_more_actions_menu(self) -> Gtk.MenuButton:
        """Create secondary popover menu for external editor, AI explain, hash, copy, reload."""
        menu = Gio.Menu()
        menu.append(_("Open with External App..."), "quicklook.open_external")
        menu.append(_("Explain with AI"), "quicklook.ai_explain")
        menu.append(_("Calculate / Verify Hash..."), "quicklook.checksum")
        menu.append(_("Copy Content"), "quicklook.copy")
        menu.append(_("Reload from Disk"), "quicklook.reload")

        action_group = Gio.SimpleActionGroup.new()

        act_open = Gio.SimpleAction.new("open_external", None)
        act_open.connect("activate", lambda *_: self._on_open_editor_clicked(None))
        action_group.add_action(act_open)

        act_ai = Gio.SimpleAction.new("ai_explain", None)
        act_ai.connect("activate", lambda *_: self._on_ai_explain_clicked(None))
        action_group.add_action(act_ai)

        act_hash = Gio.SimpleAction.new("checksum", None)
        act_hash.connect("activate", lambda *_: self._on_checksum_clicked(None))
        action_group.add_action(act_hash)

        act_copy = Gio.SimpleAction.new("copy", None)
        act_copy.connect("activate", lambda *_: self._on_copy_clicked(None))
        action_group.add_action(act_copy)

        act_reload = Gio.SimpleAction.new("reload", None)
        act_reload.connect("activate", lambda *_: self._on_reload_clicked(None))
        action_group.add_action(act_reload)

        self.insert_action_group("quicklook", action_group)

        btn = Gtk.MenuButton(icon_name="view-more-symbolic", menu_model=menu)
        btn.set_tooltip_text(_("More Options"))
        btn.add_css_class("flat")
        return btn

    def _setup_keyboard_shortcuts(self) -> None:
        """Handle keyboard navigation and editor shortcuts."""
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
        ctrl = bool(state & Gdk.ModifierType.CONTROL_MASK)
        shift = bool(state & Gdk.ModifierType.SHIFT_MASK)

        # 1. Editor Shortcuts (Ctrl+S, Ctrl+Shift+S, Ctrl+E)
        if ctrl and keyval in (Gdk.KEY_s, Gdk.KEY_S):
            if not self._is_binary and not self._is_image:
                if shift:
                    self._on_save_clicked(as_sudo=True)
                else:
                    self._on_save_clicked(as_sudo=False)
                return Gdk.EVENT_STOP

        if ctrl and keyval in (Gdk.KEY_e, Gdk.KEY_E):
            if not self._is_binary and not self._is_image:
                self.edit_toggle_btn.set_active(not self.edit_toggle_btn.get_active())
                return Gdk.EVENT_STOP

        # 2. Close on Escape
        if keyval == Gdk.KEY_Escape:
            self._handle_close_request()
            return Gdk.EVENT_STOP

        # 3. If in Edit Mode, let text navigation keys pass directly to TextView
        if self.is_editing:
            return Gdk.EVENT_PROPAGATE

        # 4. Preview-only keys (Space/q to close, Up/Down for next/prev file)
        if keyval in (Gdk.KEY_space, Gdk.KEY_q, Gdk.KEY_Q):
            self._handle_close_request()
            return Gdk.EVENT_STOP

        if keyval in (Gdk.KEY_Up, Gdk.KEY_k):
            if self.on_navigate:
                res = self.on_navigate(-1)
                if res:
                    item, folder = res
                    self.preview_item(item, folder, self.operations)
                return Gdk.EVENT_STOP

        elif keyval in (Gdk.KEY_Down, Gdk.KEY_j):
            if self.on_navigate:
                res = self.on_navigate(1)
                if res:
                    item, folder = res
                    self.preview_item(item, folder, self.operations)
                return Gdk.EVENT_STOP

        return Gdk.EVENT_PROPAGATE

    def _init_syntax_tags(self, buffer: Gtk.TextBuffer) -> None:
        """Create standard syntax highlighting text tags for the TextBuffer."""
        tags = [
            ("kw", "#c678dd", True, False),
            ("str", "#98c379", False, False),
            ("num", "#d19a66", False, False),
            ("comment", "#6c7086", False, True),
            ("fn", "#61afef", True, False),
            ("op", "#56b6c2", False, False),
            ("err", "#e06c75", True, False),
            ("warn", "#e5c07b", True, False),
            ("info", "#61afef", False, False),
            ("date", "#858894", False, False),
        ]
        for name, color, bold, italic in tags:
            tag = buffer.create_tag(name, foreground=color)
            if bold:
                tag.set_property("weight", Pango.Weight.BOLD)
            if italic:
                tag.set_property("style", Pango.Style.ITALIC)

    def _on_buffer_changed(self, buffer: Gtk.TextBuffer) -> None:
        """Track dirty status when user modifies text in the editor."""
        if self._is_loading_content:
            return

        start_iter = buffer.get_start_iter()
        end_iter = buffer.get_end_iter()
        current_text = buffer.get_text(start_iter, end_iter, True)

        is_now_dirty = current_text != self._original_text_content
        if is_now_dirty != self.is_dirty:
            self.is_dirty = is_now_dirty
            self.save_btn.set_sensitive(self.is_dirty)
            self._update_title_display()

    def _update_title_display(self) -> None:
        """Update window title and subtitle with file info and dirty indicator."""
        if not self.current_item:
            return
        dirty_marker = " ●" if self.is_dirty else ""
        self.title_label.set_text(f"{self.current_item.name}{dirty_marker}")
        full_path = f"{self.current_folder.rstrip('/')}/{self.current_item.name}"
        self.subtitle_label.set_text(f"{self.current_item.formatted_size} • {full_path}")

    def preview_item(
        self,
        item: FileItem,
        current_folder: str,
        operations=None,
    ) -> None:
        """Update and present preview for a given FileItem."""
        self.current_item = item
        self.current_folder = current_folder
        self.operations = operations
        self._current_text_content = ""
        self._original_text_content = ""
        self.is_dirty = False
        self._full_file_loaded = False
        self._is_binary = False
        self._is_image = False

        self._update_title_display()
        ext = Path(item.name).suffix.lower()

        # Folders cannot be previewed/edited in quick look
        if item.is_directory:
            self._is_binary = True
            self.binary_status.set_title(_("Directory"))
            self.binary_status.set_icon_name("folder-symbolic")
            self.binary_status.set_description(
                f"{_('Permissions')}: {item.permissions}\n"
                f"{_('Owner')}: {item.owner}:{item.group}\n"
                f"{_('Date Modified')}: {item.date_modified}"
            )
            self.hex_label.set_text("")
            self.edit_toggle_btn.set_visible(False)
            self.save_btn.set_visible(False)
            self.save_sudo_btn.set_visible(False)
            self.menu_btn.set_visible(False)
            self.stack.set_visible_child_name("binary")
            self.present()
            return

        self.edit_toggle_btn.set_visible(True)
        self.menu_btn.set_visible(True)

        # 1. Image Preview
        if ext in IMAGE_EXTENSIONS:
            self._is_image = True
            self.edit_toggle_btn.set_visible(False)
            self.save_btn.set_visible(False)
            self.save_sudo_btn.set_visible(False)
            self._preview_image(item, f"{current_folder.rstrip('/')}/{item.name}", operations)
            self.present()
            return

        # 2. Text / Code / Script / Log Preview
        self._preview_text_or_binary(item, f"{current_folder.rstrip('/')}/{item.name}", operations)
        self.present()

    def _preview_image(self, item: FileItem, full_path: str, operations=None) -> None:
        """Load and display image file."""
        self.stack.set_visible_child_name("loading")
        self.spinner.start()

        def load_worker():
            try:
                local_path = full_path
                is_temp = False

                if operations and operations.session_item and operations.session_item.is_ssh():
                    import tempfile
                    ext = Path(item.name).suffix.lower()
                    with tempfile.NamedTemporaryFile(delete=False, suffix=ext) as tmp:
                        temp_path = tmp.name
                    success, _err = operations.download_file_sync(full_path, temp_path)
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

        threading.Thread(target=load_worker, daemon=True).start()

    def _preview_text_or_binary(
        self,
        item: FileItem,
        full_path: str,
        operations=None,
        load_full: bool = False,
    ) -> None:
        """Asynchronously load text sample and apply syntax highlighting or binary fallback."""
        self.stack.set_visible_child_name("loading")
        self.spinner.start()

        def load_worker():
            try:
                raw_bytes = b""
                is_truncated = False

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
                        if load_full:
                            raw_bytes = f.read()
                            is_truncated = False
                        else:
                            raw_bytes = f.read(MAX_PREVIEW_BYTES)
                            is_truncated = file_size > MAX_PREVIEW_BYTES
                else:
                    # Remote read
                    if load_full:
                        cmd = ["cat", full_path]
                        success, output = operations.execute_command_on_session(cmd, timeout=30)
                    else:
                        cmd = ["head", "-c", str(MAX_PREVIEW_BYTES + 1), full_path]
                        success, output = operations.execute_command_on_session(cmd, timeout=10)

                    if not success:
                        raise RuntimeError(output or _("Failed to read remote file"))
                    raw_bytes = output.encode("utf-8", errors="surrogateescape")
                    if not load_full and len(raw_bytes) > MAX_PREVIEW_BYTES:
                        raw_bytes = raw_bytes[:MAX_PREVIEW_BYTES]
                        is_truncated = True

                is_binary = b"\x00" in raw_bytes[:2048]

                def on_data_ready():
                    self.spinner.stop()
                    self._is_binary = is_binary
                    if is_binary:
                        self.edit_toggle_btn.set_visible(False)
                        self.save_btn.set_visible(False)
                        self.save_sudo_btn.set_visible(False)
                        self._render_binary_preview(item, raw_bytes)
                    else:
                        self.edit_toggle_btn.set_visible(True)
                        self._render_text_preview(item, raw_bytes, is_truncated, full_path)
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

        threading.Thread(target=load_worker, daemon=True).start()

    def _render_text_preview(
        self,
        item: FileItem,
        raw_bytes: bytes,
        is_truncated: bool,
        full_path: str = "",
    ) -> None:
        """Render text content into the TextBuffer and evaluate permissions."""
        if not full_path and self.current_folder and item:
            full_path = f"{self.current_folder.rstrip('/')}/{item.name}"
        try:
            text = raw_bytes.decode("utf-8")
        except UnicodeDecodeError:
            try:
                text = raw_bytes.decode("latin-1")
            except Exception:
                text = str(raw_bytes)

        self._is_loading_content = True
        self._current_text_content = text
        self._original_text_content = text
        self.is_truncated = is_truncated
        self.is_dirty = False
        self.save_btn.set_sensitive(False)

        self.truncated_banner.set_revealed(is_truncated and not self.is_editing)

        # Check write permissions
        is_remote = (
            self.operations is not None
            and self.operations.session_item is not None
            and self.operations.session_item.is_ssh()
        )
        is_writable = True
        if not is_remote:
            try:
                is_writable = os.access(full_path, os.W_OK)
            except Exception:
                is_writable = False
        else:
            if item.owner == "root" and item.permissions and "w" not in item.permissions[-3:]:
                is_writable = False

        self.readonly_banner.set_revealed(not is_writable)

        self.text_buffer.set_text("")
        self._apply_syntax_highlighting(item.name, text)
        self._is_loading_content = False

        lines = text.splitlines(keepends=True)
        num_lines = len(lines)
        mime, _encoding = mimetypes.guess_type(item.name)
        mime_str = mime or "text/plain"
        self.text_info_label.set_text(
            f"{num_lines} {_('lines')} • {item.formatted_size} • {mime_str} • UTF-8"
        )

        self.stack.set_visible_child_name("text")
        self._update_title_display()

    def _apply_syntax_highlighting(self, filename: str, text: str) -> None:
        """Parse tokens with Pygments and insert tagged text into TextBuffer."""
        try:
            from pygments.lexers import get_lexer_for_filename, TextLexer
            from pygments.token import Token

            try:
                lexer = get_lexer_for_filename(filename, text)
            except Exception:
                lexer = TextLexer()

            iter_end = self.text_buffer.get_end_iter()

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
        """Render binary file card with hex header dump."""
        self.binary_status.set_title(item.name)
        self.binary_status.set_icon_name(item.icon_name)
        self.binary_status.set_description(
            f"{_('Size')}: {item.formatted_size}\n"
            f"{_('Permissions')}: {item.permissions}\n"
            f"{_('Owner')}: {item.owner}:{item.group}\n"
            f"{_('Date Modified')}: {item.date_modified}"
        )

        hex_sample = raw_bytes[:128]
        hex_str = binascii.hexlify(hex_sample).decode("ascii")
        formatted_hex = " ".join(
            hex_str[i : i + 2] for i in range(0, len(hex_str), 2)
        )
        rows = [
            formatted_hex[i : i + 48]
            for i in range(0, len(formatted_hex), 48)
        ]
        self.hex_label.set_text(
            f"--- Hex Header ---\n" + "\n".join(rows) if rows else ""
        )
        self.stack.set_visible_child_name("binary")

    # =========================================================================
    # In-Place Editor and Sudo Saving Logic
    # =========================================================================

    def _on_edit_toggle_toggled(self, btn: Gtk.ToggleButton) -> None:
        """Callback when user toggles edit mode."""
        is_active = btn.get_active()
        self._toggle_edit_mode(is_active)

    def _toggle_edit_mode(self, enabled: bool) -> None:
        """Switch between Preview and In-Place Editor Mode."""
        if enabled:
            # If file was truncated, load full content first
            if self.is_truncated and not self._full_file_loaded and self.current_item:
                full_path = f"{self.current_folder.rstrip('/')}/{self.current_item.name}"
                self._full_file_loaded = True
                self._preview_text_or_binary(
                    self.current_item, full_path, self.operations, load_full=True
                )

            self.is_editing = True
            self.text_view.set_editable(True)
            self.text_view.add_css_class("editing")
            self.save_btn.set_visible(True)
            self.save_sudo_btn.set_visible(True)
            self.mode_badge.set_text(_("Edit Mode"))
            self.mode_badge.add_css_class("badge-exec")
            self.text_view.grab_focus()
        else:
            if self.is_dirty:
                self._show_discard_changes_dialog(
                    on_discard=lambda: self._force_exit_edit_mode()
                )
                return
            self._force_exit_edit_mode()

    def _force_exit_edit_mode(self) -> None:
        """Exit edit mode and restore preview state."""
        self.is_editing = False
        self.text_view.set_editable(False)
        self.text_view.remove_css_class("editing")
        self.save_btn.set_visible(False)
        self.save_sudo_btn.set_visible(False)
        self.mode_badge.set_text(_("Preview Mode"))
        self.mode_badge.remove_css_class("badge-exec")
        self.edit_toggle_btn.set_active(False)

    def _on_save_clicked(
        self,
        as_sudo: bool = False,
        sudo_password: Optional[str] = None,
        on_success: Optional[Callable[[], None]] = None,
    ) -> None:
        """Handle saving text buffer content locally or remotely with optional sudo elevation."""
        if not self.current_item or self._is_saving:
            return

        start_iter = self.text_buffer.get_start_iter()
        end_iter = self.text_buffer.get_end_iter()
        content = self.text_buffer.get_text(start_iter, end_iter, True)
        full_path = f"{self.current_folder.rstrip('/')}/{self.current_item.name}"

        self._is_saving = True
        self.save_btn.set_sensitive(False)
        self.save_sudo_btn.set_sensitive(False)

        def save_worker():
            try:
                if self.operations:
                    success, msg = self.operations.save_file_content(
                        full_path, content, as_sudo=as_sudo, sudo_password=sudo_password
                    )
                else:
                    Path(full_path).write_text(content, encoding="utf-8")
                    success, msg = True, "OK"

                def on_done():
                    self._is_saving = False
                    self.save_btn.set_sensitive(self.is_dirty)
                    self.save_sudo_btn.set_sensitive(True)

                    if success:
                        self.is_dirty = False
                        self._original_text_content = content
                        self._current_text_content = content
                        self._update_title_display()
                        self.save_btn.set_sensitive(False)

                        # Show success toast
                        if hasattr(self.parent_window, "toast_overlay"):
                            self.parent_window.toast_overlay.add_toast(
                                Adw.Toast(title=_("File saved successfully."))
                            )

                        if self.on_file_saved and self.current_item:
                            self.on_file_saved(self.current_item, self.current_folder)

                        if on_success:
                            on_success()

                    else:
                        if msg == "PERMISSION_DENIED" and not as_sudo:
                            self._show_permission_denied_dialog()
                        elif msg == "PASSWORD_REQUIRED":
                            self._prompt_sudo_password(
                                on_submit=lambda pwd: self._on_save_clicked(
                                    as_sudo=True, sudo_password=pwd, on_success=on_success
                                )
                            )
                        else:
                            self._show_error_dialog(
                                _("Save Failed"),
                                str(msg),
                            )
                    return GLib.SOURCE_REMOVE

                GLib.idle_add(on_done)

            except Exception as e:
                self.logger.error(f"Save worker error: {e}")

                def on_err():
                    self._is_saving = False
                    self.save_btn.set_sensitive(self.is_dirty)
                    self.save_sudo_btn.set_sensitive(True)
                    self._show_error_dialog(_("Save Failed"), str(e))
                    return GLib.SOURCE_REMOVE

                GLib.idle_add(on_err)

        threading.Thread(target=save_worker, daemon=True).start()

    def _show_permission_denied_dialog(self) -> None:
        """Prompt user when normal save fails due to permission denial."""
        dialog = Adw.MessageDialog(
            transient_for=self,
            heading=_("Permission Denied"),
            body=_(
                "You do not have permission to modify '{filename}'. Would you like to save with Superuser (Root) privileges?"
            ).format(filename=self.current_item.name if self.current_item else ""),
            close_response="cancel",
        )
        dialog.add_response("cancel", _("Cancel"))
        dialog.add_response("save-root", _("Save as Root"))
        dialog.set_response_appearance("save-root", Adw.ResponseAppearance.SUGGESTED)

        def on_resp(d, response_id):
            if response_id == "save-root":
                self._on_save_clicked(as_sudo=True)

        dialog.connect("response", on_resp)
        dialog.present()

    def _prompt_sudo_password(self, on_submit: Callable[[str], None]) -> None:
        """Display secure password entry modal when sudo requires authentication."""
        dialog = Adw.MessageDialog(
            transient_for=self,
            heading=_("Superuser Authentication"),
            body=_("A password is required to save changes with administrative privileges."),
            close_response="cancel",
        )

        entry_row = Adw.PasswordEntryRow(title=_("Password"))
        entry_row.set_margin_start(12)
        entry_row.set_margin_end(12)
        entry_row.set_margin_top(8)
        entry_row.set_margin_bottom(8)

        dialog.set_extra_child(entry_row)
        dialog.add_response("cancel", _("Cancel"))
        dialog.add_response("auth", _("Authenticate & Save"))
        dialog.set_response_appearance("auth", Adw.ResponseAppearance.SUGGESTED)

        def on_resp(d, response_id):
            if response_id == "auth":
                pwd = entry_row.get_text()
                on_submit(pwd)

        dialog.connect("response", on_resp)
        dialog.present()

    def _show_discard_changes_dialog(self, on_discard: Callable[[], None]) -> None:
        """Prompt user before discarding unsaved buffer changes."""
        dialog = Adw.MessageDialog(
            transient_for=self,
            heading=_("Discard Changes?"),
            body=_("You have unsaved changes that will be lost. Are you sure you want to discard them?"),
            close_response="cancel",
        )
        dialog.add_response("cancel", _("Cancel"))
        dialog.add_response("discard", _("Discard"))
        dialog.set_response_appearance("discard", Adw.ResponseAppearance.DESTRUCTIVE)

        def on_resp(d, response_id):
            if response_id == "discard":
                self.is_dirty = False
                self._current_text_content = self._original_text_content
                self.text_buffer.set_text(self._original_text_content)
                self._update_title_display()
                on_discard()

        dialog.connect("response", on_resp)
        dialog.present()

    def _show_error_dialog(self, heading: str, body: str) -> None:
        """Display user-friendly error dialog."""
        dialog = Adw.MessageDialog(
            transient_for=self,
            heading=heading,
            body=body,
            close_response="ok",
        )
        dialog.add_response("ok", _("OK"))
        dialog.present()

    def _handle_close_request(self) -> None:
        """Check for unsaved changes before closing the dialog."""
        if self.is_dirty and self.current_item:
            dialog = Adw.MessageDialog(
                transient_for=self,
                heading=_("Unsaved Changes"),
                body=_(
                    "You have unsaved changes in '{filename}'. Do you want to save them before closing?"
                ).format(filename=self.current_item.name),
                close_response="cancel",
            )
            dialog.add_response("cancel", _("Cancel"))
            dialog.add_response("discard", _("Discard Changes"))
            dialog.add_response("save", _("Save & Close"))
            dialog.set_response_appearance("discard", Adw.ResponseAppearance.DESTRUCTIVE)
            dialog.set_response_appearance("save", Adw.ResponseAppearance.SUGGESTED)

            def on_resp(d, response_id):
                if response_id == "discard":
                    self.is_dirty = False
                    self.close()
                elif response_id == "save":
                    self._on_save_clicked(as_sudo=False, on_success=self.close)

            dialog.connect("response", on_resp)
            dialog.present()
        else:
            self.close()

    def _on_window_close_request(self, _window) -> bool:
        """Intercept window close event if there are unsaved changes."""
        if self.is_dirty and self.current_item:
            self._handle_close_request()
            return True  # Stop closing until user confirms
        return False  # Allow closing

    # =========================================================================
    # Auxiliary Menu Actions
    # =========================================================================

    def _on_reload_clicked(self, _btn) -> None:
        """Reload file preview from disk."""
        if self.current_item and self.current_folder:
            if self.is_dirty:
                self._show_discard_changes_dialog(
                    on_discard=lambda: self.preview_item(
                        self.current_item, self.current_folder, self.operations
                    )
                )
            else:
                self.preview_item(
                    self.current_item, self.current_folder, self.operations
                )

    def _on_checksum_clicked(self, _btn) -> None:
        """Trigger checksum verification dialog for current previewed file."""
        if not self.current_item or not self.current_folder:
            return
        if self.on_calculate_checksum:
            self.on_calculate_checksum(self.current_item, self.current_folder)
        else:
            base_path = Path(self.current_folder)
            full_path = str(base_path / self.current_item.name)
            from ..ui.dialogs.checksum_dialog import ChecksumDialog

            dialog = ChecksumDialog(
                parent_window=self.parent_window,
                file_path=full_path,
                file_name=self.current_item.name,
                file_size_str=self.current_item.formatted_size,
            )
            dialog.present()

    def _on_ai_explain_clicked(self, _btn) -> None:
        """Trigger AI explanation for current previewed file and close dialog."""
        if self.current_item and self.on_ai_explain:
            self.on_ai_explain(self.current_item, self.current_folder)
            self.close()

    def _on_copy_clicked(self, _btn) -> None:
        """Copy current preview text to clipboard."""
        start_iter = self.text_buffer.get_start_iter()
        end_iter = self.text_buffer.get_end_iter()
        text = self.text_buffer.get_text(start_iter, end_iter, True)
        if not text:
            return
        clipboard = self.get_clipboard()
        clipboard.set(text)
        if hasattr(self.parent_window, "toast_overlay"):
            self.parent_window.toast_overlay.add_toast(
                Adw.Toast(title=_("Preview content copied to clipboard"))
            )

    def _on_open_editor_clicked(self, _btn) -> None:
        """Trigger opening in configured external editor."""
        if self.current_item and self.on_open_editor:
            self.on_open_editor(self.current_item, self.current_folder)
            self.close()
