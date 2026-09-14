# onyxsh/ui/dialogs/export_dialog.py
"""
Export dialog allowing users to save or copy the terminal buffer in multiple formats:
- Direct Export: Plain Text (.txt), Log File (.log), Markdown (.md), HTML (.html), Asciinema v2 (.cast)
- Caderno de Bordo / Runbook Export: Structured procedures with annotations, objectives and conclusions (.md, .html, .log)
"""

import os
import pathlib
import time
from typing import Any, Dict, List, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
gi.require_version("Vte", "3.91")
from gi.repository import Adw, Gdk, Gio, GLib, Gtk, Vte

from ...terminal.exporter import get_terminal_exporter
from ...terminal.runbook import RunbookData, RunbookStep, get_runbook_generator
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .base_dialog import BaseDialog


class ExportTerminalDialog(BaseDialog):
    """Modern Libadwaita dialog for exporting terminal output and creating interactive Runbooks."""

    FORMAT_OPTIONS = [
        ("txt", _("Plain Text (.txt)"), _("Clean unformatted terminal output"), ".txt", "text/plain"),
        ("log", _("Log File (.log)"), _("Output with session, date and system headers"), ".log", "text/plain"),
        ("md", _("Markdown Document (.md)"), _("Formatted with code blocks and metadata table"), ".md", "text/markdown"),
        ("html", _("Styled HTML Page (.html)"), _("Self-contained modern dark-themed webpage"), ".html", "text/html"),
        ("cast", _("Asciinema Recording (.cast)"), _("Asciinema v2 JSON format for CLI & web players"), ".cast", "application/json"),
    ]

    RUNBOOK_FORMAT_OPTIONS = [
        ("runbook_md", _("Markdown Runbook (.md)"), _("Documentation format with callouts, badges and steps"), ".md", "text/markdown"),
        ("runbook_html", _("Interactive HTML Report (.html)"), _("Dark-themed standalone webpage with print / PDF styling"), ".html", "text/html"),
        ("runbook_log", _("Structured Runbook Log (.log)"), _("Text report for ticketing systems and audit attachments"), ".log", "text/plain"),
    ]

    def __init__(
        self,
        parent_window: Any,
        terminal: Vte.Terminal,
        start_in_runbook_mode: bool = False,
    ) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Export Terminal & Runbook"),
            auto_setup_toolbar=True,
            default_width=720,
            default_height=650,
        )
        self.terminal = terminal
        self.exporter = get_terminal_exporter()
        self.runbook_generator = get_runbook_generator()
        self.logger = get_logger("onyxsh.ui.dialogs.export_dialog")

        self.selected_format = "md"
        self.selected_runbook_format = "runbook_md"
        self.selection_only = False
        self.is_runbook_mode = start_in_runbook_mode

        self._has_selection = (
            self.terminal.get_has_selection()
            if hasattr(self.terminal, "get_has_selection")
            else False
        )

        # Initialize runbook data from terminal
        try:
            self.runbook_data = self.runbook_generator.from_terminal(
                self.terminal, selection_only=self.selection_only
            )
        except Exception as e:
            self.logger.error(f"Failed to generate initial runbook data: {e}")
            from ...terminal.runbook import RunbookMetadata
            self.runbook_data = RunbookData(metadata=RunbookMetadata())

        self._build_ui()
        if start_in_runbook_mode:
            self.stack.set_visible_child_name("runbook")

        self._update_preview()

    def _build_ui(self) -> None:
        """Constructs the complete dialog layout."""
        main_box = Gtk.Box(
            orientation=Gtk.Orientation.VERTICAL,
            spacing=14,
            margin_top=14,
            margin_bottom=14,
            margin_start=18,
            margin_end=18,
        )

        # 1. Mode Switcher (Direct Export vs Caderno de Bordo)
        switcher_box = Gtk.Box(
            orientation=Gtk.Orientation.HORIZONTAL,
            halign=Gtk.Align.CENTER,
            margin_bottom=4,
        )
        self.stack_switcher = Gtk.StackSwitcher()
        switcher_box.append(self.stack_switcher)
        main_box.append(switcher_box)

        # 2. Main Stack
        self.stack = Gtk.Stack(
            transition_type=Gtk.StackTransitionType.SLIDE_LEFT_RIGHT,
            transition_duration=200,
        )
        self.stack_switcher.set_stack(self.stack)

        # Page 1: Direct Export
        direct_page = self._build_direct_export_page()
        self.stack.add_titled(direct_page, "direct", _("📜 Exportação Direta"))

        # Page 2: Caderno de Bordo (Runbook)
        runbook_page = self._build_runbook_page()
        self.stack.add_titled(runbook_page, "runbook", _("📘 Caderno de Bordo (Runbook)"))

        self.stack.connect("notify::visible-child-name", self._on_mode_switched)
        main_box.append(self.stack)

        # 3. Live Preview Card (Shared between modes)
        preview_group = Adw.PreferencesGroup(title=_("Pré-visualização do Arquivo"))
        preview_card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["card"])
        preview_scroller = Gtk.ScrolledWindow(
            min_content_height=120,
            max_content_height=150,
            hexpand=True,
            vexpand=False,
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
        main_box.append(preview_group)

        # 4. Action Buttons Bar
        actions_box = Gtk.Box(
            orientation=Gtk.Orientation.HORIZONTAL,
            spacing=10,
            halign=Gtk.Align.END,
            margin_top=6,
        )

        self.copy_button = Gtk.Button(label=_("Copiar para a Área de Transferência"))
        self.copy_button.set_icon_name("edit-copy-symbolic")
        self.copy_button.connect("clicked", self._on_copy_clicked)
        actions_box.append(self.copy_button)

        self.save_button = Gtk.Button(label=_("Salvar como Arquivo..."))
        self.save_button.add_css_class("suggested-action")
        self.save_button.set_icon_name("document-save-symbolic")
        self.save_button.connect("clicked", self._on_save_clicked)
        actions_box.append(self.save_button)

        main_box.append(actions_box)

        # Scrollable container
        scroller = Gtk.ScrolledWindow(
            hscrollbar_policy=Gtk.PolicyType.NEVER,
            vscrollbar_policy=Gtk.PolicyType.AUTOMATIC,
            hexpand=True,
            vexpand=True,
        )
        scroller.set_child(main_box)

        if self._toolbar_view:
            self._toolbar_view.set_content(scroller)
        else:
            self.set_content(scroller)

    def _build_direct_export_page(self) -> Gtk.Widget:
        """Constructs the standard direct export page."""
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)

        # Scope Selection Group
        scope_group = Adw.PreferencesGroup(
            title=_("Escopo da Exportação"),
            description=_("Escolha entre exportar todo o histórico do terminal ou apenas a seleção ativa."),
        )

        self.scope_full_row = Adw.ActionRow(
            title=_("Buffer Completo do Terminal"),
            subtitle=_("Todos os comandos e saídas gravados nesta sessão"),
        )
        self.scope_full_check = Gtk.CheckButton(active=True)
        self.scope_full_check.connect("toggled", self._on_scope_changed)
        self.scope_full_row.add_prefix(self.scope_full_check)
        self.scope_full_row.set_activatable_widget(self.scope_full_check)
        scope_group.add(self.scope_full_row)

        self.scope_sel_row = Adw.ActionRow(
            title=_("Apenas Texto Selecionado"),
            subtitle=_("Apenas o trecho atualmente destacado no terminal")
            if self._has_selection
            else _("Nenhum texto selecionado no terminal"),
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

        box.append(scope_group)

        # Format Selection Group
        format_group = Adw.PreferencesGroup(
            title=_("Formato do Arquivo"),
            description=_("Selecione o formato de destino do texto exportado."),
        )

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

        box.append(format_group)
        return box

    def _build_runbook_page(self) -> Gtk.Widget:
        """Constructs the interactive Runbook (Caderno de Bordo) page."""
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=14)

        # 1. Metadata Group
        meta_group = Adw.PreferencesGroup(
            title=_("Metadados do Caderno de Bordo"),
            description=_("Informações gerais para o cabeçalho do relatório operacional."),
        )

        self.rb_title_row = Adw.EntryRow(title=_("Título do Procedimento"))
        self.rb_title_row.set_text(self.runbook_data.metadata.title)
        self.rb_title_row.connect("changed", self._on_runbook_meta_changed)
        meta_group.add(self.rb_title_row)

        self.rb_author_row = Adw.EntryRow(title=_("Operador / Autor"))
        self.rb_author_row.set_text(self.runbook_data.metadata.author)
        self.rb_author_row.connect("changed", self._on_runbook_meta_changed)
        meta_group.add(self.rb_author_row)

        self.rb_objective_row = Adw.EntryRow(title=_("Objetivo / Contexto"))
        self.rb_objective_row.set_text(self.runbook_data.metadata.objective)
        self.rb_objective_row.connect("changed", self._on_runbook_meta_changed)
        meta_group.add(self.rb_objective_row)

        # Status Dropdown
        self.rb_status_row = Adw.ComboRow(title=_("Status do Procedimento"))
        status_model = Gtk.StringList()
        status_model.append(_("Concluído"))
        status_model.append(_("Em Andamento"))
        status_model.append(_("Incidente Investigado"))
        status_model.append(_("Manutenção Preventiva"))
        self.rb_status_row.set_model(status_model)
        self.rb_status_row.set_selected(0)
        self.rb_status_row.connect("notify::selected", self._on_runbook_status_changed)
        meta_group.add(self.rb_status_row)

        box.append(meta_group)

        # 2. Steps & Annotations List Group
        steps_count = len(self.runbook_data.steps)
        steps_group = Adw.PreferencesGroup(
            title=_("Etapas da Sessão e Anotações"),
            description=_(
                "Selecione os comandos a incluir e adicione comentários técnicos explicativos para cada etapa."
            )
            + f" ({steps_count} {_('identificadas')})",
        )

        if not self.runbook_data.steps:
            empty_row = Adw.ActionRow(
                title=_("Nenhum comando registrado"),
                subtitle=_("Nenhum comando foi executado nesta sessão do terminal ainda."),
            )
            steps_group.add(empty_row)
        else:
            steps_scroller = Gtk.ScrolledWindow(
                min_content_height=140,
                max_content_height=260,
                hexpand=True,
                vexpand=False,
                css_classes=["card"],
            )
            steps_box = Gtk.Box(
                orientation=Gtk.Orientation.VERTICAL,
                spacing=6,
                margin_top=8,
                margin_bottom=8,
                margin_start=8,
                margin_end=8,
            )

            self.step_widgets = []

            for idx, step in enumerate(self.runbook_data.steps, start=1):
                status_str = f"exit {step.exit_code}" if step.exit_code is not None else "OK"
                dur_str = f" • ⏱ {step.duration_str}" if step.duration_str else ""
                time_str = f" • 🕒 {step.timestamp_str}" if step.timestamp_str else ""
                sub_info = f"{status_str}{dur_str}{time_str}"

                safe_cmd = GLib.markup_escape_text(step.command) if step.command else ""
                expander = Adw.ExpanderRow(
                    title=f"#{idx}  {safe_cmd}",
                    subtitle=GLib.markup_escape_text(sub_info),
                )

                check = Gtk.CheckButton(active=step.included)
                check.connect("toggled", self._on_step_included_toggled, step)
                expander.add_prefix(check)

                # Annotation entry inside expander
                entry_row = Adw.EntryRow(title=_("Anotação / Comentário:"))
                entry_row.set_text(step.annotation)
                entry_row.connect("changed", self._on_step_annotation_changed, step)
                expander.add_row(entry_row)

                # Output preview row
                out_snippet = step.output.strip()
                if out_snippet:
                    first_line = out_snippet.splitlines()[0][:90]
                    lines_count = len(out_snippet.splitlines())
                    safe_first_line = GLib.markup_escape_text(first_line)
                    out_row = Adw.ActionRow(
                        title=_("Saída do Comando"),
                        subtitle=f"{safe_first_line}... ({lines_count} {_('linhas')})",
                    )
                    expander.add_row(out_row)

                steps_box.append(expander)
                self.step_widgets.append((expander, check, entry_row, step))

            steps_scroller.set_child(steps_box)
            steps_group.add(steps_scroller)

        box.append(steps_group)

        # 3. Conclusion Group
        conclusion_group = Adw.PreferencesGroup(
            title=GLib.markup_escape_text(_("Conclusão & Lições Aprendidas"))
        )
        self.rb_conclusion_row = Adw.EntryRow(title=_("Notas Finais do Procedimento"))
        self.rb_conclusion_row.set_text(self.runbook_data.metadata.conclusion)
        self.rb_conclusion_row.connect("changed", self._on_runbook_conclusion_changed)
        conclusion_group.add(self.rb_conclusion_row)
        box.append(conclusion_group)

        # 4. Runbook Format & Export Options Group
        options_group = Adw.PreferencesGroup(title=_("Formato do Caderno de Bordo"))
        first_rb_check = None

        for fmt_id, title, desc, _ext, _mime in self.RUNBOOK_FORMAT_OPTIONS:
            row = Adw.ActionRow(
                title=GLib.markup_escape_text(title),
                subtitle=GLib.markup_escape_text(desc),
            )
            if first_rb_check is None:
                check = Gtk.CheckButton(active=(fmt_id == self.selected_runbook_format))
                first_rb_check = check
            else:
                check = Gtk.CheckButton(
                    group=first_rb_check,
                    active=(fmt_id == self.selected_runbook_format),
                )

            check.connect("toggled", self._on_runbook_format_changed, fmt_id)
            row.add_prefix(check)
            row.set_activatable_widget(check)
            options_group.add(row)

        self.collapse_switch = Adw.SwitchRow(
            title=_("Recolher saídas longas de comandos"),
            subtitle=GLib.markup_escape_text(
                _("Usa blocos retráteis (<details>) para saídas com mais de 15 linhas")
            ),
            active=True,
        )
        self.collapse_switch.connect("notify::active", lambda *_: self._update_preview())
        options_group.add(self.collapse_switch)

        box.append(options_group)
        return box

    def _on_mode_switched(self, stack: Gtk.Stack, _param: Any) -> None:
        """Handles tab switching between direct export and runbook."""
        name = stack.get_visible_child_name()
        self.is_runbook_mode = (name == "runbook")
        self._update_preview()

    def _on_scope_changed(self, button: Gtk.CheckButton) -> None:
        """Handles switching between full buffer and active selection in direct export."""
        self.selection_only = self.scope_sel_check.get_active()
        self._update_preview()

    def _on_format_changed(self, button: Gtk.CheckButton, fmt_id: str) -> None:
        """Handles changing format radio button in direct export."""
        if button.get_active():
            self.selected_format = fmt_id
            self._update_preview()

    def _on_runbook_format_changed(self, button: Gtk.CheckButton, fmt_id: str) -> None:
        """Handles changing runbook format."""
        if button.get_active():
            self.selected_runbook_format = fmt_id
            self._update_preview()

    def _on_runbook_meta_changed(self, entry: Adw.EntryRow) -> None:
        """Updates runbook metadata in real time."""
        self.runbook_data.metadata.title = self.rb_title_row.get_text().strip()
        self.runbook_data.metadata.author = self.rb_author_row.get_text().strip()
        self.runbook_data.metadata.objective = self.rb_objective_row.get_text().strip()
        self._update_preview()

    def _on_runbook_status_changed(self, row: Adw.ComboRow, _param: Any) -> None:
        """Updates runbook status string."""
        model = row.get_model()
        sel = row.get_selected()
        if model and sel < model.get_n_items():
            self.runbook_data.metadata.status = model.get_string(sel)
            self._update_preview()

    def _on_runbook_conclusion_changed(self, entry: Adw.EntryRow) -> None:
        """Updates runbook conclusion text."""
        self.runbook_data.metadata.conclusion = self.rb_conclusion_row.get_text().strip()
        self._update_preview()

    def _on_step_included_toggled(self, button: Gtk.CheckButton, step: RunbookStep) -> None:
        """Updates inclusion status of a command step."""
        step.included = button.get_active()
        self._update_preview()

    def _on_step_annotation_changed(self, entry: Adw.EntryRow, step: RunbookStep) -> None:
        """Updates annotation of a command step."""
        step.annotation = entry.get_text()
        self._update_preview()

    def _get_active_content_and_meta(self) -> tuple[str, str, str]:
        """Returns (content_str, extension, mime_type) based on active mode."""
        if self.is_runbook_mode:
            collapse = (
                self.collapse_switch.get_active()
                if hasattr(self, "collapse_switch")
                else True
            )
            return self.exporter.format_content(
                self.terminal,
                self.selected_runbook_format,
                selection_only=False,
                runbook_data=self.runbook_data,
                collapse_long_outputs=collapse,
            )
        else:
            return self.exporter.format_content(
                self.terminal,
                self.selected_format,
                selection_only=self.selection_only,
            )

    def _update_preview(self) -> None:
        """Refreshes the live preview text box."""
        try:
            content, _ext, _mime = self._get_active_content_and_meta()
            lines = content.splitlines()[:25]
            preview_str = "\n".join(lines)
            if len(content.splitlines()) > 25:
                preview_str += (
                    "\n\n... ("
                    + _("restante da saída truncado na pré-visualização")
                    + ")"
                )
            self.preview_buffer.set_text(preview_str)
        except Exception as e:
            self.logger.error(f"Error updating preview: {e}")
            self.preview_buffer.set_text(_("Falha ao renderizar pré-visualização."))

    def _on_copy_clicked(self, _button: Gtk.Button) -> None:
        """Copies the formatted export directly to clipboard."""
        try:
            content, _ext, _mime = self._get_active_content_and_meta()
            clipboard = Gdk.Display.get_default().get_clipboard()
            clipboard.set(content)

            self.copy_button.set_label(_("Copiado!"))
            self.copy_button.set_icon_name("object-select-symbolic")

            if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                msg = (
                    _("Caderno de Bordo copiado para a área de transferência.")
                    if self.is_runbook_mode
                    else _("Saída do terminal copiada para a área de transferência.")
                )
                self.parent_window.toast_overlay.add_toast(Adw.Toast(title=msg))

            GLib.timeout_add(
                2000,
                lambda: (
                    self.copy_button.set_label(
                        _("Copiar para a Área de Transferência")
                    ),
                    self.copy_button.set_icon_name("edit-copy-symbolic"),
                    False,
                )[2],
            )
        except Exception as e:
            self.logger.error(f"Failed to copy export to clipboard: {e}")
            if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                self.parent_window.toast_overlay.add_toast(
                    Adw.Toast(title=_("Erro ao copiar para a área de transferência."))
                )

    def _on_save_clicked(self, _button: Gtk.Button) -> None:
        """Opens native file chooser dialog and writes file to disk."""
        content, ext, mime = self._get_active_content_and_meta()
        timestamp_str = time.strftime("%Y%m%d_%H%M%S")

        prefix = "onyxsh_runbook" if self.is_runbook_mode else "onyxsh_export"
        default_filename = f"{prefix}_{timestamp_str}{ext}"

        title = (
            _("Salvar Caderno de Bordo")
            if self.is_runbook_mode
            else _("Salvar Saída do Terminal")
        )

        if hasattr(Gtk, "FileDialog"):
            file_dialog = Gtk.FileDialog(
                title=title,
                initial_name=default_filename,
            )

            filters = Gio.ListStore.new(Gtk.FileFilter)
            active_filter = Gtk.FileFilter()
            active_filter.set_name(f"*{ext}")
            active_filter.add_pattern(f"*{ext}")
            filters.append(active_filter)

            all_filter = Gtk.FileFilter()
            all_filter.set_name(_("Todos os Arquivos"))
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
            native = Gtk.FileChooserNative.new(
                title,
                self,
                Gtk.FileChooserAction.SAVE,
                _("_Salvar"),
                _("_Cancelar"),
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

            self.logger.info(f"Export successfully saved to: {file_path}")
            if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                toast = Adw.Toast(
                    title=f"{_('Salvo com sucesso:')} {os.path.basename(file_path)}"
                )
                self.parent_window.toast_overlay.add_toast(toast)

            self.close()
        except Exception as e:
            self.logger.error(f"Failed to write export file: {e}")
            if self.parent_window and hasattr(self.parent_window, "toast_overlay"):
                self.parent_window.toast_overlay.add_toast(
                    Adw.Toast(title=f"{_('Falha ao salvar arquivo:')} {e}")
                )
