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

/// Built-in color scheme identifiers in standard order (compatible with Python OnyxSH).
pub const BUILTIN_SCHEME_KEYS: &[&str] = &[
    "ayu_mirage",
    "catppuccin",
    "catppuccin_latte",
    "dark",
    "dracula",
    "everforest",
    "everforest_dark",
    "everforest_light",
    "github_dark",
    "github_light",
    "gruvbox_dark_hard",
    "gruvbox_light",
    "horizon_dark",
    "kanagawa_wave",
    "light",
    "material_ocean",
    "monokai",
    "moonfly",
    "night_owl",
    "nord",
    "onedark_pro",
    "oxocarbon_dark",
    "palenight",
    "pink_light",
    "rose_pine",
    "solarized_dark",
    "solarized_light",
    "system_default",
    "tokyonight",
    "vscode_dark_plus",
];

/// Deserializes `color_scheme` from either a string name (e.g. `"dracula"`) or legacy integer index (e.g. `8`).
pub fn deserialize_flexible_color_scheme<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SchemeVal {
        Str(String),
        Int(usize),
    }

    match SchemeVal::deserialize(deserializer)? {
        SchemeVal::Str(s) => Ok(s),
        SchemeVal::Int(idx) => {
            if let Some(key) = BUILTIN_SCHEME_KEYS.get(idx) {
                Ok((*key).to_string())
            } else {
                Ok("dracula".to_string())
            }
        }
    }
}

/// Deserializes a numeric u8 setting (transparency, headerbar transparency) from integer, float, or string.
pub fn deserialize_flexible_u8<'de, D>(deserializer: D) -> Result<u8, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrStr {
        Int(i64),
        Float(f64),
        Str(String),
    }

    match NumOrStr::deserialize(deserializer)? {
        NumOrStr::Int(i) => Ok(i.clamp(0, 100) as u8),
        NumOrStr::Float(f) => Ok(f.clamp(0.0, 100.0).round() as u8),
        NumOrStr::Str(s) => {
            if let Ok(i) = s.trim().parse::<i64>() {
                Ok(i.clamp(0, 100) as u8)
            } else if let Ok(f) = s.trim().parse::<f64>() {
                Ok(f.clamp(0.0, 100.0).round() as u8)
            } else {
                Ok(16)
            }
        }
    }
}

/// Default height for the bottom file manager panel in pixels (~48-50% screen split).
pub fn default_file_manager_height() -> u32 {
    500
}

pub fn default_fm_sort_by() -> String {
    "name".to_string()
}

pub fn default_true() -> bool {
    true
}

pub fn default_fm_view_mode() -> String {
    "list".to_string()
}

pub fn default_gtk_theme() -> String {
    "default".to_string()
}

pub fn default_ai_panel_height() -> u32 {
    420
}

pub fn default_ai_agent_allowed_roots() -> Vec<String> {
    let home = crate::platform::home_dir();
    vec![
        home.join("Documents").to_string_lossy().to_string(),
        home.join("Projects").to_string_lossy().to_string(),
        home.join("Downloads").to_string_lossy().to_string(),
        home.join("scripts").to_string_lossy().to_string(),
    ]
}

/// Deserializes a numeric u32 setting (e.g. `file_manager_height`) from integer, float, or string.
pub fn deserialize_flexible_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrStr {
        Int(i64),
        Float(f64),
        Str(String),
    }

    match NumOrStr::deserialize(deserializer)? {
        NumOrStr::Int(i) => Ok(i.clamp(120, 2000) as u32),
        NumOrStr::Float(f) => Ok(f.clamp(120.0, 2000.0).round() as u32),
        NumOrStr::Str(s) => {
            if let Ok(i) = s.trim().parse::<i64>() {
                Ok(i.clamp(120, 2000) as u32)
            } else if let Ok(f) = s.trim().parse::<f64>() {
                Ok(f.clamp(120.0, 2000.0).round() as u32)
            } else {
                Ok(280)
            }
        }
    }
}

