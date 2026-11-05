//! Reactive configuration management and shortcut sanitization.
//! Port of `src/onyxsh/settings/config.py` and `src/onyxsh/settings/manager.py`.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;

/// Obsolete shortcut action keys that must be purged on load (mirrors `OBSOLETE_SHORTCUT_ACTIONS`).
pub const OBSOLETE_SHORTCUT_ACTIONS: &[&str] = &[
    "export-terminal-buffer",
    "keyboard-copy-mode",
];

/// Default keyboard shortcuts for OnyxSH.
pub fn default_shortcuts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("new-tab", "<Control><Shift>t"),
        ("close-tab", "<Control><Shift>w"),
        ("copy", "<Control><Shift>c"),
        ("paste", "<Control><Shift>v"),
        ("find", "<Control><Shift>f"),
        ("toggle-selection-mode", "<Control><Shift>x"),
        ("command-palette", "<Control><Shift>p"),
        ("ai-assistant", "<Control><Shift>i"),
        ("split-horizontal", "<Control><Shift>h"),
        ("split-vertical", "<Control><Shift>d"),
        ("zoom-in", "<Control>plus"),
        ("zoom-out", "<Control>minus"),
        ("zoom-reset", "<Control>0"),
        ("quit", "<Control><Shift>q"),
    ]
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
        let defaults: HashMap<&str, &str> = default_shortcuts().into_iter().collect();

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
                    // Conflict detected: restore default if free, otherwise unassign
                    if !used_accels.contains(default_accel) {
                        self.shortcuts.insert(action.to_string(), default_accel.to_string());
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

        let _ = defaults;
        modified
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
        settings.shortcuts.insert(
            "export-terminal-buffer".to_string(),
            "<Control><Shift>x".to_string(),
        );
        // Create a duplicate shortcut between copy and new-tab
        settings
            .shortcuts
            .insert("copy".to_string(), "<Control><Shift>t".to_string());

        let changed = settings.repair_shortcuts();
        assert!(changed);
        assert!(!settings.shortcuts.contains_key("export-terminal-buffer"));
        assert_eq!(
            settings.shortcuts.get("new-tab").map(String::as_str),
            Some("<Control><Shift>t")
        );
        assert_eq!(
            settings.shortcuts.get("copy").map(String::as_str),
            Some("<Control><Shift>c")
        );
    }
}
