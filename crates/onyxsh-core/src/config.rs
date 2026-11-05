//! Reactive configuration management and shortcut sanitization.
//! Port of `src/onyxsh/settings/config.py`, `src/onyxsh/settings/manager.py`,
//! and `src/onyxsh/ui/dialogs/shortcuts_dialog.py`.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Obsolete shortcut action keys that must be purged on load (mirrors `OBSOLETE_SHORTCUT_ACTIONS`).
pub const OBSOLETE_SHORTCUT_ACTIONS: &[&str] = &[
    "export-terminal-buffer",
    "keyboard-copy-mode",
    "new-tab",
    "find",
];

/// Complete list of 34 default keyboard shortcuts for OnyxSH (`DefaultSettings._get_base_defaults()["shortcuts"]`).
pub fn default_shortcuts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("new-local-tab", "<Control><Shift>t"),
        ("close-tab", "<Control><Shift>w"),
        ("next-tab", "<Alt>Page_Down"),
        ("previous-tab", "<Alt>Page_Up"),
        ("new-window", "<Control><Shift>n"),
        ("split-horizontal", "<Control><Shift>parenleft"),
        ("split-vertical", "<Control><Shift>parenright"),
        ("close-pane", "<Control><Shift>k"),
        ("focus-pane-up", "<Control><Shift>Up"),
        ("focus-pane-down", "<Control><Shift>Down"),
        ("focus-pane-left", "<Control><Shift>Left"),
        ("focus-pane-right", "<Control><Shift>Right"),
        ("copy", "<Control><Shift>c"),
        ("paste", "<Control><Shift>v"),
        ("select-all", "<Control><Shift>a"),
        ("clear-session", "<Control><Shift>l"),
        ("toggle-search", "<Control><Shift>f"),
        ("global-search", "<Control><Shift>g"),
        ("toggle-broadcast", "<Control><Shift>b"),
        ("ai-assistant", "<Control><Shift>i"),
        ("opencode-launcher", "<Control><Shift>o"),
        ("toggle-selection-mode", "<Control><Shift>s"),
        ("show-command-history", "<Control>h"),
        ("jump-previous-prompt", "<Alt>Up"),
        ("jump-next-prompt", "<Alt>Down"),
        ("zoom-in", "<Control>plus"),
        ("zoom-out", "<Control>minus"),
        ("zoom-reset", "<Control>0"),
        ("toggle-sidebar", "<Control><Shift>h"),
        ("toggle-file-manager", "<Control><Shift>e"),
        ("show-command-manager", "<Control><Shift>m"),
        ("command-palette", "<Control><Shift>p"),
        ("preferences", "F2"),
        ("quit", "<Control><Shift>q"),
    ]
}

