//! Phase 15: Agent Audit Log Dialog.
//! Ports `src/onyxsh/ui/dialogs/audit_log_dialog.py`.
//!
//! Displays historical agent decisions, commands evaluated, risk levels, and 1-click atomic rollback.

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::platform;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Individual audit record entry in JSONL format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub plan_id: String,
    #[serde(default)]
    pub step_id: String,
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub argv: Vec<String>,
    #[serde(default)]
    pub risk: i32,
    #[serde(default)]
    pub user_decision: String,
    #[serde(default)]
    pub result_status: String,
    // Optional compatibility fields with Rust AuditRecord:
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub command_or_path: String,
    pub backup_path: Option<String>,
    pub target_path: Option<String>,
}

/// Backup manifest recorded for file modifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    #[serde(default)]
    pub backup_id: String,
    #[serde(default)]
    pub target_path: String,
    #[serde(default)]
    pub backup_path: String,
    #[serde(default)]
    pub plan_id: String,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub sha256_original: String,
}

/// Returns the formatted Pango markup string for risk level badges matching `registrodeauditoria.png`.
pub fn risk_badge_markup(risk: i32) -> String {
    match risk {
        0 => "<span foreground=\"#2ecc71\"><b>🟢 Nível 0 (Leitura)</b></span>".to_string(),
        1 => "<span foreground=\"#3498db\"><b>🔵 Nível 1 (Escrita)</b></span>".to_string(),
        2 => "<span foreground=\"#e67e22\"><b>🟠 Nível 2 (Admin)</b></span>".to_string(),
        3 => "<span foreground=\"#e74c3c\"><b>🔴 Nível 3 (Crítico)</b></span>".to_string(),
        4 => "<span foreground=\"#95a5a6\"><b>⛔ Nível 4 (Bloqueado)</b></span>".to_string(),
        other => format!("<b>Nível {other}</b>"),
    }
}

/// Resolves candidate audit log file paths (`~/.local/share/onyxsh/audit/audit.jsonl` and `~/.config/onyxsh/audit.jsonl`).
pub fn audit_log_candidate_paths() -> Vec<PathBuf> {
    vec![
        platform::data_dir().join("audit").join("audit.jsonl"),
        platform::config_dir().join("audit.jsonl"),
        platform::config_dir().join("audit").join("audit.jsonl"),
    ]
}

/// Loads all recorded audit entries from the candidate files.
pub fn load_audit_records() -> Vec<AuditEntry> {
    let mut records = Vec::new();
    for path in audit_log_candidate_paths() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(record) = serde_json::from_str::<AuditEntry>(trimmed) {
                        records.push(record);
                    }
                }
            }
        }
    }
    records
}