/// Complete list of default keyboard shortcuts for OnyxSH (`DefaultSettings._get_base_defaults()["shortcuts"]`).
pub fn default_shortcuts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("new-local-tab", "<Control><Shift>t"),
        ("close-tab", "<Control><Shift>w"),
        ("next-tab", "<Alt>Page_Down"),
        ("previous-tab", "<Alt>Page_Up"),
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
        ("show-tunnel-manager", "<Control><Shift>u"),
        ("show-resource-monitor", "<Control><Shift>r"),
        ("toggle-tftp-server", "<Control><Shift>y"),
        ("new-window", "<Control><Shift>n"),
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
                ("new-local-tab", "Nova Aba"),
                ("close-tab", "Fechar Aba"),
                ("next-tab", "Próxima Aba"),
                ("previous-tab", "Aba Anterior"),
            ],
        ),
        (
            "Divisão",
            vec![
                ("split-horizontal", "Dividir Horizontalmente"),
                ("split-vertical", "Dividir Verticalmente"),
                ("close-pane", "Fechar Painel Ativo"),
                ("focus-pane-up", "Focar painel acima"),
                ("focus-pane-down", "Focar painel abaixo"),
                ("focus-pane-left", "Focar painel à esquerda"),
                ("focus-pane-right", "Focar painel à direita"),
            ],
        ),
        (
            "Interação do Terminal",
            vec![
                ("copy", "Copiar"),
                ("paste", "Colar"),
                ("select-all", "Selecionar Tudo"),
                ("clear-session", "Limpar Sessão"),
                ("toggle-search", "Pesquisar no Terminal"),
                ("global-search", "Busca Global Multi-Abas / Histórico"),
                ("toggle-broadcast", "Enviar Comando para Todas as Abas"),
                ("ai-assistant", "Perguntar ao Assistente de IA"),
                ("opencode-launcher", "Assistente de Código OpenCode"),
                ("toggle-selection-mode", "Modo Seleção de Texto"),
                ("show-command-history", "Histórico de Comandos"),
                ("jump-previous-prompt", "Pular para o Prompt Anterior"),
                ("jump-next-prompt", "Pular para o Próximo Prompt"),
            ],
        ),
        (
            "Zoom",
            vec![
                ("zoom-in", "Aproximar"),
                ("zoom-out", "Reduzir Zoom"),
                ("zoom-reset", "Redefinir Zoom"),
            ],
        ),
        (
            "Aplicativo e Janela",
            vec![
                ("toggle-sidebar", "Painel de Sessões"),
                ("toggle-file-manager", "Gerenciador de Arquivos"),
                ("show-command-manager", "Gerenciador de Comandos"),
                ("command-palette", "Paleta de Comandos"),
                ("show-tunnel-manager", "Gerenciador de Túneis SSH"),
                ("show-resource-monitor", "Monitor de Recursos do Sistema"),
                ("toggle-tftp-server", "Servidor TFTP"),
                ("new-window", "Nova Janela"),
                ("preferences", "Preferências"),
                ("quit", "Sair do Aplicativo"),
            ],
        ),
    ]
}