/// Group metadata for the Keyboard Shortcuts dialog (`SHORTCUT_DATA` in `shortcuts_dialog.py`).
pub fn shortcut_groups() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    vec![
        (
            "Gerenciamento de Abas e Painéis",
            vec![
                ("new-local-tab", "Nova Aba Local"),
                ("close-tab", "Fechar Aba Atual"),
                ("next-tab", "Próxima Aba"),
                ("previous-tab", "Aba Anterior"),
            ],
        ),
        (
            "Divisão de Tela (Splitting)",
            vec![
                ("split-horizontal", "Dividir Horizontalmente"),
                ("split-vertical", "Dividir Verticalmente"),
                ("close-pane", "Fechar Painel Ativo"),
                ("focus-pane-up", "Focar Painel Acima"),
                ("focus-pane-down", "Focar Painel Abaixo"),
                ("focus-pane-left", "Focar Painel à Esquerda"),
                ("focus-pane-right", "Focar Painel à Direita"),
            ],
        ),
        (
            "Interação com o Terminal & IA",
            vec![
                ("copy", "Copiar Seleção"),
                ("paste", "Colar"),
                ("select-all", "Selecionar Tudo"),
                ("clear-session", "Limpar Sessão do Terminal"),
                ("toggle-search", "Buscar no Terminal Atual"),
                ("global-search", "Busca Global Multi-Abas / Sessões / Histórico"),
                ("toggle-broadcast", "Enviar Comando para Todas as Abas (Broadcast)"),
                ("ai-assistant", "Abrir / Fechar Assistente de IA"),
                ("opencode-launcher", "Abrir Assistente de Código OpenCode"),
                ("toggle-selection-mode", "Alternar Modo Seleção de Texto"),
                ("show-command-history", "Histórico Enriquecido de Comandos"),
                ("jump-previous-prompt", "Pular para o Prompt Anterior (OSC 133)"),
                ("jump-next-prompt", "Pular para o Próximo Prompt (OSC 133)"),
            ],
        ),
        (
            "Zoom e Fonte",
            vec![
                ("zoom-in", "Aumentar Zoom (Fonte)"),
                ("zoom-out", "Diminuir Zoom (Fonte)"),
                ("zoom-reset", "Resetar Zoom"),
            ],
        ),
        (
            "Aplicativo e Janela",
            vec![
                ("toggle-sidebar", "Painel Lateral de Sessões SSH"),
                ("toggle-file-manager", "Gerenciador de Arquivos Dual-Pane"),
                ("show-command-manager", "Gerenciador de Comandos e Snippets"),
                ("command-palette", "Paleta de Comandos (Spotlight)"),
                ("new-window", "Nova Janela"),
                ("preferences", "Preferências"),
                ("quit", "Sair do Aplicativo"),
            ],
        ),
    ]
}

/// Converts a GTK accelerator string (e.g. `<Control><Shift>t`) into a human-readable badge (`Ctrl+Shift+T`).
pub fn accelerator_to_label(accel: &str) -> String {
    if accel.trim().is_empty() {
        return "Não definido".to_string();
    }
    let mut rest = accel.trim().to_string();
    let mut parts = Vec::new();

    for (tag, label) in [
        ("<Control>", "Ctrl"),
        ("<Primary>", "Ctrl"),
        ("<Shift>", "Shift"),
        ("<Alt>", "Alt"),
        ("<Super>", "Super"),
    ] {
        if rest.contains(tag) {
            parts.push(label.to_string());
            rest = rest.replace(tag, "");
        }
    }

    let key_display = match rest.as_str() {
        "plus" | "equal" => "+".to_string(),
        "minus" => "-".to_string(),
        "parenleft" => "(".to_string(),
        "parenright" => ")".to_string(),
        "Page_Down" => "Page Down".to_string(),
        "Page_Up" => "Page Up".to_string(),
        "Up" | "KP_Up" => "↑".to_string(),
        "Down" | "KP_Down" => "↓".to_string(),
        "Left" | "KP_Left" => "←".to_string(),
        "Right" | "KP_Right" => "→".to_string(),
        "Insert" => "Ins".to_string(),
        "Delete" => "Del".to_string(),
        "Return" | "KP_Enter" => "Enter".to_string(),
        other if other.len() == 1 => other.to_uppercase(),
        other => other.to_string(),
    };

    if !key_display.is_empty() {
        parts.push(key_display);
    }
    parts.join("+")
}