/// Scans backup folders for manifests associated with agent action plans.
pub fn list_file_backups() -> HashMap<String, BackupManifest> {
    let mut map = HashMap::new();
    let dirs_to_check = [
        platform::data_dir().join("backups"),
        platform::config_dir().join("backups"),
    ];
    for base_dir in dirs_to_check {
        if let Ok(entries) = fs::read_dir(base_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let mf_path = entry.path().join("manifest.json");
                    if mf_path.exists() {
                        if let Ok(data) = fs::read_to_string(&mf_path) {
                            if let Ok(manifest) = serde_json::from_str::<BackupManifest>(&data) {
                                if !manifest.plan_id.is_empty() {
                                    map.insert(manifest.plan_id.clone(), manifest);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    map
}

/// Atomically restores a file from its backup copy.
pub fn rollback_backup_manifest(manifest: &BackupManifest) -> std::io::Result<()> {
    let target = Path::new(&manifest.target_path);
    let backup = Path::new(&manifest.backup_path);
    if !backup.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Backup não encontrado em {}", backup.display()),
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_restore = target.with_extension("rollback.tmp");
    fs::copy(backup, &tmp_restore)?;
    fs::rename(&tmp_restore, target)?;
    Ok(())
}

/// Displays the Agent Audit Log dialog matching `registrodeauditoria.png` and Python `audit_log_dialog.py`.
pub fn show_audit_log_dialog(parent: &adw::ApplicationWindow, status_lbl: &gtk4::Label) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Registro de Auditoria do Agente")
        .default_width(850)
        .default_height(560)
        .build();

    let root_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Header bar with "Cancelar" on the left and Close on the right
    let header_bar = adw::HeaderBar::new();
    header_bar.set_show_end_title_buttons(true);
    header_bar.set_show_start_title_buttons(false);

    let title_lbl = gtk4::Label::builder()
        .label("Registro de Auditoria do Agente")
        .css_classes(["title", "heading"])
        .build();
    header_bar.set_title_widget(Some(&title_lbl));

    let btn_cancel = gtk4::Button::builder()
        .label("Cancelar")
        .css_classes(["flat"])
        .build();
    let d_close = dlg.clone();
    btn_cancel.connect_clicked(move |_| {
        d_close.close();
    });
    header_bar.pack_start(&btn_cancel);
    root_vbox.append(&header_bar);

    let content_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content_vbox.set_margin_top(16);
    content_vbox.set_margin_bottom(16);
    content_vbox.set_margin_start(16);
    content_vbox.set_margin_end(16);

    // Header description matching Python markup and screenshot
    let desc_label = gtk4::Label::new(None);
    desc_label.set_markup(
        "Trilha contínua de auditoria das decisões, comandos avaliados e ações executadas pelo <b>Modo Agente Seguro</b>.",
    );
    desc_label.set_halign(gtk4::Align::Start);
    desc_label.set_wrap(true);
    content_vbox.append(&desc_label);

    // Preferences group / list
    let list_group = adw::PreferencesGroup::new();
    list_group.set_title("Histórico de Ações");

    let records = load_audit_records();
    let backups = list_file_backups();

    if records.is_empty() {
        let row = adw::ActionRow::builder()
            .title("Nenhum registro de auditoria encontrado")
            .subtitle("As ações executadas pelo agente serão registradas aqui automaticamente.")
            .build();
        list_group.add(&row);
    } else {
        // Show newest first (reversed, up to 50 records)
        let slice: Vec<&AuditEntry> = records.iter().rev().take(50).collect();
        for rec in slice {
            let cmd_display = if !rec.argv.is_empty() {
                rec.argv.join(" ")
            } else if !rec.command_or_path.is_empty() {
                rec.command_or_path.clone()
            } else {
                rec.tool.clone()
            };

            let row = adw::ActionRow::builder()
                .title(&cmd_display)
                .subtitle(&format!(
                    "{} | Decisão: {} | Status: {}",
                    rec.timestamp, rec.user_decision, rec.result_status
                ))
                .build();

            // Check if record has associated backup for rollback
            if let Some(backup) = backups.get(&rec.plan_id) {
                let btn_rollback = gtk4::Button::builder()
                    .label("Desfazer")
                    .valign(gtk4::Align::Center)
                    .css_classes(["destructive-action"])
                    .build();
                let b_manifest = backup.clone();
                let st = status_lbl.clone();
                btn_rollback.connect_clicked(move |btn| {
                    if let Ok(()) = rollback_backup_manifest(&b_manifest) {
                        btn.set_sensitive(false);
                        btn.set_label("Restaurado ✓");
                        st.set_text(&format!(
                            "Arquivo restaurado pós-rollback: {}",
                            b_manifest.target_path
                        ));
                    }
                });
                row.add_suffix(&btn_rollback);
            }

            let badge_lbl = gtk4::Label::new(None);
            badge_lbl.set_markup(&risk_badge_markup(rec.risk));
            row.add_suffix(&badge_lbl);

            list_group.add(&row);
        }
    }

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_child(Some(&list_group));
    content_vbox.append(&scroll);

    root_vbox.append(&content_vbox);
    dlg.set_content(Some(&root_vbox));
    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_badge_markup() {
        assert!(risk_badge_markup(0).contains("Nível 0 (Leitura)"));
        assert!(risk_badge_markup(0).contains("#2ecc71"));
        assert!(risk_badge_markup(1).contains("Nível 1 (Escrita)"));
        assert!(risk_badge_markup(1).contains("#3498db"));
        assert!(risk_badge_markup(2).contains("Nível 2 (Admin)"));
        assert!(risk_badge_markup(3).contains("Nível 3 (Crítico)"));
        assert!(risk_badge_markup(4).contains("Nível 4 (Bloqueado)"));
        assert!(risk_badge_markup(9).contains("Nível 9"));
    }

    #[test]
    fn test_audit_entry_json_deserialization() {
        let json_line = r#"{"timestamp": "2026-11-09T02:32:51.779214+00:00", "plan_id": "opencode_error_resolution", "step_id": "error_0", "tool": "opencode.resolve_error", "argv": ["python3 manage.py migrate"], "risk": 1, "user_decision": "approved", "result_status": "ok"}"#;
        let entry: AuditEntry = serde_json::from_str(json_line).expect("parse entry");
        assert_eq!(entry.timestamp, "2026-11-09T02:32:51.779214+00:00");
        assert_eq!(entry.plan_id, "opencode_error_resolution");
        assert_eq!(entry.argv, vec!["python3 manage.py migrate"]);
        assert_eq!(entry.risk, 1);
        assert_eq!(entry.user_decision, "approved");
        assert_eq!(entry.result_status, "ok");
    }

    #[test]
    fn test_rollback_backup_manifest() {
        let tmp = std::env::temp_dir().join(format!("onyxsh_rollback_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let target_file = tmp.join("app.py");
        let backup_file = tmp.join("app.py.bak");

        fs::write(&target_file, "print('new modified code')").unwrap();
        fs::write(&backup_file, "print('original safe code')").unwrap();

        let manifest = BackupManifest {
            backup_id: "test_bak_01".to_string(),
            target_path: target_file.to_string_lossy().to_string(),
            backup_path: backup_file.to_string_lossy().to_string(),
            plan_id: "plan_test".to_string(),
            timestamp: "2026-11-09T00:00:00Z".to_string(),
            sha256_original: "".to_string(),
        };

        rollback_backup_manifest(&manifest).expect("rollback success");
        let restored_content = fs::read_to_string(&target_file).unwrap();
        assert_eq!(restored_content, "print('original safe code')");

        let _ = fs::remove_dir_all(&tmp);
    }
}