/// Converts a GTK accelerator string (e.g. `<Control><Shift>t`) into a human-readable badge (`Shift+Ctrl+T`).
pub fn accelerator_to_label(accel: &str) -> String {
    if accel.trim().is_empty() {
        return "Não definido".to_string();
    }
    let mut rest = accel.trim().to_string();
    let mut parts = Vec::new();

    for (tag, label) in [
        ("<Shift>", "Shift"),
        ("<Control>", "Ctrl"),
        ("<Primary>", "Ctrl"),
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
        "Up" | "KP_Up" => "Acima".to_string(),
        "Down" | "KP_Down" => "Abaixo".to_string(),
        "Left" | "KP_Left" => "Esquerda".to_string(),
        "Right" | "KP_Right" => "Direita".to_string(),
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

/// Core settings model compatible with `~/.config/onyxsh/settings.json` (`src/onyxsh/settings/config.py`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    // 1. Appearance (Aparência)
    pub font: String,
    pub font_scale: f64,
    pub line_spacing: f64,
    #[serde(deserialize_with = "deserialize_flexible_color_scheme")]
    pub color_scheme: String,
    #[serde(deserialize_with = "deserialize_flexible_u8")]
    pub transparency: u8,
    #[serde(deserialize_with = "deserialize_flexible_u8")]
    pub headerbar_transparency: u8,
    pub bold_is_bright: bool,
    pub auto_hide_sidebar: bool,
    pub text_blink_mode: u32,
    pub tab_alignment: String,
    pub icon_theme_strategy: String,
    pub hide_headerbar_buttons_when_maximized: String,
    #[serde(default = "default_gtk_theme")]
    pub gtk_theme: String,
    #[serde(default = "default_file_manager_height", deserialize_with = "deserialize_flexible_u32")]
    pub file_manager_height: u32,
    #[serde(default = "default_fm_sort_by")]
    pub file_manager_sort_by: String,
    #[serde(default = "default_true")]
    pub file_manager_sort_ascending: bool,
    #[serde(default = "default_fm_view_mode")]
    pub file_manager_view_mode: String,

    // 2. Terminal
    pub new_instance_behavior: String,
    pub cursor_shape: u32,
    pub cursor_blink: u32,
    pub scrollback_lines: u32,
    pub mouse_scroll_sensitivity: f64,
    pub touchpad_scroll_sensitivity: f64,
    pub scroll_on_insert: bool,
    pub use_login_shell: bool,
    pub bell_sound: bool,
    pub notify_long_commands: bool,
    pub notify_long_commands_threshold: u32,
    pub notify_long_commands_condition: String,
    pub notify_long_commands_sound: bool,
    pub ai_proactive_error_suggestions: bool,
    pub ai_error_suggestion_mode: String,
    pub ai_error_auto_execute_quick_fix: bool,
    pub autocomplete_enabled: bool,
    pub autocomplete_specs_enabled: bool,
    pub autocomplete_history_enabled: bool,
    pub autocomplete_snippets_enabled: bool,
    pub show_selection_toolbar: bool,
    pub enable_osc52_clipboard: bool,

    // 3. Profiles & Data (Perfis e Dados)
    pub session_restore_policy: String,
    pub session_restore_ssh_auto_reconnect: bool,
    pub session_restore_ui_panels: bool,
    pub use_system_tmp_for_edit: bool,
    pub clear_remote_edit_files_on_exit: bool,
    pub ssh_health_check_enabled: bool,
    pub ssh_health_check_interval: u32,
    pub ssh_auto_reconnect_enabled: bool,
    pub ssh_auto_reconnect_attempts: u32,
    pub ssh_keepalive_interval: u32,
    pub ssh_control_persist_duration: u32,

    // 4. Advanced (Avançado)
    pub bidi_enabled: bool,
    pub enable_shaping: bool,
    pub sixel_enabled: bool,
    pub backspace_binding: u32,
    pub delete_binding: u32,
    pub cjk_ambiguous_width: u32,
    pub word_char_exceptions: String,
    pub log_to_file: bool,
    pub console_log_level: String,

    // 5. Search & AI
    pub search_case_sensitive: bool,
    pub search_use_regex: bool,
    pub search_whole_word: bool,
    pub ai_enabled: bool,
    pub ai_smart_routing: bool,
    pub ai_offline_mode: bool,
    pub ai_provider: String,
    pub ai_model: String,
    pub ai_local_base_url: String,
    pub ai_preload_local_model: bool,
    pub ai_unload_on_exit: bool,
    pub ai_api_key: String,
    pub ai_routing_profile: String,
    pub ai_fast_provider: String,
    pub ai_fast_model: String,
    pub ai_advanced_provider: String,
    pub ai_advanced_model: String,
    pub ai_api_key_gemini: String,
    pub ai_api_key_groq: String,
    pub ai_api_key_openrouter: String,
    pub ai_agent_max_risk_level: u8,
    pub ai_agent_auto_run_level0: bool,
    pub ai_agent_post_verification: bool,
    pub ai_agent_auto_verify: bool,
    pub ai_agent_include_system_context: bool,
    pub ai_agent_include_pwd_context: bool,
    pub ai_context_size: u32,
    #[serde(default = "default_ai_panel_height", deserialize_with = "deserialize_flexible_u32")]
    pub ai_panel_height: u32,
    #[serde(default = "default_ai_agent_allowed_roots")]
    pub ai_agent_allowed_roots: Vec<String>,
    pub shortcuts: HashMap<String, String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut shortcuts = HashMap::new();
        for (k, v) in default_shortcuts() {
            shortcuts.insert(k.to_string(), v.to_string());
        }
        Self {
            font: "Ubuntu Mono Regular 12".to_string(),
            font_scale: 1.0,
            line_spacing: 1.0,
            color_scheme: "onyx-dark".to_string(),
            transparency: 16,
            headerbar_transparency: 12,
            bold_is_bright: false,
            auto_hide_sidebar: true,
            text_blink_mode: 0,
            tab_alignment: "center".to_string(),
            icon_theme_strategy: "system".to_string(),
            hide_headerbar_buttons_when_maximized: "auto".to_string(),
            gtk_theme: default_gtk_theme(),
            file_manager_height: 500,
            file_manager_sort_by: "name".to_string(),
            file_manager_sort_ascending: true,
            file_manager_view_mode: "list".to_string(),

            new_instance_behavior: "new_tab".to_string(),
            cursor_shape: 0,
            cursor_blink: 0,
            scrollback_lines: 5_000,
            mouse_scroll_sensitivity: 30.0,
            touchpad_scroll_sensitivity: 30.0,
            scroll_on_insert: true,
            use_login_shell: false,
            bell_sound: false,
            notify_long_commands: true,
            notify_long_commands_threshold: 10,
            notify_long_commands_condition: "unfocused".to_string(),
            notify_long_commands_sound: true,
            ai_proactive_error_suggestions: true,
            ai_error_suggestion_mode: "toast_and_badge".to_string(),
            ai_error_auto_execute_quick_fix: false,
            autocomplete_enabled: true,
            autocomplete_specs_enabled: true,
            autocomplete_history_enabled: true,
            autocomplete_snippets_enabled: true,
            show_selection_toolbar: true,
            enable_osc52_clipboard: true,

            session_restore_policy: "never".to_string(),
            session_restore_ssh_auto_reconnect: true,
            session_restore_ui_panels: true,
            use_system_tmp_for_edit: false,
            clear_remote_edit_files_on_exit: true,
            ssh_health_check_enabled: true,
            ssh_health_check_interval: 10,
            ssh_auto_reconnect_enabled: true,
            ssh_auto_reconnect_attempts: 5,
            ssh_keepalive_interval: 15,
            ssh_control_persist_duration: 600,

            bidi_enabled: false,
            enable_shaping: false,
            sixel_enabled: true,
            backspace_binding: 0,
            delete_binding: 0,
            cjk_ambiguous_width: 1,
            word_char_exceptions: "-_.:/~".to_string(),
            log_to_file: false,
            console_log_level: "ERROR".to_string(),

            search_case_sensitive: false,
            search_use_regex: false,
            search_whole_word: false,
            ai_enabled: true,
            ai_smart_routing: true,
            ai_offline_mode: false,
            ai_provider: "groq".to_string(),
            ai_model: "llama-3.1-8b-instant".to_string(),
            ai_local_base_url: "http://localhost:11434/v1".to_string(),
            ai_preload_local_model: true,
            ai_unload_on_exit: true,
            ai_api_key: String::new(),
            ai_routing_profile: "auto".to_string(),
            ai_fast_provider: "groq".to_string(),
            ai_fast_model: "llama-3.1-8b-instant".to_string(),
            ai_advanced_provider: "gemini".to_string(),
            ai_advanced_model: "gemini-2.5-flash".to_string(),
            ai_api_key_gemini: String::new(),
            ai_api_key_groq: String::new(),
            ai_api_key_openrouter: String::new(),
            ai_agent_max_risk_level: 3,
            ai_agent_auto_run_level0: false,
            ai_agent_post_verification: true,
            ai_agent_auto_verify: false,
            ai_agent_include_system_context: true,
            ai_agent_include_pwd_context: true,
            ai_context_size: 8192,
            ai_panel_height: default_ai_panel_height(),
            ai_agent_allowed_roots: default_ai_agent_allowed_roots(),
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
        assert_eq!(settings.shortcuts.len(), default_shortcuts().len());
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
        assert_eq!(accelerator_to_label("<Control><Shift>t"), "Shift+Ctrl+T");
        assert_eq!(accelerator_to_label("<Alt>Page_Down"), "Alt+Page Down");
    }

    #[test]
    fn test_flexible_deserialization_of_color_scheme_and_transparency() {
        // Test with Python OnyxSH format: integer color_scheme and float transparency
        let raw_json = r#"{
            "color_scheme": 8,
            "transparency": 0.0,
            "headerbar_transparency": 12.0
        }"#;

        let settings: AppSettings = serde_json::from_str(raw_json).expect("Failed to deserialize flexible settings");
        assert_eq!(settings.color_scheme, "github_dark");
        assert_eq!(settings.transparency, 0);
        assert_eq!(settings.headerbar_transparency, 12);

        // Test with string color_scheme and int 0 transparency
        let raw_json2 = r#"{
            "color_scheme": "dracula",
            "transparency": 0
        }"#;
        let settings2: AppSettings = serde_json::from_str(raw_json2).expect("Failed to deserialize string scheme");
        assert_eq!(settings2.color_scheme, "dracula");
        assert_eq!(settings2.transparency, 0);

        // Test with roundtrip save and load to tempfile
        let tmp_dir = std::env::temp_dir();
        let test_file = tmp_dir.join(format!("test_onyxsh_settings_{}.json", std::process::id()));
        settings2.save_to_path(&test_file).expect("Failed to save settings");
        let loaded = AppSettings::load_from_path(&test_file).expect("Failed to reload settings");
        assert_eq!(loaded.transparency, 0);
        assert_eq!(loaded.color_scheme, "dracula");
        let _ = std::fs::remove_file(test_file);
    }

    #[test]
    fn test_file_manager_settings_persistence() {
        let raw = r#"{
            "file_manager_height": 420.0,
            "file_manager_sort_by": "group",
            "file_manager_sort_ascending": false,
            "file_manager_view_mode": "grid",
            "gtk_theme": "dark",
            "ai_panel_height": 450
        }"#;
        let s: AppSettings = serde_json::from_str(raw).expect("parse fm settings");
        assert_eq!(s.file_manager_height, 420);
        assert_eq!(s.file_manager_sort_by, "group");
        assert!(!s.file_manager_sort_ascending);
        assert_eq!(s.file_manager_view_mode, "grid");
        assert_eq!(s.gtk_theme, "dark");
        assert_eq!(s.ai_panel_height, 450);

        let default_st = AppSettings::default();
        assert_eq!(default_st.file_manager_height, 500);
        assert_eq!(default_st.file_manager_sort_by, "name");
        assert!(default_st.file_manager_sort_ascending);
        assert_eq!(default_st.file_manager_view_mode, "list");
        assert_eq!(default_st.gtk_theme, "default");
        assert_eq!(default_st.ai_panel_height, 420);
        assert_eq!(default_st.ai_agent_allowed_roots.len(), 4);
    }
}
