# zashterminal/ui/dialogs/command_history_dialog.py

"""
Enhanced Command History Dialog (Ctrl + R).
Provides instant fuzzy search, filters by CWD/host/pinned, syntax display,
keyboard navigation (Enter to execute, Tab to edit), and command management.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Pango", "1.0")
gi.require_version("Gdk", "4.0")
gi.require_version("Vte", "3.91")

from gi.repository import Adw, Gdk, GLib, Gtk, Pango, Vte

from ...data.command_history_manager import (
    CommandHistoryItem,
    get_command_history_manager,
)
from ...utils.icons import icon_button, icon_image
from ...utils.logger import get_logger
from ...utils.translation_utils import _


class CommandHistoryDialog(Gtk.Window):
    """Modern modal dialog for enriched command history search and execution."""

    def __init__(
        self,
        parent_window: Gtk.Window,
        current_terminal: Optional[Vte.Terminal] = None,
        on_insert_callback: Optional[Callable[[str, bool], None]] = None,
    ) -> None:
        super().__init__(
            title=_("Histórico de Comandos"),
            transient_for=parent_window,
            modal=True,
            default_width=760,
            default_height=540,
        )
        self.logger = get_logger("zashterminal.ui.dialogs.command_history_dialog")
        self.parent_window = parent_window
        self.current_terminal = current_terminal
        self.on_insert_callback = on_insert_callback
        self.history_mgr = get_command_history_manager()

        # Determine current context (CWD & Host)
        self.current_cwd = self._detect_current_cwd()
        self.current_host = self._detect_current_host()

        self._active_filter = "all"  # "all", "cwd", "host", "pinned"
        self._items: List[CommandHistoryItem] = []

        self._setup_ui()
        self._setup_key_controller()
        self._reload_items()

    def _detect_current_cwd(self) -> str:
        """Detects CWD of the current active terminal."""
        if self.current_terminal and hasattr(
            self.current_terminal, "get_current_directory_uri"
        ):
            uri = self.current_terminal.get_current_directory_uri()
            if uri and uri.startswith("file://"):
                path = uri[7:]
                if path.startswith("localhost/"):
                    path = path[9:]
                elif path.startswith("localhost"):
                    path = path[len("localhost") :]
                return path
        return str(Path.home())

    def _detect_current_host(self) -> str:
        """Detects current host / session name."""
        if self.current_terminal and hasattr(
            self.current_terminal, "zashterminal_session"
        ):
            sess = getattr(self.current_terminal, "zashterminal_session", None)
            if sess and hasattr(sess, "host") and sess.host:
                return sess.host
        return "localhost"

    def _setup_ui(self) -> None:
        """Builds the dialog layout and widgets."""
        self.add_css_class("command-history-dialog")

        toolbar_view = Adw.ToolbarView()
        self.set_child(toolbar_view)

        # Header bar
        header_bar = Adw.HeaderBar()
        header_bar.set_show_end_title_buttons(True)
        header_bar.set_show_start_title_buttons(False)

        # Clear history menu button
        clear_btn = icon_button(
            "edit-clear-all-symbolic",
            tooltip=_("Limpar Histórico..."),
        )
        clear_btn.add_css_class("flat")
        clear_btn.connect("clicked", self._on_clear_clicked)
        header_bar.pack_start(clear_btn)

        toolbar_view.add_top_bar(header_bar)

        # Main content box
        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        content_box.set_margin_top(12)
        content_box.set_margin_bottom(12)
        content_box.set_margin_start(16)
        content_box.set_margin_end(16)

        # Search Entry
        self._search_entry = Gtk.SearchEntry()
        self._search_entry.set_placeholder_text(
            _("Buscar comando por texto, diretório ou parâmetros...")
        )
        self._search_entry.add_css_class("command-history-search-entry")
        self._search_entry.connect("search-changed", self._on_search_changed)
        self._search_entry.connect("activate", self._on_search_activate)
        content_box.append(self._search_entry)

        # Filter Pills Bar
        filter_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        filter_box.set_margin_top(4)
        filter_box.set_margin_bottom(4)

        self._btn_all = Gtk.ToggleButton(label=_("Todos"))
        self._btn_all.add_css_class("pill")
        self._btn_all.set_active(True)
        self._btn_all.connect("toggled", lambda b: self._on_filter_toggled(b, "all"))
        filter_box.append(self._btn_all)

        display_cwd = self.current_cwd
        home = str(Path.home())
        if display_cwd.startswith(home):
            display_cwd = "~" + display_cwd[len(home) :]
        if len(display_cwd) > 20:
            display_cwd = "..." + display_cwd[-18:]

        self._btn_cwd = Gtk.ToggleButton(label=f"📁 {display_cwd}")
        self._btn_cwd.set_tooltip_text(f"{_('Filtrar por diretório:')} {self.current_cwd}")
        self._btn_cwd.add_css_class("pill")
        self._btn_cwd.connect("toggled", lambda b: self._on_filter_toggled(b, "cwd"))
        filter_box.append(self._btn_cwd)

        if self.current_host != "localhost":
            self._btn_host = Gtk.ToggleButton(label=f"🖥️ {self.current_host}")
            self._btn_host.add_css_class("pill")
            self._btn_host.connect(
                "toggled", lambda b: self._on_filter_toggled(b, "host")
            )
            filter_box.append(self._btn_host)
        else:
            self._btn_host = None

        self._btn_pinned = Gtk.ToggleButton(label=_("⭐ Favoritos"))
        self._btn_pinned.add_css_class("pill")
        self._btn_pinned.connect(
            "toggled", lambda b: self._on_filter_toggled(b, "pinned")
        )
        filter_box.append(self._btn_pinned)

        # Stats label on right side
        self._stats_label = Gtk.Label(label="")
        self._stats_label.add_css_class("caption")
        self._stats_label.add_css_class("dim-label")
        self._stats_label.set_hexpand(True)
        self._stats_label.set_halign(Gtk.Align.END)
        filter_box.append(self._stats_label)

        content_box.append(filter_box)

        # Scrolled List Box
        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_hexpand(True)
        scrolled.set_min_content_height(340)
        scrolled.add_css_class("command-history-scrolled")

        self._list_box = Gtk.ListBox()
        self._list_box.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self._list_box.add_css_class("boxed-list")
        self._list_box.connect("row-activated", self._on_row_activated)
        scrolled.set_child(self._list_box)

        content_box.append(scrolled)

        # Footer shortcut hints
        footer_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=16)
        footer_box.set_margin_top(4)
        footer_box.add_css_class("caption")
        footer_box.add_css_class("dim-label")

        hint_enter = Gtk.Label(label=_("↵ Executar"))
        hint_tab = Gtk.Label(label=_("Tab Inserir no Prompt"))
        hint_pin = Gtk.Label(label=_("Ctrl+P Favoritar"))
        hint_del = Gtk.Label(label=_("Del Excluir"))
        hint_esc = Gtk.Label(label=_("Esc Fechar"))

        footer_box.append(hint_enter)
        footer_box.append(hint_tab)
        footer_box.append(hint_pin)
        footer_box.append(hint_del)
        footer_box.append(hint_esc)

        content_box.append(footer_box)
        toolbar_view.set_content(content_box)

    def _setup_key_controller(self) -> None:
        """Configures keyboard navigation for dialog."""
        key_controller = Gtk.EventControllerKey()
        key_controller.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
        key_controller.connect("key-pressed", self._on_key_pressed)
        self.add_controller(key_controller)

    def _on_key_pressed(
        self,
        _controller: Gtk.EventControllerKey,
        keyval: int,
        _keycode: int,
        state: Gdk.ModifierType,
    ) -> bool:
        """Handles keyboard shortcuts."""
        # 1. Escape closes
        if keyval == Gdk.KEY_Escape:
            self.close()
            return True

        # 2. Ctrl + P toggles pin on selected item
        if (state & Gdk.ModifierType.CONTROL_MASK) and keyval in (
            Gdk.KEY_p,
            Gdk.KEY_P,
        ):
            selected_row = self._list_box.get_selected_row()
            if selected_row and hasattr(selected_row, "_history_item"):
                self._toggle_pin_item(selected_row._history_item)
                return True

        # 3. Tab or Shift + Enter inserts command without running
        if keyval == Gdk.KEY_Tab or (
            (state & Gdk.ModifierType.SHIFT_MASK)
            and keyval in (Gdk.KEY_Return, Gdk.KEY_KP_Enter)
        ):
            selected_row = self._list_box.get_selected_row()
            if not selected_row:
                selected_row = self._list_box.get_row_at_index(0)
            if selected_row and hasattr(selected_row, "_history_item"):
                self._apply_command(selected_row._history_item.command, execute=False)
                return True

        # 4. Delete key deletes selected item
        if keyval == Gdk.KEY_Delete:
            selected_row = self._list_box.get_selected_row()
            if selected_row and hasattr(selected_row, "_history_item"):
                self._delete_item(selected_row._history_item)
                return True

        # 5. Up / Down arrow navigation between rows
        if keyval in (Gdk.KEY_Up, Gdk.KEY_Down):
            current_row = self._list_box.get_selected_row()
            current_index = current_row.get_index() if current_row else -1

            if keyval == Gdk.KEY_Down:
                next_index = current_index + 1
            else:
                next_index = max(current_index - 1, 0)

            target_row = self._list_box.get_row_at_index(next_index)
            if target_row:
                self._list_box.select_row(target_row)
                target_row.grab_focus()
                self._search_entry.grab_focus()
            return True

        return False

    def _on_filter_toggled(self, button: Gtk.ToggleButton, filter_type: str) -> None:
        """Handles filter pill selection."""
        if not button.get_active():
            # Prevent unchecking the active pill without checking another
            if self._active_filter == filter_type:
                button.set_active(True)
            return

        self._active_filter = filter_type
        # Untoggle other filter buttons
        for btn, name in [
            (self._btn_all, "all"),
            (self._btn_cwd, "cwd"),
            (self._btn_host, "host"),
            (self._btn_pinned, "pinned"),
        ]:
            if btn and name != filter_type and btn.get_active():
                btn.set_active(False)

        self._reload_items()

    def _on_search_changed(self, _entry: Gtk.SearchEntry) -> None:
        """Triggers live search filtering."""
        self._reload_items()

    def _on_search_activate(self, _entry: Gtk.SearchEntry) -> None:
        """Executes the selected or first command when Enter is pressed in search entry."""
        selected_row = self._list_box.get_selected_row()
        if not selected_row:
            selected_row = self._list_box.get_row_at_index(0)
        if selected_row and hasattr(selected_row, "_history_item"):
            self._apply_command(selected_row._history_item.command, execute=True)

    def _on_row_activated(self, _list_box: Gtk.ListBox, row: Gtk.ListBoxRow) -> None:
        """Executes command on row activation."""
        item: Optional[CommandHistoryItem] = getattr(row, "_history_item", None)
        if item:
            self._apply_command(item.command, execute=True)

    def _reload_items(self) -> None:
        """Queries the history manager and rebuilds list box rows."""
        query = self._search_entry.get_text().strip()
        cwd_filter = self.current_cwd if self._active_filter == "cwd" else None
        host_filter = (
            self.current_host if self._active_filter == "host" else None
        )
        only_pinned = self._active_filter == "pinned"

        self._items = self.history_mgr.search_history(
            query=query,
            cwd=cwd_filter,
            host=host_filter,
            only_pinned=only_pinned,
            limit=150,
        )

        # Clear rows
        while (child := self._list_box.get_first_child()) is not None:
            self._list_box.remove(child)

        for item in self._items:
            row = self._create_row(item)
            self._list_box.append(row)

        # Update stats label
        stats = self.history_mgr.get_stats()
        self._stats_label.set_text(
            f"{len(self._items)} {_('itens')} | {stats.get('total_entries', 0)} {_('total')}"
        )

        # Select first row
        first_row = self._list_box.get_row_at_index(0)
        if first_row:
            self._list_box.select_row(first_row)

    def _create_row(self, item: CommandHistoryItem) -> Gtk.ListBoxRow:
        """Builds a rich UI row widget for a command history item."""
        row = Gtk.ListBoxRow()
        row._history_item = item

        main_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        main_box.set_margin_top(6)
        main_box.set_margin_bottom(6)
        main_box.set_margin_start(10)
        main_box.set_margin_end(10)

        # Pin / Favorite toggle button
        star_icon = "starred-symbolic" if item.is_pinned else "non-starred-symbolic"
        pin_btn = icon_button(
            star_icon,
            size=14,
            tooltip=_("Fixar / Desafixar comando"),
        )
        pin_btn.add_css_class("flat")
        if item.is_pinned:
            pin_btn.add_css_class("accent")
        pin_btn.connect("clicked", lambda _, i=item: self._toggle_pin_item(i))
        main_box.append(pin_btn)

        # Center: Command text & metadata badges
        text_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=3)
        text_box.set_hexpand(True)
        text_box.set_valign(Gtk.Align.CENTER)

        # Command text
        cmd_label = Gtk.Label(label=item.command)
        cmd_label.set_halign(Gtk.Align.START)
        cmd_label.set_ellipsize(Pango.EllipsizeMode.END)
        cmd_label.add_css_class("monospace")
        cmd_label.add_css_class("heading")
        text_box.append(cmd_label)

        # Metadata line
        meta_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        meta_box.add_css_class("caption")
        meta_box.add_css_class("dim-label")

        # CWD badge
        if item.display_cwd:
            cwd_label = Gtk.Label(label=f"📁 {item.display_cwd}")
            cwd_label.set_ellipsize(Pango.EllipsizeMode.MIDDLE)
            meta_box.append(cwd_label)

        # Status badge
        if item.exit_code is not None:
            if item.exit_code == 0:
                status_label = Gtk.Label(label="✓ 0")
                status_label.add_css_class("success")
            else:
                status_label = Gtk.Label(label=f"✗ {item.exit_code}")
                status_label.add_css_class("error")
            meta_box.append(status_label)

        # Duration badge
        if item.formatted_duration:
            dur_label = Gtk.Label(label=f"⏱ {item.formatted_duration}")
            meta_box.append(dur_label)

        # Execution count badge
        if item.execution_count > 1:
            count_label = Gtk.Label(label=f"×{item.execution_count}")
            count_label.add_css_class("badge")
            meta_box.append(count_label)

        # Relative time badge
        time_label = Gtk.Label(label=item.formatted_relative_time)
        meta_box.append(time_label)

        text_box.append(meta_box)
        main_box.append(text_box)

        # Action Buttons on Right
        actions_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        actions_box.set_valign(Gtk.Align.CENTER)

        # 1. Copy button
        copy_btn = icon_button(
            "edit-copy-symbolic", size=14, tooltip=_("Copiar comando")
        )
        copy_btn.add_css_class("flat")
        copy_btn.connect("clicked", lambda _, c=item.command: self._copy_command(c))
        actions_box.append(copy_btn)

        # 2. Insert to prompt button (Edit)
        insert_btn = icon_button(
            "document-edit-symbolic",
            size=14,
            tooltip=_("Inserir no prompt para edição (Tab)"),
        )
        insert_btn.add_css_class("flat")
        insert_btn.connect(
            "clicked",
            lambda _, c=item.command: self._apply_command(c, execute=False),
        )
        actions_box.append(insert_btn)

        # 3. Run button
        run_btn = icon_button(
            "media-playback-start-symbolic",
            size=14,
            tooltip=_("Executar agora (Enter)"),
        )
        run_btn.add_css_class("flat")
        run_btn.add_css_class("suggested-action")
        run_btn.connect(
            "clicked", lambda _, c=item.command: self._apply_command(c, execute=True)
        )
        actions_box.append(run_btn)

        # 4. Delete button
        del_btn = icon_button(
            "user-trash-symbolic", size=14, tooltip=_("Excluir do histórico (Del)")
        )
        del_btn.add_css_class("flat")
        del_btn.connect("clicked", lambda _, i=item: self._delete_item(i))
        actions_box.append(del_btn)

        main_box.append(actions_box)
        row.set_child(main_box)
        return row

    def _toggle_pin_item(self, item: CommandHistoryItem) -> None:
        """Toggles pin status and reloads list."""
        self.history_mgr.toggle_pin(item.id)
        self._reload_items()

    def _delete_item(self, item: CommandHistoryItem) -> None:
        """Deletes item and reloads list."""
        self.history_mgr.delete_entry(item.id)
        self._reload_items()

    def _copy_command(self, command_text: str) -> None:
        """Copies command string to system clipboard."""
        clipboard = Gdk.Display.get_default().get_clipboard()
        clipboard.set(command_text)
        if hasattr(self.parent_window, "toast_overlay"):
            toast = Adw.Toast.new(_("Comando copiado para a área de transferência"))
            toast.set_timeout(2)
            self.parent_window.toast_overlay.add_toast(toast)

    def _apply_command(self, command_text: str, execute: bool = True) -> None:
        """Inserts or executes the command in the terminal and closes the dialog."""
        self.close()
        if self.on_insert_callback:
            self.on_insert_callback(command_text, execute)
        elif self.current_terminal:
            if execute:
                self.current_terminal.feed_child(command_text.encode("utf-8") + b"\n")
            else:
                self.current_terminal.feed_child(command_text.encode("utf-8"))

    def _on_clear_clicked(self, _btn: Gtk.Button) -> None:
        """Prompts confirmation dialog to clear history."""
        dialog = Adw.MessageDialog(
            transient_for=self,
            heading=_("Limpar Histórico de Comandos"),
            body=_(
                "Deseja limpar os comandos do histórico? Comandos favoritos (fixados) serão preservados."
            ),
        )
        dialog.add_response("cancel", _("Cancelar"))
        dialog.add_response("failed", _("Limpar com Erro"))
        dialog.add_response("all", _("Limpar Não Favoritos"))
        dialog.set_response_appearance("all", Adw.ResponseAppearance.DESTRUCTIVE)

        def _on_response(_d, response_id):
            if response_id == "all":
                self.history_mgr.clear_history(scope="all")
                self._reload_items()
            elif response_id == "failed":
                self.history_mgr.clear_history(scope="failed")
                self._reload_items()

        dialog.connect("response", _on_response)
        dialog.present()
