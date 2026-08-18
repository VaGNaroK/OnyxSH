# onyxsh/ui/dialogs/export_dialog.py
"""
Export dialog allowing users to save or copy the terminal buffer in multiple formats:
- Plain Text (.txt)
- Log File (.log)
- Markdown (.md)
- HTML (.html)
- Asciinema v2 (.cast)
"""

import os
import pathlib
import time
from typing import Any, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Vte", "3.91")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk, Vte

from ...terminal.exporter import get_terminal_exporter
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .base_dialog import BaseDialog


class ExportTerminalDialog(BaseDialog):
    """Modern Libadwaita dialog for exporting terminal output in multiple formats."""

    FORMAT_OPTIONS = [
        ("txt", _("Plain Text (.txt)"), _("Clean unformatted terminal output"), ".txt", "text/plain"),
        ("log", _("Log File (.log)"), _("Output with session, date and system headers"), ".log", "text/plain"),
        ("md", _("Markdown Document (.md)"), _("Formatted with code blocks and metadata table"), ".md", "text/markdown"),
        ("html", _("Styled HTML Page (.html)"), _("Self-contained modern dark-themed webpage"), ".html", "text/html"),
        ("cast", _("Asciinema Recording (.cast)"), _("Asciinema v2 JSON format for CLI & web players"), ".cast", "application/json"),
    ]

    def __init__(self, parent_window: Any, terminal: Vte.Terminal) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Export Terminal Output"),
            auto_setup_toolbar=True,
            default_width=620,
            default_height=540,
        )
        self.terminal = terminal
        self.exporter = get_terminal_exporter()
        self.logger = get_logger("onyxsh.ui.dialogs.export_dialog")

        self.selected_format = "md"  # Default to Markdown
        self.selection_only = False

        self._has_selection = (
            self.terminal.get_has_selection()
            if hasattr(self.terminal, "get_has_selection")
            else False
        )

        self._build_ui()
        self._update_preview()

    def _build_ui(self) -> None:
        """Constructs the dialog layout."""
        content_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=16,
            margin_top=16,
            margin_bottom=16,
            margin_start=20,
            margin_end=20,
        )

        # 1. Scope Selection Group (Full Buffer vs Selection)
        scope_group = Adw.PreferencesGroup(
            title=_("Export Scope"),
            description=_("Choose whether to export the entire scrollback history or active selection."),
        )

        self.scope_full_row = Adw.ActionRow(
            title=_("Full Scrollback Buffer"),
            subtitle=_("All commands and output in this terminal session"),
        )
        self.scope_full_check = Gtk.CheckButton(active=True)
        self.scope_full_check.connect("toggled", self._on_scope_changed)
        self.scope_full_row.add_prefix(self.scope_full_check)
        self.scope_full_row.set_activatable_widget(self.scope_full_check)
        scope_group.add(self.scope_full_row)

        self.scope_sel_row = Adw.ActionRow(
            title=_("Selected Text Only"),
            subtitle=_("Only the currently highlighted terminal text")
            if self._has_selection
            else _("No text currently selected in the terminal"),
        )
        self.scope_sel_check = Gtk.CheckButton(
            group=self.scope_full_check,
            active=False,
            sensitive=self._has_selection,
        )
        self.scope_sel_check.connect("toggled", self._on_scope_changed)
        self.scope_sel_row.add_prefix(self.scope_sel_check)
        self.scope_sel_row.set_activatable_widget(self.scope_sel_check)
        self.scope_sel_row.set_sensitive(self._has_selection)
        scope_group.add(self.scope_sel_row)

        content_box.append(scope_group)

        # 2. Format Selection Group
        format_group = Adw.PreferencesGroup(
            title=_("File Format"),
            description=_("Select the destination format for the exported output."),
        )

        self.format_rows = {}
        first_check = None

        for fmt_id, title, desc, _ext, _mime in self.FORMAT_OPTIONS:
            row = Adw.ActionRow(title=title, subtitle=desc)
            if first_check is None:
                check = Gtk.CheckButton(active=(fmt_id == self.selected_format))
                first_check = check
            else:
                check = Gtk.CheckButton(
                    group=first_check,
                    active=(fmt_id == self.selected_format),
                )

            check.connect("toggled", self._on_format_changed, fmt_id)
            row.add_prefix(check)
            row.set_activatable_widget(check)
            format_group.add(row)
            self.format_rows[fmt_id] = (row, check)

        content_box.append(format_group)

        # 3. Live Preview Card
        preview_group = Adw.PreferencesGroup(title=_("Preview"))
        preview_card = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            css_classes=["card"],
        )

        preview_scroller = Gtk.ScrolledWindow(
            min_content_height=110,
            max_content_height=140,
            hexpand=True,
            vexpand=True,
        )
        self.preview_text_view = Gtk.TextView(
            editable=False,
            monospace=True,
            wrap_mode=Gtk.WrapMode.NONE,
            top_margin=10,
            bottom_margin=10,
            left_margin=12,
            right_margin=12,
            css_classes=["card"],
        )
        self.preview_buffer = self.preview_text_view.get_buffer()
        preview_scroller.set_child(self.preview_text_view)
        preview_card.append(preview_scroller)
        preview_group.add(preview_card)
        content_box.append(preview_group)

        # 4. Action Buttons Bar
        actions_box = Gtk.Box(
            orientation=Gtk.Orientation.HORIZONTAL,
            spacing=10,
            halign=Gtk.Align.END,
            margin_top=8,
        )

        self.copy_button = Gtk.Button(label=_("Copy to Clipboard"))
        self.copy_button.set_icon_name("edit-copy-symbolic")
        self.copy_button.connect("clicked", self._on_copy_clicked)
        actions_box.append(self.copy_button)

        self.save_button = Gtk.Button(label=_("Save as File..."))
        self.save_button.add_css_class("suggested-action")
        self.save_button.set_icon_name("document-save-symbolic")
        self.save_button.connect("clicked", self._on_save_clicked)
        actions_box.append(self.save_button)

        content_box.append(actions_box)

        # Put everything inside a scrolled window for small screen adaptability
        main_scroller = Gtk.ScrolledWindow(
            hscrollbar_policy=Gtk.PolicyType.NEVER,
            vscrollbar_policy=Gtk.PolicyType.AUTOMATIC,
            hexpand=True,
            vexpand=True,
        )
        main_scroller.set_child(content_box)

        if self._toolbar_view:
            self._toolbar_view.set_content(main_scroller)
        else:
            self.set_content(main_scroller)

    def _on_scope_changed(self, button: Gtk.CheckButton) -> None:
        """Handles switching between full buffer and active selection."""
        if self.scope_sel_check.get_active():
            self.selection_only = True
        else:
            self.selection_only = False
        self._update_preview()

    def _on_format_changed(self, button: Gtk.CheckButton, fmt_id: str) -> None:
        """Handles changing format radio button."""
        if button.get_active():
            self.selected_format = fmt_id
            self._update_preview()

    def _update_preview(self) -> None:
        """Refreshes the live preview text box."""
        try:
            content, _ext, _mime = self.exporter.format_content(
                self.terminal, self.selected_format, self.selection_only
            )
            # Display first 25 lines in preview
            lines = content.splitlines()[:25]
            preview_str = "\n".join(lines)
            if len(content.splitlines()) > 25:
                preview_str += "\n\n... (" + _("rest of output truncated in preview") + ")"
            self.preview_buffer.set_text(preview_str)
        except Exception as e:
            self.logger.error(f"Error updating preview: {e}")
            self.preview_buffer.set_text(_("Failed to render preview."))

    def _on_copy_clicked(self, _button: Gtk.Button) -> None:
        """Copies the formatted export directly to clipboard."""
        try:
            content, _ext, _mime = self.exporter.format_content(
                self.terminal, self.selected_format, self.selection_only
            )
            clipboard = Gdk.Display.get_default().get_clipboard()
            clipboard.set(content)

            self.copy_button.set_label(_("Copied!"))
            self.copy_button.set_icon_name("object-select-symbolic")

            if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                self.parent_window.toast_overlay.add_toast(
                    Adw.Toast(title=_("Terminal output copied to clipboard."))
                )

            GLib.timeout_add(
                2000,
                lambda: (
                    self.copy_button.set_label(_("Copy to Clipboard")),
                    self.copy_button.set_icon_name("edit-copy-symbolic"),
                    False,
                )[2],
            )
        except Exception as e:
            self.logger.error(f"Failed to copy export to clipboard: {e}")
            if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                self.parent_window.toast_overlay.add_toast(
                    Adw.Toast(title=_("Error copying to clipboard."))
                )

    def _on_save_clicked(self, _button: Gtk.Button) -> None:
        """Opens native file chooser dialog and writes file to disk."""
        content, ext, mime = self.exporter.format_content(
            self.terminal, self.selected_format, self.selection_only
        )
        timestamp_str = time.strftime("%Y%m%d_%H%M%S")
        default_filename = f"onyxsh_export_{timestamp_str}{ext}"

        # In modern GTK4 / Libadwaita, use Gtk.FileDialog if available
        if hasattr(Gtk, "FileDialog"):
            file_dialog = Gtk.FileDialog(
                title=_("Save Terminal Output"),
                initial_name=default_filename,
            )

            # Add filter for current format
            filters = Gio.ListStore.new(Gtk.FileFilter)
            active_filter = Gtk.FileFilter()
            active_filter.set_name(f"*{ext}")
            active_filter.add_pattern(f"*{ext}")
            filters.append(active_filter)

            all_filter = Gtk.FileFilter()
            all_filter.set_name(_("All Files"))
            all_filter.add_pattern("*")
            filters.append(all_filter)

            file_dialog.set_filters(filters)
            file_dialog.set_default_filter(active_filter)

            def _on_file_dialog_saved(dialog, result):
                try:
                    file = dialog.save_finish(result)
                    if file:
                        path = file.get_path()
                        if path:
                            self._write_export_file(path, content)
                except GLib.Error as err:
                    if err.code != Gtk.DialogError.DISMISSED:
                        self.logger.error(f"FileDialog error: {err.message}")

            file_dialog.save(self, None, _on_file_dialog_saved)
        else:
            # Fallback to Gtk.FileChooserNative
            native = Gtk.FileChooserNative.new(
                _("Save Terminal Output"),
                self,
                Gtk.FileChooserAction.SAVE,
                _("_Save"),
                _("_Cancel"),
            )
            native.set_current_name(default_filename)

            def _on_native_response(dialog, response):
                if response == Gtk.ResponseType.ACCEPT:
                    file = dialog.get_file()
                    if file and file.get_path():
                        self._write_export_file(file.get_path(), content)
                dialog.destroy()

            native.connect("response", _on_native_response)
            native.show()

    def _write_export_file(self, file_path: str, content: str) -> None:
        """Writes content to disk and displays confirmation toast."""
        try:
            with open(file_path, "w", encoding="utf-8") as f:
                f.write(content)

            self.logger.info(f"Terminal output successfully saved to: {file_path}")
            if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                toast = Adw.Toast(
                    title=f"{_('Saved successfully:')} {os.path.basename(file_path)}"
                )
                self.parent_window.toast_overlay.add_toast(toast)

            self.close()
        except Exception as e:
            self.logger.error(f"Failed to write export file: {e}")
            if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                self.parent_window.toast_overlay.add_toast(
                    Adw.Toast(title=f"{_('Failed to save file:')} {e}")
                )
