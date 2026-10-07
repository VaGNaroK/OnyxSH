# onyxsh/ui/widgets/selection_popover.py

"""Selection Action Popover - Floating quick actions on terminal text selection."""

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Vte", "3.91")
from gi.repository import Gdk, GLib, Gtk, Vte

from ...utils.icons import icon_image
from ...utils.logger import get_logger
from ...utils.translation_utils import _


class SelectionActionPopover(Gtk.Popover):
    """
    Floating action popover displayed above or below selected text in Vte.Terminal.

    Provides quick actions:
    1. 📋 Copiar: Writes selection directly to system clipboard.
    2. 🤖 Perguntar à IA: Opens AIChatPanel with pre-filled draft context (offloading to local/cloud LLM).
    3. 🛠️ Refatorar: Sends selection to OpenCode refactor pipeline with Zero-Trust protection.
    """

    def __init__(self, terminal: Vte.Terminal, parent_window=None):
        super().__init__()
        self.terminal = terminal
        self.parent_window = parent_window
        self.logger = get_logger("onyxsh.ui.selection_popover")
        self._current_text = ""

        self.set_parent(terminal)
        self.set_autohide(True)
        self.set_has_arrow(True)
        self.add_css_class("selection-action-popover")

        # Container box
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=2)
        box.add_css_class("linked")
        box.set_margin_top(2)
        box.set_margin_bottom(2)
        box.set_margin_start(2)
        box.set_margin_end(2)

        # 1. Copy button
        self.copy_btn = Gtk.Button()
        self.copy_btn.add_css_class("flat")
        self.copy_btn.set_tooltip_text(_("Copiar seleção (Ctrl+Shift+C)"))
        copy_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        self.copy_icon = icon_image("edit-copy-symbolic", size=14)
        self.copy_label = Gtk.Label(label=_("Copiar"))
        self.copy_label.add_css_class("caption")
        copy_box.append(self.copy_icon)
        copy_box.append(self.copy_label)
        self.copy_btn.set_child(copy_box)
        self.copy_btn.connect("clicked", self._on_copy_clicked)
        box.append(self.copy_btn)

        # 2. Ask AI button
        self.ask_ai_btn = Gtk.Button()
        self.ask_ai_btn.add_css_class("flat")
        self.ask_ai_btn.set_tooltip_text(_("Perguntar à IA (abrir no chat com rascunho)"))
        ai_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        ai_icon = icon_image("sparkles-symbolic", size=14)
        ai_label = Gtk.Label(label=_("Perguntar à IA"))
        ai_label.add_css_class("caption")
        ai_box.append(ai_icon)
        ai_box.append(ai_label)
        self.ask_ai_btn.set_child(ai_box)
        self.ask_ai_btn.connect("clicked", self._on_ask_ai_clicked)
        box.append(self.ask_ai_btn)

        # 3. Refactor button (OpenCode)
        self.refactor_btn = Gtk.Button()
        self.refactor_btn.add_css_class("flat")
        self.refactor_btn.set_tooltip_text(_("Refatorar com OpenCode (Zero-Trust)"))
        refactor_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        refactor_icon = icon_image("utilities-terminal-symbolic", size=14)
        refactor_label = Gtk.Label(label=_("Refatorar"))
        refactor_label.add_css_class("caption")
        refactor_box.append(refactor_icon)
        refactor_box.append(refactor_label)
        self.refactor_btn.set_child(refactor_box)
        self.refactor_btn.connect("clicked", self._on_refactor_clicked)
        box.append(self.refactor_btn)

        self.set_child(box)

    def show_for_selection(self, selected_text: str, pointer_pos=None) -> None:
        """Display the popover positioned at the selection / mouse pointer."""
        if not selected_text or not selected_text.strip():
            self.popdown()
            return

        self._current_text = selected_text

        # Position rectangle
        rect = Gdk.Rectangle()
        if pointer_pos and len(pointer_pos) == 2:
            x, y = pointer_pos
            rect.x = max(0, int(x) - 20)
            rect.y = max(0, int(y) - 10)
            rect.width = 40
            rect.height = 20
        else:
            term_w = self.terminal.get_width()
            term_h = self.terminal.get_height()
            rect.x = max(0, int(term_w / 2) - 50)
            rect.y = max(0, int(term_h - 60))
            rect.width = 100
            rect.height = 20

        self.set_pointing_to(rect)
        self.popup()

    def _on_copy_clicked(self, _btn):
        if self._current_text:
            clipboard = (
                self.terminal.get_clipboard()
                if hasattr(self.terminal, "get_clipboard")
                else self.terminal.get_display().get_clipboard()
            )
            clipboard.set(self._current_text)

            # Micro visual feedback
            self.copy_label.set_label(_("Copiado!"))
            self.copy_icon.set_from_icon_name("emblem-ok-symbolic")

            def _reset_and_close():
                self.copy_label.set_label(_("Copiar"))
                self.copy_icon.set_from_icon_name("edit-copy-symbolic")
                self.popdown()
                return GLib.SOURCE_REMOVE

            GLib.timeout_add(600, _reset_and_close)
        else:
            self.popdown()

    def _on_ask_ai_clicked(self, _btn):
        self.popdown()
        if not self._current_text:
            return

        # 1. Resolve working directory (OSC 7)
        cwd = None
        if hasattr(self.terminal, "get_current_directory_uri"):
            from urllib.parse import unquote, urlparse

            uri = self.terminal.get_current_directory_uri()
            if uri:
                parsed = urlparse(uri)
                if parsed.scheme == "file":
                    cwd = unquote(parsed.path)

        if not cwd:
            try:
                from ...utils.osc7_tracker import get_osc7_tracker

                tracker = get_osc7_tracker()
                if self.terminal in tracker._terminals:
                    cwd = tracker._terminals[self.terminal].get("current_dir")
            except Exception:
                pass

        # 2. Get window and ui_builder
        win = self.parent_window
        if not win and hasattr(self.terminal, "get_root"):
            win = self.terminal.get_root()

        if win and hasattr(win, "ui_builder") and win.ui_builder:
            win.ui_builder.show_ai_panel()
            chat_panel = getattr(win.ui_builder, "ai_chat_panel", None)
            if chat_panel and hasattr(chat_panel, "set_initial_text"):
                header_context = f"Contexto: diretório {cwd}\n\n" if cwd else ""
                draft_text = _(
                    "{context}```\n{text}\n```\n\nExplique ou analise este código:"
                ).format(context=header_context, text=self._current_text.strip())
                chat_panel.set_initial_text(draft_text)

    def _on_refactor_clicked(self, _btn):
        self.popdown()
        if not self._current_text:
            return

        win = self.parent_window
        if not win and hasattr(self.terminal, "get_root"):
            win = self.terminal.get_root()

        if win and hasattr(win, "action_handler") and win.action_handler:
            if hasattr(win.action_handler, "opencode_selection"):
                win.action_handler.opencode_selection()