/// Core settings model compatible with `~/.config/onyxsh/settings.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub font: String,
    pub font_scale: f64,
    pub color_scheme: String,
    pub scrollback_lines: u32,
    pub transparency: u8,
    pub enable_osc52_clipboard: bool,
    pub search_case_sensitive: bool,
    pub search_use_regex: bool,
    pub search_whole_word: bool,
    pub ai_enabled: bool,
    pub ai_smart_routing: bool,
    pub ai_offline_mode: bool,
    pub ai_provider: String,
    pub ai_model: String,
    pub shortcuts: HashMap<String, String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut shortcuts = HashMap::new();
        for (k, v) in default_shortcuts() {
            shortcuts.insert(k.to_string(), v.to_string());
        }
        Self {
            font: "Monospace 11".to_string(),
            font_scale: 1.0,
            color_scheme: "onyx-dark".to_string(),
            scrollback_lines: 10_000,
            transparency: 0,
            enable_osc52_clipboard: true,
            search_case_sensitive: false,
            search_use_regex: false,
            search_whole_word: false,
            ai_enabled: true,
            ai_smart_routing: true,
            ai_offline_mode: false,
            ai_provider: "gemini".to_string(),
            ai_model: "gemini-2.5-flash".to_string(),
            shortcuts,
        }
    }
}

impl AppSettings {
    /// Repairs obsolete or conflicting shortcuts (Port of `SettingsManager._repair_shortcuts`).
    pub fn repair_shortcuts(&mut self) -> bool {
        let mut modified = false;

        // 1. Remove obsolete actions
        let keys: Vec<String> = self.shortcuts.keys().cloned().collect();
        for key in keys {
            if OBSOLETE_SHORTCUT_ACTIONS.contains(&key.as_str()) {
                self.shortcuts.remove(&key);
                modified = true;
            }
        }

        // 2. Ensure default actions exist and resolve duplicates deterministically
        let mut used_accels = HashSet::new();
        for (action, default_accel) in default_shortcuts() {
            let current = self
                .shortcuts
                .entry(action.to_string())
                .or_insert_with(|| {
                    modified = true;
                    default_accel.to_string()
                })
                .clone();

            if !current.is_empty() {
                if used_accels.contains(&current) {
                    if !used_accels.contains(default_accel) {
                        self.shortcuts
                            .insert(action.to_string(), default_accel.to_string());
                        used_accels.insert(default_accel.to_string());
                    } else {
                        self.shortcuts.insert(action.to_string(), String::new());
                    }
                    modified = true;
                } else {
                    used_accels.insert(current);
                }
            }
        }

        modified
    }

    /// Resets all shortcuts to their default bindings.
    pub fn reset_shortcuts_to_default(&mut self) {
        self.shortcuts.clear();
        for (k, v) in default_shortcuts() {
            self.shortcuts.insert(k.to_string(), v.to_string());
        }
    }

    /// Loads settings from a JSON file, repairing shortcuts if needed.
    pub fn load_from_path(path: &Path) -> std::io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path)?;
        let mut settings: Self = serde_json::from_str(&raw).unwrap_or_default();
        settings.repair_shortcuts();
        Ok(settings)
    }

    /// Saves settings atomically via a temporary file + fsync + rename.
    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("json.tmp");
        let serialized = serde_json::to_string_pretty(self)?;
        {
            let mut file = fs::File::create(&tmp_path)?;
            file.write_all(serialized.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shortcut_repair_removes_obsolete_and_duplicates() {
        let mut settings = AppSettings::default();
        assert_eq!(settings.shortcuts.len(), 34);
        settings.shortcuts.insert(
            "export-terminal-buffer".to_string(),
            "<Control><Shift>x".to_string(),
        );
        // Create a duplicate shortcut between copy and new-local-tab
        settings
            .shortcuts
            .insert("copy".to_string(), "<Control><Shift>t".to_string());

        let changed = settings.repair_shortcuts();
        assert!(changed);
        assert!(!settings.shortcuts.contains_key("export-terminal-buffer"));
        assert_eq!(
            settings.shortcuts.get("new-local-tab").map(String::as_str),
            Some("<Control><Shift>t")
        );
        assert_eq!(
            settings.shortcuts.get("copy").map(String::as_str),
            Some("<Control><Shift>c")
        );
        assert_eq!(accelerator_to_label("<Control><Shift>t"), "Ctrl+Shift+T");
        assert_eq!(accelerator_to_label("<Alt>Page_Down"), "Alt+Page Down");
    }
}
