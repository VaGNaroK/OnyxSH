# onyxsh/ui/dialogs/diagnostics_dialog.py
"""
Secure System Diagnostics viewer dialog for OnyxSH.

Displays sanitized system environment, stack versions, AI configuration status
and recent logs, allowing the user to copy or save the report for GitHub issues.
"""

from __future__ import annotations

import threading
from datetime import datetime
from typing import Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk

from ...utils.diagnostics import SystemDiagnostics
from ...utils.logger import get_logger
from ...utils.translation_utils import _


class SystemDiagnosticsDialog(Adw.Window):
    """Modern modal dialog for viewing and exporting sanitized system diagnostics."""

    def __init__(self, parent_window: Optional[Gtk.Window] = None) -> None:
        super().__init__(
            title=_("Diagnóstico do Sistema"),
            transient_for=parent_window,
            modal=True,
            default_width=720,
            default_height=560,
        )
        self.logger = get_logger("onyxsh.ui.dialogs.diagnostics")
        self._report_markdown = ""
        self._report_json = ""
        self._data = {}

        self._build_ui()
        self._load_diagnostics_async()

    def _build_ui(self) -> None:
        """Constructs the dialog layout and widgets."""
        # Main layout container
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        self.set_content(main_box)

        # Header bar
        header = Adw.HeaderBar()
        main_box.append(header)

        # Copy button in header
        self.copy_btn = Gtk.Button(
            icon_name="edit-copy-symbolic",
            tooltip_text=_("Copiar Relatório Markdown"),
        )
        self.copy_btn.connect("clicked", self._on_copy_clicked)
        self.copy_btn.set_sensitive(False)
        header.pack_start(self.copy_btn)

        # Save button in header
        self.save_btn = Gtk.Button(
            icon_name="document-save-symbolic",
            tooltip_text=_("Salvar Relatório em Arquivo..."),
        )
        self.save_btn.connect("clicked", self._on_save_clicked)
        self.save_btn.set_sensitive(False)
        header.pack_start(self.save_btn)

        # Format switch (Markdown / JSON)
        self.format_toggle = Gtk.ToggleButton(
            label="JSON",
            tooltip_text=_("Alternar entre formato Markdown e JSON"),
        )
        self.format_toggle.connect("toggled", self._on_format_toggled)
        self.format_toggle.set_sensitive(False)
        header.pack_end(self.format_toggle)

        # Toast overlay for user feedback
        self.toast_overlay = Adw.ToastOverlay()
        main_box.append(self.toast_overlay)

        # Content container
        content_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        content_box.set_margin_start(16)
        content_box.set_margin_end(16)
        content_box.set_margin_top(8)
        content_box.set_margin_bottom(16)
        self.toast_overlay.set_child(content_box)

        # Info Banner
        info_banner = Adw.Banner(
            title=_("Relatório técnico sanitizado: senhas, caminhos e IPs foram mascarados automaticamente."),
            revealed=True,
        )
        content_box.append(info_banner)

        # Loading spinner
        self.spinner = Gtk.Spinner()
        self.spinner.set_spinning(True)
        self.spinner.set_halign(Gtk.Align.CENTER)
        self.spinner.set_valign(Gtk.Align.CENTER)
        self.spinner.set_vexpand(True)
        content_box.append(self.spinner)

        # Scrolled Text View
        self.scrolled_window = Gtk.ScrolledWindow()
        self.scrolled_window.set_vexpand(True)
        self.scrolled_window.set_hexpand(True)
        self.scrolled_window.set_visible(False)
        content_box.append(self.scrolled_window)

        self.text_view = Gtk.TextView()
        self.text_view.set_editable(False)
        self.text_view.set_cursor_visible(False)
        self.text_view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self.text_view.set_monospace(True)
        self.text_view.add_css_class("card")
        self.text_view.set_margin_start(4)
        self.text_view.set_margin_end(4)
        self.text_view.set_margin_top(4)
        self.text_view.set_margin_bottom(4)

        self.scrolled_window.set_child(self.text_view)

    def _show_toast(self, message: str) -> None:
        """Displays a temporary toast notification."""
        toast = Adw.Toast(title=message)
        self.toast_overlay.add_toast(toast)

    def _load_diagnostics_async(self) -> None:
        """Runs the diagnostics data collection in a background thread."""
        def worker():
            data = SystemDiagnostics.collect_system_data(log_lines=50)
            md_text = SystemDiagnostics.generate_markdown_report(data)
            json_text = SystemDiagnostics.generate_json_report(data)

            def update_ui():
                self._data = data
                self._report_markdown = md_text
                self._report_json = json_text

                buffer = self.text_view.get_buffer()
                buffer.set_text(self._report_markdown)

                self.spinner.set_spinning(False)
                self.spinner.set_visible(False)
                self.scrolled_window.set_visible(True)

                self.copy_btn.set_sensitive(True)
                self.save_btn.set_sensitive(True)
                self.format_toggle.set_sensitive(True)

            GLib.idle_add(update_ui)

        threading.Thread(target=worker, daemon=True).start()

    def _on_format_toggled(self, button: Gtk.ToggleButton) -> None:
        """Switches between Markdown and JSON view."""
        buffer = self.text_view.get_buffer()
        if button.get_active():
            button.set_label("Markdown")
            buffer.set_text(self._report_json)
        else:
            button.set_label("JSON")
            buffer.set_text(self._report_markdown)

    def _on_copy_clicked(self, _button: Gtk.Button) -> None:
        """Copies the currently displayed text to the system clipboard."""
        is_json = self.format_toggle.get_active()
        text_to_copy = self._report_json if is_json else self._report_markdown

        if not text_to_copy:
            return

        display = Gdk.Display.get_default()
        if display:
            clipboard = display.get_clipboard()
            clipboard.set(text_to_copy)
            self._show_toast(_("Relatório copiado para a área de transferência!"))

    def _on_save_clicked(self, _button: Gtk.Button) -> None:
        """Opens file dialog to save report as .md or .json file."""
        is_json = self.format_toggle.get_active()
        content = self._report_json if is_json else self._report_markdown
        ext = ".json" if is_json else ".md"
        filter_name = _("JSON files (*.json)") if is_json else _("Markdown files (*.md)")
        mime = "application/json" if is_json else "text/markdown"
        pattern = "*.json" if is_json else "*.md"

        timestamp_str = datetime.now().strftime("%Y%m%d_%H%M%S")
        default_filename = f"onyxsh_diagnostics_{timestamp_str}{ext}"

        file_dialog = Gtk.FileDialog(
            title=_("Salvar Relatório de Diagnóstico"),
            modal=True,
            initial_name=default_filename,
        )

        filters = Gio.ListStore.new(Gtk.FileFilter)
        target_filter = Gtk.FileFilter()
        target_filter.set_name(filter_name)
        target_filter.add_pattern(pattern)
        target_filter.add_mime_type(mime)
        filters.append(target_filter)

        all_filter = Gtk.FileFilter()
        all_filter.set_name(_("All files"))
        all_filter.add_pattern("*")
        filters.append(all_filter)

        file_dialog.set_filters(filters)
        file_dialog.set_default_filter(target_filter)

        def on_save_finish(dialog, result):
            try:
                gfile = dialog.save_finish(result)
                if gfile:
                    filepath = gfile.get_path()
                    if filepath:
                        with open(filepath, "w", encoding="utf-8") as f:
                            f.write(content)
                        self._show_toast(_("Relatório salvo com sucesso em: {}").format(filepath))
            except GLib.Error as e:
                if not e.matches(Gtk.DialogError.quark(), Gtk.DialogError.DISMISSED):
                    self.logger.error(f"Failed to save diagnostic report: {e}")
                    self._show_toast(_("Erro ao salvar relatório: {}").format(e.message))
            except Exception as e:
                self.logger.error(f"Failed to save diagnostic report: {e}")
                self._show_toast(_("Erro ao salvar relatório: {}").format(e))

        file_dialog.save(self, None, on_save_finish)
