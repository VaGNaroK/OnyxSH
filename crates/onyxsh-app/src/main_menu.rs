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

/// Generates CSS styling for the 4 circular theme selector buttons, menu layout, and file manager badges.
pub fn menu_custom_css() -> &'static str {
    "
    .themeselector {
        margin: 8px 10px;
    }
    .themeselector checkbutton {
        padding: 0;
        min-height: 44px;
        min-width: 44px;
        background-clip: content-box;
        border-radius: 9999px;
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.22);
        transition: all 150ms ease-in-out;
    }
    .themeselector checkbutton:checked {
        box-shadow: inset 0 0 0 2px #3584e4;
    }
    .themeselector checkbutton.follow {
        background-image: linear-gradient(to bottom right, #ffffff 49.99%, #202020 50.01%);
    }
    .themeselector checkbutton.light {
        background-color: #ffffff;
    }
    .themeselector checkbutton.dark {
        background-color: #202020;
    }
    .themeselector checkbutton.terminal {
        background: linear-gradient(45deg, #000000 50%, #ffffff 50%);
    }
    .themeselector checkbutton radio,
    .themeselector checkbutton check {
        -gtk-icon-source: none;
        border: none;
        background: none;
        box-shadow: none;
        min-width: 14px;
        min-height: 14px;
        transform: translate(25px, 13px);
        padding: 2px;
        border-radius: 9999px;
    }
    .themeselector checkbutton radio:checked,
    .themeselector checkbutton check:checked {
        -gtk-icon-source: -gtk-icontheme(\"object-select-symbolic\");
        background-color: #3584e4;
        color: #ffffff;
        border-radius: 9999px;
    }
    .font-sizer {
        margin-top: 4px;
        margin-bottom: 6px;
    }
    .search-bar-box {
        padding: 4px 8px;
    }
    .badge-pill {
        border-radius: 6px;
        padding: 1px 5px;
        font-size: 9px;
        font-weight: 700;
        letter-spacing: 0.3px;
        margin-left: 4px;
    }
    .badge-exec {
        background-color: rgba(46, 194, 126, 0.18);
        color: #2ec4b6;
        border: 1px solid rgba(46, 194, 126, 0.35);
    }
    .badge-root {
        background-color: rgba(246, 211, 45, 0.18);
        color: #f6d32d;
        border: 1px solid rgba(246, 211, 45, 0.35);
    }
    .badge-py {
        background-color: rgba(53, 132, 228, 0.2);
        color: #3584e4;
        border: 1px solid rgba(53, 132, 228, 0.35);
    }
    .badge-sh {
        background-color: rgba(51, 209, 122, 0.2);
        color: #33d17a;
        border: 1px solid rgba(51, 209, 122, 0.35);
    }
    .badge-rs {
        background-color: rgba(230, 97, 0, 0.2);
        color: #e66100;
        border: 1px solid rgba(230, 97, 0, 0.35);
    }
    .badge-log {
        background-color: rgba(255, 120, 0, 0.2);
        color: #ff7800;
        border: 1px solid rgba(255, 120, 0, 0.35);
    }
    .badge-docker {
        background-color: rgba(28, 113, 216, 0.2);
        color: #1c71d8;
        border: 1px solid rgba(28, 113, 216, 0.35);
    }
    .badge-json {
        background-color: rgba(153, 68, 228, 0.2);
        color: #9944e4;
        border: 1px solid rgba(153, 68, 228, 0.35);
    }
    .badge-yaml {
        background-color: rgba(225, 78, 202, 0.2);
        color: #e14eca;
        border: 1px solid rgba(225, 78, 202, 0.35);
    }
    .file-detailed-header {
        padding: 2px 6px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.12);
        background-color: rgba(255, 255, 255, 0.03);
    }
    .file-detailed-header button,
    .file-detailed-header button:focus,
    .file-detailed-header button:active {
        font-weight: 600;
        color: rgba(255, 255, 255, 0.7);
        padding: 3px 6px;
        border-radius: 4px;
        background: transparent;
        border: none;
        box-shadow: none;
    }
    .file-detailed-header button:hover {
        background-color: rgba(255, 255, 255, 0.12);
        color: #ffffff;
    }
    .file-detailed-header-label {
        font-weight: 600;
        font-size: 0.9em;
    }
    .file-detailed-row {
        padding: 3px 6px;
        border-radius: 6px;
        margin: 1px 2px;
        transition: background-color 100ms ease;
    }
    .file-detailed-row:hover {
        background-color: rgba(255, 255, 255, 0.06);
    }
    .file-detailed-col-size {
        font-family: monospace;
        color: rgba(255, 255, 255, 0.85);
        min-width: 130px;
    }
    .file-detailed-col-date {
        font-family: monospace;
        color: rgba(255, 255, 255, 0.85);
        min-width: 150px;
    }
    .file-detailed-col-perms {
        font-family: monospace;
        color: rgba(255, 255, 255, 0.85);
        min-width: 110px;
    }
    .file-detailed-col-owner {
        font-family: monospace;
        color: rgba(255, 255, 255, 0.85);
        min-width: 160px;
    }
    .file-manager-grid-view {
        background: transparent;
        padding: 8px 10px;
    }
    .file-manager-grid-view > child,
    .file-manager-grid-view listitem {
        padding: 3px;
        margin: 3px;
        border-radius: 10px;
        background: transparent;
        transition: background-color 150ms ease-out;
    }
    .file-manager-grid-view > child:hover,
    .file-manager-grid-view listitem:hover {
        background-color: rgba(255, 255, 255, 0.10);
    }
    .file-manager-grid-view > child:selected,
    .file-manager-grid-view listitem:selected {
        background-color: rgba(53, 132, 228, 0.28);
    }
    .file-manager-grid-view > child:selected:hover,
    .file-manager-grid-view listitem:selected:hover {
        background-color: rgba(53, 132, 228, 0.36);
    }
    .file-grid-card {
        padding: 6px 4px;
        border-radius: 10px;
        min-width: 90px;
    }
    .file-grid-card-icon {
        margin-bottom: 4px;
    }
    .file-grid-card-label {
        font-size: 0.85rem;
    }
    .file-grid-card-size {
        font-size: 0.75rem;
    }
    .file-manager-tree-view {
        background: transparent;
    }
    .file-tree-row {
        padding: 3px 6px;
        border-radius: 6px;
        margin: 1px 2px;
        transition: background-color 100ms ease;
    }
    .file-tree-row:hover {
        background-color: rgba(255, 255, 255, 0.06);
    }
    .file-tree-expander-btn {
        min-width: 20px;
        min-height: 20px;
        padding: 0;
        margin-right: 4px;
        border-radius: 4px;
        color: rgba(255, 255, 255, 0.7);
    }
    .file-tree-expander-btn:hover {
        background-color: rgba(255, 255, 255, 0.15);
        color: #ffffff;
    }
    .breadcrumb-trail button {
        padding: 2px 6px;
        font-weight: 600;
    }
    .quick-jump-popover {
        min-width: 320px;
        padding: 6px;
    }
    .quick-jump-header {
        font-size: 10px;
        font-weight: bold;
        letter-spacing: 0.5px;
        opacity: 0.6;
        margin-top: 6px;
        margin-bottom: 4px;
        margin-left: 8px;
        text-transform: uppercase;
    }
    .quick-jump-item-btn {
        border-radius: 6px;
        padding: 6px 10px;
    }
    .quick-jump-name {
        font-weight: 600;
    }
    .bookmark-active-star {
        color: #f6d32d;
    }
    paned.filemanager-paned > separator {
        min-height: 6px;
        background-color: rgba(255, 255, 255, 0.08);
        border-top: 1px solid rgba(255, 255, 255, 0.12);
        transition: background-color 150ms ease-in-out;
    }
    paned.filemanager-paned > separator:hover {
        background-color: alpha(@accent_color, 0.55);
    }
    paned.ai-paned > separator {
        min-height: 6px;
        background-color: rgba(255, 255, 255, 0.08);
        border-top: 1px solid rgba(255, 255, 255, 0.12);
        transition: background-color 150ms ease-in-out;
    }
    paned.ai-paned > separator:hover {
        background-color: alpha(@accent_color, 0.55);
    }
    .ai-chat-panel {
        background-color: rgba(24, 26, 32, 0.96);
    }
    .ai-panel-header {
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        padding: 4px 6px;
    }
    .quick-prompt-btn {
        padding: 8px 12px;
        border-radius: 8px;
        background-color: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        transition: all 120ms ease;
    }
    .quick-prompt-btn:hover {
        background-color: rgba(255, 255, 255, 0.09);
        border-color: rgba(255, 255, 255, 0.16);
    }
    .ai-input-pill {
        border-radius: 12px;
        padding: 4px 6px;
        background-color: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.12);
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

    #[test]
    fn test_theme_selector_css_contains_four_circle_modes() {
        let css = menu_custom_css();
        assert!(css.contains(".themeselector checkbutton"));
        assert!(css.contains(".themeselector checkbutton.follow"));
        assert!(css.contains(".themeselector checkbutton.light"));
        assert!(css.contains(".themeselector checkbutton.dark"));
        assert!(css.contains(".themeselector checkbutton.terminal"));
        assert!(css.contains("object-select-symbolic"));
        assert!(css.contains("border-radius: 9999px"));
    }

    #[test]
    fn test_file_manager_grid_css_matches_gtk_gridview() {
        let css = menu_custom_css();
        assert!(css.contains(".file-manager-grid-view > child"));
        assert!(css.contains(".file-manager-grid-view listitem"));
        assert!(!css.contains(".file-manager-grid-view flowboxchild"));
        assert!(css.contains("min-width: 90px;"));
    }
}
