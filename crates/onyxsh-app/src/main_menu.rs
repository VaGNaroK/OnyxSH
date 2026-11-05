//! Phase 10: Main Hamburger Menu Popover, Theme Selector (4 modes), and Font Sizer (`- / 12 pt / +`).
//! Ports `src/onyxsh/ui/menus.py`.

use libadwaita as adw;
use serde::{Deserialize, Serialize};

/// The 4 theme modes available in `ThemeSelectorWidget` (`src/onyxsh/ui/menus.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GtkThemeMode {
    System,
    Light,
    Dark,
    TerminalMatch,
}

impl GtkThemeMode {
    pub fn from_setting(val: &str) -> Self {
        match val.trim().to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            "terminal" => Self::TerminalMatch,
            _ => Self::System,
        }
    }

    pub fn as_setting(&self) -> &'static str {
        match self {
            Self::System => "default",
            Self::Light => "light",
            Self::Dark => "dark",
            Self::TerminalMatch => "terminal",
        }
    }
}

/// Parses a Pango font string (e.g. `"Monospace 12"` or `"JetBrains Mono 14"`) into `(family, size_pt)`.
pub fn parse_font_string(font_string: &str) -> (String, i32) {
    let trimmed = font_string.trim();
    if let Some((family, size_str)) = trimmed.rsplit_once(' ') {
        if let Ok(size) = size_str.trim().parse::<i32>() {
            let fam = family.trim();
            if !fam.is_empty() {
                return (fam.to_string(), size.clamp(6, 72));
            }
        }
    }
    ("Monospace".to_string(), 12)
}

/// Adjusts the font size by `delta` points (clamped to `6..=72`) and returns `(new_font_string, new_size)`.
pub fn change_font_size(current_font: &str, delta: i32) -> (String, i32) {
    let (family, size) = parse_font_string(current_font);
    let new_size = (size + delta).clamp(6, 72);
    (format!("{family} {new_size}"), new_size)
}

/// Applies the selected [`GtkThemeMode`] to `adw::StyleManager`.
pub fn apply_gtk_theme_mode(mode: GtkThemeMode, terminal_bg_is_light: bool) {
    let style_manager = adw::StyleManager::default();
    match mode {
        GtkThemeMode::System => style_manager.set_color_scheme(adw::ColorScheme::Default),
        GtkThemeMode::Light => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
        GtkThemeMode::Dark => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
        GtkThemeMode::TerminalMatch => {
            if terminal_bg_is_light {
                style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
            } else {
                style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
            }
        }
    }
}

/// Generates CSS styling for the 4 circular theme selector buttons and menu layout.
pub fn menu_custom_css() -> &'static str {
    "
    .themeselector {
        margin-top: 8px;
        margin-bottom: 6px;
    }
    .theme-selector {
        min-height: 28px;
        min-width: 28px;
        border-radius: 999px;
        padding: 2px;
    }
    .font-sizer {
        margin-top: 4px;
        margin-bottom: 6px;
    }
    .search-bar-box {
        padding: 4px 8px;
    }
    "
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_and_change_font_size() {
        assert_eq!(
            parse_font_string("JetBrains Mono 14"),
            ("JetBrains Mono".to_string(), 14)
        );
        assert_eq!(parse_font_string("invalid"), ("Monospace".to_string(), 12));

        let (inc_str, inc_sz) = change_font_size("Monospace 12", 2);
        assert_eq!(inc_str, "Monospace 14");
        assert_eq!(inc_sz, 14);

        let (_, clamped_min) = change_font_size("Monospace 7", -10);
        assert_eq!(clamped_min, 6);

        let (_, clamped_max) = change_font_size("Monospace 70", 15);
        assert_eq!(clamped_max, 72);
    }

    #[test]
    fn test_gtk_theme_mode_roundtrip() {
        assert_eq!(GtkThemeMode::from_setting("light"), GtkThemeMode::Light);
        assert_eq!(GtkThemeMode::from_setting("dark"), GtkThemeMode::Dark);
        assert_eq!(
            GtkThemeMode::from_setting("terminal"),
            GtkThemeMode::TerminalMatch
        );
        assert_eq!(GtkThemeMode::from_setting("default"), GtkThemeMode::System);
        assert_eq!(GtkThemeMode::TerminalMatch.as_setting(), "terminal");
    }
}
