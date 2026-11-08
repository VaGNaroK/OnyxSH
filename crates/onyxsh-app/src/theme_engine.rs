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

pub const AYU_MIRAGE_PALETTE: TerminalPalette = TerminalPalette {
    id: "ayu_mirage",
    name: "Ayu Mirage",
    foreground: "#cccac2",
    background: "#1f2430",
    cursor: "#ffcc66",
    ansi: ["#191e2a", "#ff6666", "#b8cc52", "#ffcc66", "#73d0ff", "#d4bfff", "#95e6cb", "#e6e1cf", "#707a8c", "#ff6666", "#b8cc52", "#ffd173", "#73d0ff", "#dfbfff", "#95e6cb", "#f3f4f5"],
};

pub const CATPPUCCIN_PALETTE: TerminalPalette = TerminalPalette {
    id: "catppuccin",
    name: "Catppuccin Mocha",
    foreground: "#f5e0dc",
    background: "#1e1e2e",
    cursor: "#f5e0dc",
    ansi: ["#45475a", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#bac2de", "#585b70", "#f38ba8", "#a6e3a1", "#f9e2af", "#89b4fa", "#f5c2e7", "#94e2d5", "#cdd6f4"],
};

pub const CATPPUCCIN_LATTE_PALETTE: TerminalPalette = TerminalPalette {
    id: "catppuccin_latte",
    name: "Catppuccin Latte",
    foreground: "#4c4f69",
    background: "#eff1f5",
    cursor: "#dc8a78",
    ansi: ["#5c5f77", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299", "#acb0be", "#6c6f85", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299", "#4c4f69"],
};

pub const DARK_PALETTE: TerminalPalette = TerminalPalette {
    id: "dark",
    name: "Dark",
    foreground: "#ffffff",
    background: "#1c1c1c",
    cursor: "#ffffff",
    ansi: ["#000000", "#cc0000", "#4e9a06", "#c4a000", "#3465a4", "#75507b", "#06989a", "#d3d7cf", "#555753", "#ef2929", "#8ae234", "#fce94f", "#729fcf", "#ad7fa8", "#34e2e2", "#eeeeec"],
};

pub const DRACULA_PALETTE: TerminalPalette = TerminalPalette {
    id: "dracula",
    name: "Dracula",
    foreground: "#f8f8f2",
    background: "#282a36",
    cursor: "#f8f8f2",
    ansi: ["#000000", "#ff5555", "#50fa7b", "#f1fa8c", "#bd93f9", "#ff79c6", "#8be9fd", "#bfbfbf", "#4d4d4d", "#ff6e67", "#5af78e", "#f4f99d", "#caa9fa", "#ff92d0", "#9aedfe", "#e6e6e6"],
};

pub const EVERFOREST_PALETTE: TerminalPalette = TerminalPalette {
    id: "everforest",
    name: "Everforest Green",
    foreground: "#d3c6aa",
    background: "#2d353b",
    cursor: "#d3c6aa",
    ansi: ["#475258", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d3c6aa", "#475258", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d3c6aa"],
};

pub const EVERFOREST_DARK_PALETTE: TerminalPalette = TerminalPalette {
    id: "everforest_dark",
    name: "Everforest Dark",
    foreground: "#d3c6aa",
    background: "#272e33",
    cursor: "#d3c6aa",
    ansi: ["#414b50", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d3c6aa", "#414b50", "#e67e80", "#a7c080", "#dbbc7f", "#7fbbb3", "#d699b6", "#83c092", "#d3c6aa"],
};

pub const EVERFOREST_LIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "everforest_light",
    name: "Everforest Light",
    foreground: "#5c6a72",
    background: "#fdf6e3",
    cursor: "#5c6a72",
    ansi: ["#e0dcc7", "#f85552", "#8da101", "#dfa000", "#3a94c5", "#df69ba", "#35a77c", "#5c6a72", "#e0dcc7", "#f85552", "#8da101", "#dfa000", "#3a94c5", "#df69ba", "#35a77c", "#5c6a72"],
};

pub const GITHUB_DARK_PALETTE: TerminalPalette = TerminalPalette {
    id: "github_dark",
    name: "GitHub Dark",
    foreground: "#c9d1d9",
    background: "#0d1117",
    cursor: "#58a6ff",
    ansi: ["#484f58", "#ff7b72", "#3fb950", "#d29922", "#58a6ff", "#bc8cff", "#39c5cf", "#b1bac4", "#6e7681", "#ffa198", "#56d364", "#e3b341", "#79c0ff", "#d2a8ff", "#56d4dd", "#f0f6fc"],
};

pub const GITHUB_LIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "github_light",
    name: "GitHub Light",
    foreground: "#24292f",
    background: "#ffffff",
    cursor: "#0969da",
    ansi: ["#24292f", "#cf222e", "#116329", "#4d2d00", "#0969da", "#8250df", "#1b7c83", "#6e7781", "#57606a", "#a40e26", "#1a7f37", "#633c01", "#218bff", "#a475f9", "#3192aa", "#8c959f"],
};

pub const GRUVBOX_DARK_HARD_PALETTE: TerminalPalette = TerminalPalette {
    id: "gruvbox_dark_hard",
    name: "Gruvbox Dark Hard",
    foreground: "#ebdbb2",
    background: "#1d2021",
    cursor: "#ebdbb2",
    ansi: ["#1d2021", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a", "#a89984", "#928374", "#fb4934", "#b8bb26", "#fabd2f", "#83a598", "#d3869b", "#8ec07c", "#ebdbb2"],
};

pub const GRUVBOX_LIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "gruvbox_light",
    name: "Gruvbox Light",
    foreground: "#3c3836",
    background: "#fbf1c7",
    cursor: "#3c3836",
    ansi: ["#fbf1c7", "#cc241d", "#98971a", "#d79921", "#458588", "#b16286", "#689d6a", "#7c6f64", "#928374", "#9d0006", "#79740e", "#b57614", "#076678", "#8f3f71", "#427b58", "#3c3836"],
};

pub const HORIZON_DARK_PALETTE: TerminalPalette = TerminalPalette {
    id: "horizon_dark",
    name: "Horizon Dark",
    foreground: "#cbced0",
    background: "#1c1e26",
    cursor: "#e95678",
    ansi: ["#16161c", "#e95678", "#29d398", "#fab795", "#26bbd9", "#ee64ac", "#59e1e3", "#d5d8da", "#232530", "#ec6a88", "#3fdaa4", "#fbc3a7", "#3fc4de", "#f075b5", "#6be4e6", "#fadad1"],
};

pub const KANAGAWA_WAVE_PALETTE: TerminalPalette = TerminalPalette {
    id: "kanagawa_wave",
    name: "Kanagawa Wave",
    foreground: "#dcd7ba",
    background: "#1f1f28",
    cursor: "#c8c093",
    ansi: ["#090618", "#c34043", "#76946a", "#c0a36e", "#7e9cd8", "#957fb8", "#6a9589", "#c8c093", "#727169", "#e82424", "#98bb6c", "#e6c384", "#7fb4ca", "#938aa9", "#7aa89f", "#dcd7ba"],
};

pub const LIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "light",
    name: "Light",
    foreground: "#000000",
    background: "#ffffff",
    cursor: "#000000",
    ansi: ["#000000", "#cc0000", "#4e9a06", "#c4a000", "#3465a4", "#75507b", "#06989a", "#d3d7cf", "#555753", "#ef2929", "#8ae234", "#fce94f", "#729fcf", "#ad7fa8", "#34e2e2", "#eeeeec"],
};

pub const MATERIAL_OCEAN_PALETTE: TerminalPalette = TerminalPalette {
    id: "material_ocean",
    name: "Material Ocean",
    foreground: "#8f93a2",
    background: "#0f111a",
    cursor: "#ffcc00",
    ansi: ["#000000", "#ff5370", "#c3e88d", "#ffcb6b", "#82aaff", "#c792ea", "#89ddff", "#ffffff", "#545454", "#ff5370", "#c3e88d", "#ffcb6b", "#82aaff", "#c792ea", "#89ddff", "#ffffff"],
};

pub const MONOKAI_PALETTE: TerminalPalette = TerminalPalette {
    id: "monokai",
    name: "Monokai",
    foreground: "#f8f8f2",
    background: "#272822",
    cursor: "#f8f8f0",
    ansi: ["#272822", "#f92672", "#a6e22e", "#f4bf75", "#66d9ef", "#ae81ff", "#a1efe4", "#f8f8f2", "#75715e", "#f92672", "#a6e22e", "#f4bf75", "#66d9ef", "#ae81ff", "#a1efe4", "#f9f8f5"],
};

pub const MOONFLY_PALETTE: TerminalPalette = TerminalPalette {
    id: "moonfly",
    name: "Moonfly",
    foreground: "#bdbdbd",
    background: "#080808",
    cursor: "#9e9e9e",
    ansi: ["#323437", "#ff5454", "#8cc85f", "#e3c78a", "#80a0ff", "#cf87e8", "#79dac8", "#c6c6c6", "#949494", "#ff5189", "#36c692", "#c296eb", "#74b2ff", "#ae81ff", "#85dcbe", "#e4e4e4"],
};

pub const NIGHT_OWL_PALETTE: TerminalPalette = TerminalPalette {
    id: "night_owl",
    name: "Night Owl",
    foreground: "#d6deeb",
    background: "#011627",
    cursor: "#7e57c2",
    ansi: ["#011627", "#ef5350", "#22da6e", "#c5e478", "#82aaff", "#c792ea", "#21c7a8", "#ffffff", "#575656", "#ef5350", "#22da6e", "#ffeb95", "#82aaff", "#c792ea", "#7fdbca", "#ffffff"],
};

pub const NORD_PALETTE: TerminalPalette = TerminalPalette {
    id: "nord",
    name: "Nord",
    foreground: "#d8dee9",
    background: "#2e3440",
    cursor: "#d8dee9",
    ansi: ["#3b4252", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#88c0d0", "#e5e9f0", "#4c566a", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#8fbcbb", "#eceff4"],
};

pub const ONEDARK_PRO_PALETTE: TerminalPalette = TerminalPalette {
    id: "onedark_pro",
    name: "One Dark Pro",
    foreground: "#abb2bf",
    background: "#282c34",
    cursor: "#528bff",
    ansi: ["#282c34", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2", "#abb2bf", "#5c6370", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2", "#ffffff"],
};

pub const OXOCARBON_DARK_PALETTE: TerminalPalette = TerminalPalette {
    id: "oxocarbon_dark",
    name: "Oxocarbon Dark",
    foreground: "#f4f4f4",
    background: "#161616",
    cursor: "#ee5396",
    ansi: ["#262626", "#ff7eb6", "#42be65", "#ffe97b", "#33b1ff", "#be95ff", "#3ddbd9", "#dde1e6", "#393939", "#ff7eb6", "#42be65", "#ffe97b", "#33b1ff", "#be95ff", "#3ddbd9", "#ffffff"],
};

pub const PALENIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "palenight",
    name: "Palenight",
    foreground: "#bfc7d5",
    background: "#292d3e",
    cursor: "#ffcc00",
    ansi: ["#292d3e", "#f07178", "#c3e88d", "#ffcb6b", "#82aaff", "#c792ea", "#89ddff", "#d0d0d0", "#434758", "#f07178", "#c3e88d", "#ffcb6b", "#82aaff", "#c792ea", "#89ddff", "#ffffff"],
};

pub const PINK_LIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "pink_light",
    name: "Pink Light",
    foreground: "#4c4f69",
    background: "#fff0f5",
    cursor: "#ea76cb",
    ansi: ["#5c5f77", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299", "#acb0be", "#6c6f85", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299", "#4c4f69"],
};

pub const ROSE_PINE_PALETTE: TerminalPalette = TerminalPalette {
    id: "rose_pine",
    name: "Rosé Pine",
    foreground: "#e0def4",
    background: "#191724",
    cursor: "#56526e",
    ansi: ["#26233a", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7", "#ebbcba", "#e0def4", "#6e6a86", "#eb6f92", "#31748f", "#f6c177", "#9ccfd8", "#c4a7e7", "#ebbcba", "#e0def4"],
};

pub const SOLARIZED_DARK_PALETTE: TerminalPalette = TerminalPalette {
    id: "solarized_dark",
    name: "Solarized Dark",
    foreground: "#839496",
    background: "#002b36",
    cursor: "#839496",
    ansi: ["#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198", "#eee8d5", "#002b36", "#cb4b16", "#586e75", "#657b83", "#839496", "#6c71c4", "#93a1a1", "#fdf6e3"],
};

pub const SOLARIZED_LIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "solarized_light",
    name: "Solarized Light",
    foreground: "#586e75",
    background: "#fdf6e3",
    cursor: "#586e75",
    ansi: ["#073642", "#dc322f", "#859900", "#b58900", "#268bd2", "#d33682", "#2aa198", "#eee8d5", "#002b36", "#cb4b16", "#586e75", "#657b83", "#839496", "#6c71c4", "#93a1a1", "#fdf6e3"],
};

pub const SYSTEM_DEFAULT_PALETTE: TerminalPalette = TerminalPalette {
    id: "system_default",
    name: "System Default",
    foreground: "#ffffff",
    background: "#000000",
    cursor: "#ffffff",
    ansi: ["#000000", "#aa0000", "#00aa00", "#aa5500", "#0000aa", "#aa00aa", "#00aaaa", "#aaaaaa", "#555555", "#ff5555", "#55ff55", "#ffff55", "#5555ff", "#ff55ff", "#55ffff", "#ffffff"],
};

pub const TOKYONIGHT_PALETTE: TerminalPalette = TerminalPalette {
    id: "tokyonight",
    name: "Tokyo Night",
    foreground: "#a9b1d6",
    background: "#1a1b26",
    cursor: "#c0caf5",
    ansi: ["#15161e", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#a9b1d6", "#414868", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#bb9af7", "#7dcfff", "#c0caf5"],
};

pub const VSCODE_DARK_PLUS_PALETTE: TerminalPalette = TerminalPalette {
    id: "vscode_dark_plus",
    name: "VS Code Dark+",
    foreground: "#d4d4d4",
    background: "#1e1e1e",
    cursor: "#aeafad",
    ansi: ["#000000", "#cd3131", "#0dbc79", "#e5e510", "#2472c8", "#bc3fbc", "#11a8cd", "#e5e5e5", "#666666", "#f14c4c", "#23d18b", "#f5f543", "#3b8eea", "#d670d6", "#29b8db", "#e5e5e5"],
};

pub const BUILTIN_SCHEMES: &[TerminalPalette] = &[
    AYU_MIRAGE_PALETTE,
    CATPPUCCIN_PALETTE,
    CATPPUCCIN_LATTE_PALETTE,
    DARK_PALETTE,
    DRACULA_PALETTE,
    EVERFOREST_PALETTE,
    EVERFOREST_DARK_PALETTE,
    EVERFOREST_LIGHT_PALETTE,
    GITHUB_DARK_PALETTE,
    GITHUB_LIGHT_PALETTE,
    GRUVBOX_DARK_HARD_PALETTE,
    GRUVBOX_LIGHT_PALETTE,
    HORIZON_DARK_PALETTE,
    KANAGAWA_WAVE_PALETTE,
    LIGHT_PALETTE,
    MATERIAL_OCEAN_PALETTE,
    MONOKAI_PALETTE,
    MOONFLY_PALETTE,
    NIGHT_OWL_PALETTE,
    NORD_PALETTE,
    ONEDARK_PRO_PALETTE,
    OXOCARBON_DARK_PALETTE,
    PALENIGHT_PALETTE,
    PINK_LIGHT_PALETTE,
    ROSE_PINE_PALETTE,
    SOLARIZED_DARK_PALETTE,
    SOLARIZED_LIGHT_PALETTE,
    SYSTEM_DEFAULT_PALETTE,
    TOKYONIGHT_PALETTE,
    VSCODE_DARK_PLUS_PALETTE,
];

/// Returns all built-in terminal color schemes in order.
pub fn all_builtin_schemes() -> &'static [TerminalPalette] {
    BUILTIN_SCHEMES
}

/// Resolves a terminal palette by ID, falling back to [`DARK_PALETTE`].
pub fn get_palette(scheme_id: &str) -> &'static TerminalPalette {
    if scheme_id == "onyx_dark" {
        return &ONYX_DARK_PALETTE;
    }
    for s in BUILTIN_SCHEMES {
        if s.id == scheme_id {
            return s;
        }
    }
    &DARK_PALETTE
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

        assert_eq!(all_builtin_schemes().len(), 30);
        let github = get_palette("github_dark");
        assert_eq!(github.name, "GitHub Dark");
    }
}
