//! Terminal Color Palettes and GTK4/Libadwaita CSS Generator
//! (`src/onyxsh/utils/theme_engine.py` & `src/onyxsh/ui/colors.py`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalPalette {
    pub id: &'static str,
    pub name: &'static str,
    pub foreground: &'static str,
    pub background: &'static str,
    pub cursor: &'static str,
    pub ansi: [&'static str; 16],
}

pub const ONYX_DARK_PALETTE: TerminalPalette = TerminalPalette {
    id: "onyx_dark",
    name: "Onyx Dark",
    foreground: "#e4e4ef",
    background: "#0f1117",
    cursor: "#58a6ff",
    ansi: [
        "#161922", "#ff5f56", "#27c93f", "#ffbd2e", "#58a6ff", "#d2a8ff", "#39c5cf", "#d0d7de",
        "#484f58", "#ff7b72", "#3fb950", "#d29922", "#79c0ff", "#bc8cff", "#56d4dd", "#ffffff",
    ],
};

pub const DRACULA_PALETTE: TerminalPalette = TerminalPalette {
    id: "dracula",
    name: "Dracula",
    foreground: "#f8f8f2",
    background: "#282a36",
    cursor: "#f8f8f2",
    ansi: [
        "#21222c", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd", "#f8f8f2",
        "#6272a4", "#ff6e6e", "#69ff94", "#ffffa5", "#d6acff", "#ff92df", "#a4ffff", "#ffffff",
    ],
};

pub const SOLARIZED_LIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "solarized_light",
    name: "Solarized Light",
    foreground: "#586e75",
    background: "#fdf6e3",
    cursor: "#586e75",
    ansi: [
        "#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198", "#eee8d5",
        "#002b36", "#cb4b16", "#586e75", "#657b83", "#839496", "#6c71c4", "#93a1a1", "#fdf6e3",
    ],
};

/// Resolves a terminal palette by ID, falling back to [`ONYX_DARK_PALETTE`].
pub fn get_palette(scheme_id: &str) -> &'static TerminalPalette {
    match scheme_id {
        "dracula" => &DRACULA_PALETTE,
        "solarized_light" => &SOLARIZED_LIGHT_PALETTE,
        _ => &ONYX_DARK_PALETTE,
    }
}

/// Computes relative luminance of a `#RRGGBB` hex color to determine whether a palette is light.
pub fn is_light_hex_color(hex: &str) -> bool {
    let clean = hex.trim_start_matches('#');
    if clean.len() != 6 {
        return false;
    }
    let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0) as f64 / 255.0;
    let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0) as f64 / 255.0;
    let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0) as f64 / 255.0;
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    luma > 0.55
}

/// Generates dynamic GTK4 CSS rules matching the selected palette and background opacity.
pub fn generate_theme_css(scheme_id: &str, opacity_percent: u8) -> String {
    let palette = get_palette(scheme_id);
    let alpha = (opacity_percent.clamp(20, 100) as f64) / 100.0;
    format!(
        "window {{ background-color: {bg}; color: {fg}; opacity: {alpha:.2}; }}\n\
         .onyx-terminal-container {{ background-color: {bg}; color: {fg}; }}\n",
        bg = palette.background,
        fg = palette.foreground,
        alpha = alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_lookup_and_luminance() {
        let dark = get_palette("onyx_dark");
        assert!(!is_light_hex_color(dark.background));

        let light = get_palette("solarized_light");
        assert!(is_light_hex_color(light.background));

        let css = generate_theme_css("dracula", 90);
        assert!(css.contains("#282a36"));
        assert!(css.contains("0.90"));
    }
}
