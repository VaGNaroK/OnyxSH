"""Dialog for reviewing unified diffs before applying staged changes."""

from __future__ import annotations

from pathlib import Path
from typing import Callable, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk, Pango

from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .base_dialog import BaseDialog


class DiffReviewDialog(BaseDialog):
    """Dialog allowing the user to review code/config diffs and decide on applying with backup."""

    def __init__(
        self,
        parent_window,
        target_path: str,
        diff_text: str,
        on_apply: Optional[Callable[[bool], None]] = None,
    ) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Revisão de Alterações — Diff"),
            auto_setup_toolbar=True,
            default_width=750,
            default_height=520,
        )
        self.logger = get_logger("zashterminal.ui.dialogs.diff_review")
        self.target_path = target_path
        self.diff_text = diff_text
        self.on_apply = on_apply

        self._build_content()

    def _build_content(self) -> None:
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        main_box.set_margin_top(16)
        main_box.set_margin_bottom(16)
        main_box.set_margin_start(16)
        main_box.set_margin_end(16)

        # Header info
        info_banner = Adw.Banner()
        info_banner.set_title(_("Arquivo de Destino: {}").format(self.target_path))
        info_banner.set_revealed(True)
        main_box.append(info_banner)

        # Scrolled Text View for Diff
        diff_scrolled = Gtk.ScrolledWindow()
        diff_scrolled.set_vexpand(True)
        diff_scrolled.set_hexpand(True)
        diff_scrolled.set_min_content_height(300)
        diff_scrolled.add_css_class("card")

        self.text_view = Gtk.TextView()
        self.text_view.set_editable(False)
        self.text_view.set_monospace(True)
        self.text_view.set_cursor_visible(False)
        self.text_view.set_left_margin(12)
        self.text_view.set_right_margin(12)
        self.text_view.set_top_margin(12)
        self.text_view.set_bottom_margin(12)

        self._populate_diff_buffer(self.diff_text)
        diff_scrolled.set_child(self.text_view)
        main_box.append(diff_scrolled)

        # Backup Checkbox & Action Box
        bottom_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        bottom_box.set_valign(Gtk.Align.CENTER)

        self.backup_check = Gtk.CheckButton(label=_("Criar backup automático antes de aplicar"))
        self.backup_check.set_active(True)
        self.backup_check.set_hexpand(True)
        bottom_box.append(self.backup_check)

        cancel_btn = Gtk.Button(label=_("Cancelar"))
        cancel_btn.connect("clicked", lambda _b: self.close())
        bottom_box.append(cancel_btn)

        apply_btn = Gtk.Button(label=_("Aplicar Alterações"))
        apply_btn.add_css_class("suggested-action")
        apply_btn.connect("clicked", self._on_apply_clicked)
        bottom_box.append(apply_btn)

        main_box.append(bottom_box)

        # Add to scrolled window of toolbar view
        if self._scrolled_window:
            self._scrolled_window.set_child(main_box)
        else:
            self.set_content(main_box)

    def _populate_diff_buffer(self, diff_text: str) -> None:
        buffer = self.text_view.get_buffer()
        tag_table = buffer.get_tag_table()

        tag_add = Gtk.TextTag.new("diff_add")
        tag_add.set_property("foreground", "#2ecc71")
        tag_table.add(tag_add)

        tag_del = Gtk.TextTag.new("diff_del")
        tag_del.set_property("foreground", "#e74c3c")
        tag_table.add(tag_del)

        tag_hdr = Gtk.TextTag.new("diff_hdr")
        tag_hdr.set_property("foreground", "#3498db")
        tag_hdr.set_property("weight", Pango.Weight.BOLD)
        tag_table.add(tag_hdr)

        for line in diff_text.splitlines(keepends=True):
            iter_end = buffer.get_end_iter()
            if line.startswith("+") and not line.startswith("+++"):
                buffer.insert_with_tags(iter_end, line, tag_add)
            elif line.startswith("-") and not line.startswith("---"):
                buffer.insert_with_tags(iter_end, line, tag_del)
            elif line.startswith("@@") or line.startswith("---") or line.startswith("+++"):
                buffer.insert_with_tags(iter_end, line, tag_hdr)
            else:
                buffer.insert(iter_end, line)

    def _on_apply_clicked(self, _button: Gtk.Button) -> None:
        create_backup = self.backup_check.get_active()
        self.close()
        if self.on_apply:
            self.on_apply(create_backup)
