# onyxsh/ui/dialogs/git_commit_dialog.py

"""
Modern modal dialog for AI-assisted Git commit creation and secret leak auditing.
Generates Conventional Commits, audits staged diffs for credentials, and commits directly.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk, Pango

from ...terminal.git_assistant import GitCommitAssistant
from ...utils.git_utils import (
    audit_diff_for_secrets,
    clean_file_uri_to_path,
    commit_changes,
    get_git_diff,
    get_git_status,
    get_repo_root,
    is_git_repository,
    stage_all_files,
    unstage_all_files,
)
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .base_dialog import BaseDialog


class GitCommitDialog(BaseDialog):
    """Dialog for inspecting Git status, auditing secrets, and generating AI commits."""

    def __init__(
        self,
        parent_window: Gtk.Window,
        ai_assistant: Any,
        repo_cwd: Optional[Path | str] = None,
        on_committed: Optional[Callable[[str], None]] = None,
    ) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Assistente Git — Conventional Commits"),
            auto_setup_toolbar=True,
            default_width=780,
            default_height=640,
        )
        self.logger = get_logger("onyxsh.ui.dialogs.git_commit_dialog")
        self.parent_window = parent_window
        self.ai_assistant = ai_assistant
        self.git_assistant = GitCommitAssistant(ai_assistant)
        self.on_committed = on_committed

        # Determine target repository directory (clean URI from any semantic query tags)
        clean_path = clean_file_uri_to_path(repo_cwd)
        self.repo_cwd = Path(clean_path).resolve()
        self.repo_root = get_repo_root(self.repo_cwd) or self.repo_cwd

        self._generating = False
        self._secret_findings: List[Dict[str, Any]] = []

        self._build_ui()
        self._refresh_status()

    def _build_ui(self) -> None:
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10)
        main_box.set_margin_top(12)
        main_box.set_margin_bottom(16)
        main_box.set_margin_start(16)
        main_box.set_margin_end(16)

        # 1. Repository Header Info
        header_card = Adw.PreferencesGroup()
        main_box.append(header_card)

        self._repo_row = Adw.ActionRow(
            title=f"📁 {self.repo_root.name}",
            subtitle=str(self.repo_root),
        )
        self._branch_badge = Gtk.Label(label="🌿 main")
        self._branch_badge.add_css_class("heading")
        self._branch_badge.add_css_class("accent")
        self._branch_badge.set_valign(Gtk.Align.CENTER)
        self._repo_row.add_suffix(self._branch_badge)
        header_card.add(self._repo_row)

        # 2. Secret Guard Banner (Hidden by default, shown if leaked credentials found)
        self._secret_banner = Adw.Banner()
        self._secret_banner.set_title(
            _(
                "⚠️ Alerta de Segurança: Foram detectados potenciais segredos/chaves de API no diff staged!"
            )
        )
        self._secret_banner.set_revealed(False)
        main_box.append(self._secret_banner)

        # 3. Staging and File Summary Controls
        staging_group = Adw.PreferencesGroup(
            title=_("Status dos Arquivos Modificados"),
        )
        main_box.append(staging_group)

        self._files_summary_row = Adw.ActionRow(
            title=_("Carregando status do repositório..."),
            subtitle="",
        )

        stage_all_btn = Gtk.Button(label=_("Estagiar Tudo (+)"))
        stage_all_btn.add_css_class("flat")
        stage_all_btn.connect("clicked", self._on_stage_all_clicked)
        self._files_summary_row.add_suffix(stage_all_btn)

        unstage_all_btn = Gtk.Button(label=_("Desestagiar Tudo (-)"))
        unstage_all_btn.add_css_class("flat")
        unstage_all_btn.connect("clicked", self._on_unstage_all_clicked)
        self._files_summary_row.add_suffix(unstage_all_btn)

        staging_group.add(self._files_summary_row)

        # 4. Commit Generation Options
        options_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        options_box.set_margin_top(4)

        # Style Combo
        style_label = Gtk.Label(label=_("Formato:"))
        options_box.append(style_label)

        self._style_combo = Gtk.DropDown.new_from_strings(
            [
                _("Conventional Commits (Padrão)"),
                _("Resumido (1 Linha)"),
                _("Detalhado (Com Tópicos e Escopo)"),
            ]
        )
        self._style_combo.set_selected(0)
        options_box.append(self._style_combo)

        # Language Combo
        lang_label = Gtk.Label(label=_("Idioma:"))
        options_box.append(lang_label)

        self._lang_combo = Gtk.DropDown.new_from_strings(
            [
                "Português (pt-BR)",
                "English (en-US)",
            ]
        )
        self._lang_combo.set_selected(0)
        options_box.append(self._lang_combo)

        # Spacer
        spacer = Gtk.Box()
        spacer.set_hexpand(True)
        options_box.append(spacer)

        # Generate Button
        self._generate_btn = Gtk.Button()
        gen_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self._gen_spinner = Gtk.Spinner()
        self._gen_label = Gtk.Label(label=_("✨ Gerar Mensagem com IA"))
        gen_box.append(self._gen_spinner)
        gen_box.append(self._gen_label)
        self._generate_btn.set_child(gen_box)
        self._generate_btn.add_css_class("suggested-action")
        self._generate_btn.connect("clicked", self._on_generate_clicked)
        options_box.append(self._generate_btn)

        main_box.append(options_box)

        # 5. Commit Message Text View Editor
        msg_label = Gtk.Label(label=_("Mensagem de Commit (Editável):"))
        msg_label.set_xalign(0)
        msg_label.add_css_class("heading")
        main_box.append(msg_label)

        text_scrolled = Gtk.ScrolledWindow()
        text_scrolled.set_vexpand(True)
        text_scrolled.set_min_content_height(140)
        text_scrolled.add_css_class("card")

        self._text_view = Gtk.TextView()
        self._text_view.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
        self._text_view.set_monospace(True)
        self._text_view.set_top_margin(10)
        self._text_view.set_bottom_margin(10)
        self._text_view.set_left_margin(12)
        self._text_view.set_right_margin(12)
        self._text_buffer = self._text_view.get_buffer()
        text_scrolled.set_child(self._text_view)
        main_box.append(text_scrolled)

        # 6. Bottom Action Bar
        actions_box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        actions_box.set_margin_top(6)

        refresh_btn = Gtk.Button(label=_("Atualizar Status"))
        refresh_btn.add_css_class("flat")
        refresh_btn.connect("clicked", lambda b: self._refresh_status())
        actions_box.append(refresh_btn)

        bottom_spacer = Gtk.Box()
        bottom_spacer.set_hexpand(True)
        actions_box.append(bottom_spacer)

        copy_btn = Gtk.Button(label=_("Copiar Mensagem"))
        copy_btn.add_css_class("flat")
        copy_btn.connect("clicked", self._on_copy_message_clicked)
        actions_box.append(copy_btn)

        cancel_btn = Gtk.Button(label=_("Cancelar"))
        cancel_btn.connect("clicked", lambda b: self.close())
        actions_box.append(cancel_btn)

        self._commit_btn = Gtk.Button(label=_("🚀 Commitar Agora"))
        self._commit_btn.add_css_class("suggested-action")
        self._commit_btn.connect("clicked", self._on_commit_clicked)
        actions_box.append(self._commit_btn)

        main_box.append(actions_box)
        self.set_body_content(main_box)

    def _refresh_status(self) -> None:
        """Fetches the latest Git status and audits diff for secrets."""
        if not is_git_repository(self.repo_cwd):
            self._files_summary_row.set_title(_("Não é um repositório Git válido."))
            self._files_summary_row.set_subtitle(str(self.repo_cwd))
            self._generate_btn.set_sensitive(False)
            self._commit_btn.set_sensitive(False)
            return

        status = get_git_status(self.repo_root)
        branch = status.get("branch", "main")
        self._branch_badge.set_text(f"🌿 {branch}")

        staged = status.get("staged", [])
        unstaged = status.get("unstaged", [])
        untracked = status.get("untracked", [])

        staged_count = len(staged)
        unstaged_count = len(unstaged) + len(untracked)

        title = _("{staged} arquivos estagiados | {unstaged} modificados não-estagiados").format(
            staged=staged_count, unstaged=unstaged_count
        )
        subtitle = (
            ", ".join(f"{f['status']} {f['path']}" for f in staged[:4])
            if staged
            else _("Nenhum arquivo estagiado no momento.")
        )
        if len(staged) > 4:
            subtitle += f" (+{len(staged)-4} outros)"

        self._files_summary_row.set_title(title)
        self._files_summary_row.set_subtitle(subtitle)

        # Audit staged diff for credentials
        diff_text = get_git_diff(self.repo_root, staged=True)
        if not diff_text:
            diff_text = get_git_diff(self.repo_root, staged=False)

        self._secret_findings = audit_diff_for_secrets(diff_text)
        if self._secret_findings:
            secrets_summary = ", ".join(
                f"{item['type']} em {item['file']}" for item in self._secret_findings[:3]
            )
            self._secret_banner.set_title(
                _(
                    "⚠️ Alerta de Segurança: Possíveis segredos detectados no diff: {secrets}"
                ).format(secrets=secrets_summary)
            )
            self._secret_banner.set_revealed(True)
        else:
            self._secret_banner.set_revealed(False)

        has_changes = bool(staged_count > 0 or unstaged_count > 0)
        self._generate_btn.set_sensitive(has_changes and not self._generating)
        self._commit_btn.set_sensitive(staged_count > 0)

    def _on_stage_all_clicked(self, _button: Gtk.Button) -> None:
        """Stages all changes (git add -A)."""
        stage_all_files(self.repo_root)
        self._refresh_status()

    def _on_unstage_all_clicked(self, _button: Gtk.Button) -> None:
        """Unstages all changes (git reset HEAD)."""
        unstage_all_files(self.repo_root)
        self._refresh_status()

    def _on_generate_clicked(self, _button: Gtk.Button) -> None:
        """Starts asynchronous AI commit generation."""
        if self._generating:
            return

        self._generating = True
        self._gen_spinner.start()
        self._gen_label.set_text(_("Gerando..."))
        self._generate_btn.set_sensitive(False)

        # Get style and language options
        style_idx = self._style_combo.get_selected()
        style_map = {0: "conventional", 1: "short", 2: "detailed"}
        style = style_map.get(style_idx, "conventional")

        lang_idx = self._lang_combo.get_selected()
        lang = "pt-BR" if lang_idx == 0 else "en-US"

        self.git_assistant.generate_commit_message_async(
            cwd=self.repo_root,
            style=style,
            language=lang,
            callback=lambda msg, err: GLib.idle_add(
                self._on_generation_finished, msg, err
            ),
        )

    def _on_generation_finished(
        self, message: Optional[str], error: Optional[str]
    ) -> None:
        """Callback on main thread when AI generation finishes."""
        self._generating = False
        self._gen_spinner.stop()
        self._gen_label.set_text(_("✨ Gerar Mensagem com IA"))
        self._generate_btn.set_sensitive(True)

        if error:
            self._show_toast(_("Erro ao gerar commit: {}").format(error))
            return

        if message:
            self._text_buffer.set_text(message)
            self._commit_btn.set_sensitive(True)

    def _on_copy_message_clicked(self, _button: Gtk.Button) -> None:
        """Copies current commit message to system clipboard."""
        bounds = self._text_buffer.get_bounds()
        text = self._text_buffer.get_text(bounds[0], bounds[1], False).strip()
        if not text:
            self._show_toast(_("Nenhuma mensagem para copiar."))
            return

        clipboard = self.get_display().get_clipboard()
        clipboard.set(text)
        self._show_toast(_("Mensagem copiada para a área de transferência!"))

    def _on_commit_clicked(self, _button: Gtk.Button) -> None:
        """Executes the commit with the message in text buffer."""
        bounds = self._text_buffer.get_bounds()
        message = self._text_buffer.get_text(bounds[0], bounds[1], False).strip()
        if not message:
            self._show_toast(_("Por favor, insira uma mensagem de commit."))
            return

        # Warn if committing while secrets are detected
        if self._secret_findings:
            self.logger.warning("User proceeding with commit despite secret findings.")

        success, output = commit_changes(self.repo_root, message)
        if success:
            self._show_toast(_("✅ Commit realizado com sucesso!"))
            if self.on_committed:
                self.on_committed(message)
            GLib.timeout_add(700, lambda: self.close())
        else:
            self._show_toast(_("Falha no commit: {}").format(output))

    def _show_toast(self, message: str) -> None:
        """Displays a toast notification on the dialog or parent window."""
        if hasattr(self.parent_window, "toast_overlay") and self.parent_window.toast_overlay:
            self.parent_window.toast_overlay.add_toast(Adw.Toast(title=message))
        elif hasattr(self, "add_toast"):
            self.add_toast(Adw.Toast(title=message))
