"""Dialog for reviewing agent execution audit logs and performing rollbacks."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Optional

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Adw", "1")
from gi.repository import Adw, Gdk, GLib, Gtk, Pango

from ...agent.models import AuditRecord
from ...utils.logger import get_logger
from ...utils.translation_utils import _
from .base_dialog import BaseDialog


AUDIT_LOG_FILE = Path.home() / ".local" / "share" / "onyxsh" / "audit" / "audit.jsonl"


def _risk_badge_markup(risk: int) -> str:
    badges = {
        0: '<span foreground="#2ecc71"><b>🟢 Nível 0 (Leitura)</b></span>',
        1: '<span foreground="#3498db"><b>🔵 Nível 1 (Escrita)</b></span>',
        2: '<span foreground="#e67e22"><b>🟠 Nível 2 (Admin)</b></span>',
        3: '<span foreground="#e74c3c"><b>🔴 Nível 3 (Crítico)</b></span>',
        4: '<span foreground="#95a5a6"><b>⛔ Nível 4 (Bloqueado)</b></span>',
    }
    return badges.get(risk, f"<b>Nível {risk}</b>")


class AuditLogDialog(BaseDialog):
    """Dialog displaying historical agent actions, security audits, and rollback options."""

    def __init__(self, parent_window) -> None:
        super().__init__(
            parent_window=parent_window,
            dialog_title=_("Registro de Auditoria do Agente"),
            auto_setup_toolbar=True,
            default_width=850,
            default_height=560,
        )
        self.logger = get_logger("onyxsh.ui.dialogs.audit_log")
        self._build_content()

    def _build_content(self) -> None:
        main_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        main_box.set_margin_top(16)
        main_box.set_margin_bottom(16)
        main_box.set_margin_start(16)
        main_box.set_margin_end(16)

        # Header description
        desc_label = Gtk.Label()
        desc_label.set_markup(
            _("Trilha contínua de auditoria das decisões, comandos avaliados e ações executadas pelo <b>Modo Agente Seguro</b>.")
        )
        desc_label.set_xalign(0)
        main_box.append(desc_label)

        # Preferences group / list
        self.list_group = Adw.PreferencesGroup()
        self.list_group.set_title(_("Histórico de Ações"))

        self._populate_records()

        scrolled = Gtk.ScrolledWindow()
        scrolled.set_vexpand(True)
        scrolled.set_hexpand(True)
        scrolled.set_child(self.list_group)
        main_box.append(scrolled)

        if self._scrolled_window:
            self._scrolled_window.set_child(main_box)
        else:
            self.set_content(main_box)

    def _populate_records(self) -> None:
        if not AUDIT_LOG_FILE.exists():
            row = Adw.ActionRow()
            row.set_title(_("Nenhum registro de auditoria encontrado"))
            row.set_subtitle(_("As ações executadas pelo agente serão registradas aqui automaticamente."))
            self.list_group.add(row)
            return

        records: list[AuditRecord] = []
        try:
            with open(AUDIT_LOG_FILE, "r", encoding="utf-8") as f:
                for line in f:
                    if line.strip():
                        try:
                            records.append(AuditRecord.from_dict(json.loads(line)))
                        except Exception:
                            continue
        except Exception as e:
            self.logger.error("Failed to read audit log file: %s", e)

        if not records:
            row = Adw.ActionRow()
            row.set_title(_("Nenhum registro gravado"))
            self.list_group.add(row)
            return

        # Show newest first
        from ...utils.backup import list_file_backups, rollback_file_backup
        backups = {b.get("plan_id", ""): b for b in list_file_backups() if b.get("plan_id")}

        for rec in reversed(records[-50:]):
            cmd_display = " ".join(rec.argv) if rec.argv else rec.tool
            row = Adw.ActionRow()
            row.set_title(cmd_display)
            row.set_subtitle(
                f"{rec.timestamp} | Decisão: {rec.user_decision} | Status: {rec.result_status}"
            )

            # Check if this record has an associated backup
            matching_backup = backups.get(rec.plan_id)
            if matching_backup:
                rollback_btn = Gtk.Button(label=_("Desfazer"))
                rollback_btn.set_valign(Gtk.Align.CENTER)
                rollback_btn.add_css_class("destructive-action")
                backup_id = matching_backup["backup_id"]
                rollback_btn.connect("clicked", self._on_rollback_clicked, backup_id, matching_backup["target_path"])
                row.add_suffix(rollback_btn)

            badge_label = Gtk.Label()
            badge_label.set_markup(_risk_badge_markup(rec.risk))
            row.add_suffix(badge_label)

            self.list_group.add(row)

    def _on_rollback_clicked(self, button: Gtk.Button, backup_id: str, target_path: str) -> None:
        """Handle rollback request for a backup."""
        from ...utils.backup import rollback_file_backup
        try:
            rollback_file_backup(backup_id)
            button.set_sensitive(False)
            button.set_label(_("Restaurado ✓"))
            self.logger.info("Rollback executado com sucesso para %s (%s)", target_path, backup_id)
        except Exception as e:
            self.logger.error("Falha no rollback: %s", e)
