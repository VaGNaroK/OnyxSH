//! High-performance DFA regex syntax highlighter for terminal output and shell input.
//! Port of `src/onyxsh/terminal/_highlighter_impl.py` and `src/onyxsh/terminal/highlighter/*`.

use onyxsh_core::platform;
use regex::{Regex, RegexBuilder, RegexSet};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static HIGHLIGHT_CONFIG_REVISION: AtomicU64 = AtomicU64::new(1);

/// Returns the shared atomic revision counter bumped whenever highlight settings change.
pub fn highlight_config_revision() -> u64 {
    HIGHLIGHT_CONFIG_REVISION.load(Ordering::Relaxed)
}

/// Signals running PTY proxies to hot-reload their compiled `HighlightEngine`.
pub fn notify_highlight_config_changed() {
    HIGHLIGHT_CONFIG_REVISION.fetch_add(1, Ordering::Relaxed);
}

fn default_true() -> bool {
    true
}

pub const DEFAULT_IGNORED_COMMANDS: &[&str] = &[
    "top", "htop", "btop", "vim", "nvim", "nano", "less", "more", "man", "tmux", "screen", "tig",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileRuleConfig {
    pub name: String,
    pub pattern: String,
    pub colors: Vec<Option<String>>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub description: String,
    pub comment: String,
}

impl Default for ProfileRuleConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            pattern: String::new(),
            colors: vec![Some("bold yellow".to_string())],
            enabled: true,
            description: String::new(),
            comment: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlightProfile {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub use_global_rules: bool,
    pub rules: Vec<ProfileRuleConfig>,
}

impl Default for HighlightProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            triggers: Vec::new(),
            enabled: true,
            use_global_rules: true,
            rules: Vec::new(),
        }
    }
}

const BUNDLED_HIGHLIGHT_JSONS: &[(&str, &str)] = &[
    ("blkid", include_str!("../../../src/onyxsh/data/highlights/blkid.json")),
    ("cargo", include_str!("../../../src/onyxsh/data/highlights/cargo.json")),
    ("curl", include_str!("../../../src/onyxsh/data/highlights/curl.json")),
    ("dd", include_str!("../../../src/onyxsh/data/highlights/dd.json")),
    ("df", include_str!("../../../src/onyxsh/data/highlights/df.json")),
    ("dig", include_str!("../../../src/onyxsh/data/highlights/dig.json")),
    ("dmesg", include_str!("../../../src/onyxsh/data/highlights/dmesg.json")),
    ("docker", include_str!("../../../src/onyxsh/data/highlights/docker.json")),
    ("du", include_str!("../../../src/onyxsh/data/highlights/du.json")),
    ("find", include_str!("../../../src/onyxsh/data/highlights/find.json")),
    ("findmnt", include_str!("../../../src/onyxsh/data/highlights/findmnt.json")),
    ("free", include_str!("../../../src/onyxsh/data/highlights/free.json")),
    ("gcc", include_str!("../../../src/onyxsh/data/highlights/gcc.json")),
    ("git", include_str!("../../../src/onyxsh/data/highlights/git.json")),
    ("global", include_str!("../../../src/onyxsh/data/highlights/global.json")),
    ("help", include_str!("../../../src/onyxsh/data/highlights/help.json")),
    ("ifconfig", include_str!("../../../src/onyxsh/data/highlights/ifconfig.json")),
    ("ip", include_str!("../../../src/onyxsh/data/highlights/ip.json")),
    ("journalctl", include_str!("../../../src/onyxsh/data/highlights/journalctl.json")),
    ("kubectl", include_str!("../../../src/onyxsh/data/highlights/kubectl.json")),
    ("last", include_str!("../../../src/onyxsh/data/highlights/last.json")),
    ("ls", include_str!("../../../src/onyxsh/data/highlights/ls.json")),
    ("lsblk", include_str!("../../../src/onyxsh/data/highlights/lsblk.json")),
    ("lsmod", include_str!("../../../src/onyxsh/data/highlights/lsmod.json")),
    ("lsof", include_str!("../../../src/onyxsh/data/highlights/lsof.json")),
    ("lspci", include_str!("../../../src/onyxsh/data/highlights/lspci.json")),
    ("lsusb", include_str!("../../../src/onyxsh/data/highlights/lsusb.json")),
    ("make", include_str!("../../../src/onyxsh/data/highlights/make.json")),
    ("mount", include_str!("../../../src/onyxsh/data/highlights/mount.json")),
    ("mtr", include_str!("../../../src/onyxsh/data/highlights/mtr.json")),
    ("nc", include_str!("../../../src/onyxsh/data/highlights/nc.json")),
    ("netstat", include_str!("../../../src/onyxsh/data/highlights/netstat.json")),
    ("nmap", include_str!("../../../src/onyxsh/data/highlights/nmap.json")),
    ("npm", include_str!("../../../src/onyxsh/data/highlights/npm.json")),
    ("nslookup", include_str!("../../../src/onyxsh/data/highlights/nslookup.json")),
    ("ping", include_str!("../../../src/onyxsh/data/highlights/ping.json")),
    ("pip", include_str!("../../../src/onyxsh/data/highlights/pip.json")),
    ("ps", include_str!("../../../src/onyxsh/data/highlights/ps.json")),
    ("python", include_str!("../../../src/onyxsh/data/highlights/python.json")),
    ("rsync", include_str!("../../../src/onyxsh/data/highlights/rsync.json")),
    ("sensors", include_str!("../../../src/onyxsh/data/highlights/sensors.json")),
    ("ss", include_str!("../../../src/onyxsh/data/highlights/ss.json")),
    ("sysctl", include_str!("../../../src/onyxsh/data/highlights/sysctl.json")),
    ("systemctl", include_str!("../../../src/onyxsh/data/highlights/systemctl.json")),
    ("tar", include_str!("../../../src/onyxsh/data/highlights/tar.json")),
    ("terraform", include_str!("../../../src/onyxsh/data/highlights/terraform.json")),
    ("traceroute", include_str!("../../../src/onyxsh/data/highlights/traceroute.json")),
    ("uptime", include_str!("../../../src/onyxsh/data/highlights/uptime.json")),
    ("vmstat", include_str!("../../../src/onyxsh/data/highlights/vmstat.json")),
    ("wget", include_str!("../../../src/onyxsh/data/highlights/wget.json")),
    ("yarn", include_str!("../../../src/onyxsh/data/highlights/yarn.json")),
];

/// Returns all 51 built-in highlight profiles bundled from `src/onyxsh/data/highlights/*.json`.
pub fn builtin_highlight_profiles() -> Vec<HighlightProfile> {
    let mut out = Vec::with_capacity(BUNDLED_HIGHLIGHT_JSONS.len());
    for (fallback_name, raw) in BUNDLED_HIGHLIGHT_JSONS {
        if let Ok(mut prof) = serde_json::from_str::<HighlightProfile>(raw) {
            if prof.name.trim().is_empty() {
                prof.name = (*fallback_name).to_string();
            }
            out.push(prof);
        }
    }
    out
}

pub fn default_global_rules_from_bundle() -> Vec<ProfileRuleConfig> {
    for (name, raw) in BUNDLED_HIGHLIGHT_JSONS {
        if *name == "global" {
            if let Ok(prof) = serde_json::from_str::<HighlightProfile>(raw) {
                if !prof.rules.is_empty() {
                    return prof.rules;
                }
            }
        }
    }
    DEFAULT_OUTPUT_RULES
        .iter()
        .map(|r| ProfileRuleConfig {
            name: r.name.to_string(),
            pattern: r.pattern.to_string(),
            colors: vec![Some("bold yellow".to_string())],
            enabled: true,
            description: String::new(),
            comment: String::new(),
        })
        .collect()
}

/// Persistent syntax-highlighting configuration stored in `~/.config/onyxsh/highlights.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlightConfig {
    pub output_highlighting_enabled: bool,
    pub command_highlighting_enabled: bool,
    pub cat_colorization_enabled: bool,
    pub shell_input_highlighting_enabled: bool,
    pub ignored_commands: Vec<String>,
    pub global_rules: Vec<ProfileRuleConfig>,
    pub profiles: BTreeMap<String, HighlightProfile>,
}

impl Default for HighlightConfig {
    fn default() -> Self {
        let mut profiles = BTreeMap::new();
        for p in builtin_highlight_profiles() {
            if p.name != "global" {
                profiles.insert(p.name.clone(), p);
            }
        }
        Self {
            output_highlighting_enabled: true,
            command_highlighting_enabled: true,
            cat_colorization_enabled: true,
            shell_input_highlighting_enabled: true,
            ignored_commands: DEFAULT_IGNORED_COMMANDS
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            global_rules: default_global_rules_from_bundle(),
            profiles,
        }
    }
}

impl HighlightConfig {
    pub fn default_path() -> PathBuf {
        platform::config_dir().join("highlights.json")
    }

    pub fn load_default() -> Self {
        Self::load_from_path(&Self::default_path())
    }

    pub fn load_from_path(path: &Path) -> Self {
        let mut base = Self::default();
        if let Ok(raw) = fs::read_to_string(path) {
            if let Ok(loaded) = serde_json::from_str::<HighlightConfig>(&raw) {
                base.output_highlighting_enabled = loaded.output_highlighting_enabled;
                base.command_highlighting_enabled = loaded.command_highlighting_enabled;
                base.cat_colorization_enabled = loaded.cat_colorization_enabled;
                base.shell_input_highlighting_enabled = loaded.shell_input_highlighting_enabled;
                if !loaded.ignored_commands.is_empty() {
                    base.ignored_commands = loaded.ignored_commands;
                }
                if !loaded.global_rules.is_empty() {
                    base.global_rules = loaded.global_rules;
                }
                for (k, v) in loaded.profiles {
                    base.profiles.insert(k, v);
                }
            }
        }
        base
    }

    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, body)
    }

    pub fn save_default(&self) -> std::io::Result<()> {
        self.save_to_path(&Self::default_path())?;
        notify_highlight_config_changed();
        Ok(())
    }
}

/// Converts a color name/hex spec (e.g. `"bold red"`, `"bright_cyan"`, `"#ff5555"`) into an ANSI SGR sequence.
pub fn color_spec_to_sgr(spec: &str) -> String {
    let clean = spec.trim();
    if clean.starts_with("\x1b[") {
        return clean.to_string();
    }
    if let Some(hex) = clean.strip_prefix('#') {
        if hex.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                return format!("\x1b[38;2;{r};{g};{b}m");
            }
        }
    }

    let mut codes: Vec<&'static str> = Vec::new();
    for tok in clean.to_lowercase().split(|c: char| c.is_whitespace()) {
        match tok {
            "bold" => codes.push("1"),
            "dim" => codes.push("2"),
            "italic" => codes.push("3"),
            "underline" => codes.push("4"),
            "black" => codes.push("30"),
            "red" => codes.push("31"),
            "green" => codes.push("32"),
            "yellow" => codes.push("33"),
            "blue" => codes.push("34"),
            "magenta" => codes.push("35"),
            "cyan" => codes.push("36"),
            "white" => codes.push("37"),
            "bright_black" => codes.push("90"),
            "bright_red" => codes.push("91"),
            "bright_green" => codes.push("92"),
            "bright_yellow" => codes.push("93"),
            "bright_blue" => codes.push("94"),
            "bright_magenta" => codes.push("95"),
            "bright_cyan" => codes.push("96"),
            "bright_white" => codes.push("97"),
            "on_black" | "bg_black" => codes.push("40"),
            "on_red" | "bg_red" => codes.push("41"),
            "on_green" | "bg_green" => codes.push("42"),
            "on_yellow" | "bg_yellow" => codes.push("43"),
            "on_blue" | "bg_blue" => codes.push("44"),
            "on_magenta" | "bg_magenta" => codes.push("45"),
            "on_cyan" | "bg_cyan" => codes.push("46"),
            "on_white" | "bg_white" => codes.push("47"),
            "bright" | "none" | "default" | "" => {}
            _ => {}
        }
    }
    let lower = clean.to_lowercase();
    if lower.contains("bright red") && !codes.contains(&"91") {
        codes.retain(|c| *c != "31");
        codes.push("91");
    } else if lower.contains("bright green") && !codes.contains(&"92") {
        codes.retain(|c| *c != "32");
        codes.push("92");
    } else if lower.contains("bright yellow") && !codes.contains(&"93") {
        codes.retain(|c| *c != "33");
        codes.push("93");
    } else if lower.contains("bright blue") && !codes.contains(&"94") {
        codes.retain(|c| *c != "34");
        codes.push("94");
    } else if lower.contains("bright magenta") && !codes.contains(&"95") {
        codes.retain(|c| *c != "35");
        codes.push("95");
    } else if lower.contains("bright cyan") && !codes.contains(&"96") {
        codes.retain(|c| *c != "36");
        codes.push("96");
    } else if lower.contains("bright white") && !codes.contains(&"97") {
        codes.retain(|c| *c != "37");
        codes.push("97");
    }

    if codes.is_empty() {
        "\x1b[1;33m".to_string()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

#[derive(Debug, Clone)]
struct CompiledRuleEntry {
    regex: Regex,
    colors: Vec<Option<String>>,
}

#[derive(Debug, Clone)]
struct CompiledProfileEntry {
    triggers: Vec<String>,
    use_global_rules: bool,
    rules: Vec<CompiledRuleEntry>,
}

/// Live highlighting engine combining the 51 command profiles + custom global rules.
#[derive(Debug, Clone)]
pub struct HighlightEngine {
    pub output_enabled: bool,
    pub command_enabled: bool,
    ignored_commands: Vec<String>,
    global_rules: Vec<CompiledRuleEntry>,
    profiles: Vec<CompiledProfileEntry>,
}

impl Default for HighlightEngine {
    fn default() -> Self {
        Self::from_config(&HighlightConfig::default())
    }
}

impl HighlightEngine {
    pub fn from_config(cfg: &HighlightConfig) -> Self {
        let mut global_rules = Vec::new();
        for rule in &cfg.global_rules {
            if !rule.enabled || rule.pattern.trim().is_empty() {
                continue;
            }
            if let Ok(re) = RegexBuilder::new(&rule.pattern).case_insensitive(true).build() {
                let sgrs: Vec<Option<String>> = if rule.colors.is_empty() {
                    vec![Some("\x1b[1;33m".to_string())]
                } else {
                    rule.colors
                        .iter()
                        .map(|opt| opt.as_deref().map(color_spec_to_sgr))
                        .collect()
                };
                global_rules.push(CompiledRuleEntry {
                    regex: re,
                    colors: sgrs,
                });
            }
        }

        let mut profiles = Vec::new();
        for prof in cfg.profiles.values() {
            if !prof.enabled {
                continue;
            }
            let mut rules = Vec::new();
            for rule in &prof.rules {
                if !rule.enabled || rule.pattern.trim().is_empty() {
                    continue;
                }
                if let Ok(re) = Regex::new(&rule.pattern) {
                    let sgrs: Vec<Option<String>> = if rule.colors.is_empty() {
                        vec![Some("\x1b[36m".to_string())]
                    } else {
                        rule.colors
                            .iter()
                            .map(|opt| opt.as_deref().map(color_spec_to_sgr))
                            .collect()
                    };
                    rules.push(CompiledRuleEntry {
                        regex: re,
                        colors: sgrs,
                    });
                }
            }
            if !rules.is_empty() {
                profiles.push(CompiledProfileEntry {
                    triggers: prof.triggers.iter().map(|t| t.to_lowercase()).collect(),
                    use_global_rules: prof.use_global_rules,
                    rules,
                });
            }
        }

        Self {
            output_enabled: cfg.output_highlighting_enabled,
            command_enabled: cfg.command_highlighting_enabled,
            ignored_commands: cfg
                .ignored_commands
                .iter()
                .map(|c| c.trim().to_lowercase())
                .filter(|c| !c.is_empty())
                .collect(),
            global_rules,
            profiles,
        }
    }

    pub fn should_ignore_command(&self, command_line: Option<&str>) -> bool {
        let Some(cmd) = command_line else {
            return false;
        };
        let Some(first_tok) = cmd
            .split_whitespace()
            .find(|tok| !matches!(*tok, "sudo" | "doas" | "env" | "time" | "nohup"))
        else {
            return false;
        };
        let bin_name = Path::new(first_tok)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(first_tok)
            .to_lowercase();
        self.ignored_commands.iter().any(|c| c == &bin_name)
    }

    fn active_profile_for(&self, command_line: Option<&str>) -> Option<&CompiledProfileEntry> {
        if !self.command_enabled {
            return None;
        }
        let cmd = command_line?;
        let first_tok = cmd
            .split_whitespace()
            .find(|tok| !matches!(*tok, "sudo" | "doas" | "env" | "time" | "nohup"))?;
        let bin_name = Path::new(first_tok)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(first_tok)
            .to_lowercase();
        self.profiles
            .iter()
            .find(|p| p.triggers.iter().any(|t| t == &bin_name))
    }

    /// Highlights a single plain-text line segment, preserving existing ANSI sequences intact.
    pub fn highlight_line(&self, line: &str, active_command: Option<&str>) -> String {
        if line.is_empty() || !self.output_enabled || self.should_ignore_command(active_command) {
            return line.to_string();
        }
        // Do not alter lines that already carry explicit ANSI styling sequences.
        if line.contains('\x1b') {
            return line.to_string();
        }

        let matched_profile = self.active_profile_for(active_command);
        let apply_global = matched_profile.map_or(true, |p| p.use_global_rules);

        let mut rules_to_apply: Vec<&CompiledRuleEntry> = Vec::new();
        if let Some(prof) = matched_profile {
            rules_to_apply.extend(prof.rules.iter());
        }
        if apply_global {
            rules_to_apply.extend(self.global_rules.iter());
        }
        if rules_to_apply.is_empty() {
            return line.to_string();
        }

        // Single-pass rule application on non-ANSI spans only so earlier color tags are never mangled.
        let mut current = line.to_string();
        for rule in rules_to_apply {
            current = apply_rule_to_plain_segments(&current, rule);
        }
        current
    }
}

fn apply_rule_to_plain_segments(input: &str, rule: &CompiledRuleEntry) -> String {
    if !input.contains('\x1b') {
        return apply_single_rule(input, rule);
    }
    let mut out = String::with_capacity(input.len() + 16);
    let bytes = input.as_bytes();
    let mut idx = 0;
    let mut plain_start = 0;

    while idx < bytes.len() {
        if bytes[idx] == 0x1b && idx + 1 < bytes.len() && bytes[idx + 1] == b'[' {
            if idx > plain_start {
                out.push_str(&apply_single_rule(&input[plain_start..idx], rule));
            }
            let esc_start = idx;
            idx += 2;
            while idx < bytes.len() && !(0x40..=0x7e).contains(&bytes[idx]) {
                idx += 1;
            }
            if idx < bytes.len() {
                idx += 1;
            }
            out.push_str(&input[esc_start..idx]);
            plain_start = idx;
        } else {
            idx += 1;
        }
    }
    if plain_start < bytes.len() {
        out.push_str(&apply_single_rule(&input[plain_start..], rule));
    }
    out
}

fn apply_single_rule(segment: &str, rule: &CompiledRuleEntry) -> String {
    if segment.is_empty() {
        return String::new();
    }
    let primary = rule.colors.first().and_then(|o| o.as_deref());
    rule.regex
        .replace_all(segment, |caps: &regex::Captures| {
            let full = &caps[0];
            if caps.len() > 1 && rule.colors.len() > 1 {
                let mut rebuilt = String::with_capacity(full.len() + 24);
                let full_start = caps.get(0).map(|m| m.start()).unwrap_or(0);
                let mut cursor = full_start;
                for group_idx in 1..caps.len() {
                    if let Some(m) = caps.get(group_idx) {
                        if m.start() > cursor {
                            rebuilt.push_str(&segment[cursor..m.start()]);
                        }
                        let col_opt = match rule.colors.get(group_idx - 1) {
                            Some(inner) => inner.as_deref(),
                            None => primary,
                        };
                        if let Some(col) = col_opt {
                            rebuilt.push_str(col);
                            rebuilt.push_str(m.as_str());
                            rebuilt.push_str("\x1b[0m");
                        } else {
                            rebuilt.push_str(m.as_str());
                        }
                        cursor = m.end();
                    }
                }
                let full_end = full_start + full.len();
                if cursor < full_end {
                    rebuilt.push_str(&segment[cursor..full_end]);
                }
                rebuilt
            } else if let Some(col) = primary {
                format!("{col}{full}\x1b[0m")
            } else {
                full.to_string()
            }
        })
        .into_owned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightRule {
    pub name: &'static str,
    pub pattern: &'static str,
    pub ansi_color: &'static str,
}

pub const DEFAULT_OUTPUT_RULES: &[HighlightRule] = &[
    HighlightRule {
        name: "error_keyword",
        pattern: r"(?i)\b(?:ERROR|CRITICAL|FATAL|PANIC|FAILED|FAILURE)\b",
        ansi_color: "\x1b[1;31m", // Bold Red
    },
    HighlightRule {
        name: "warning_keyword",
        pattern: r"(?i)\b(?:WARN|WARNING|DEPRECATED|CAUTION)\b",
        ansi_color: "\x1b[1;33m", // Bold Yellow
    },
    HighlightRule {
        name: "success_keyword",
        pattern: r"(?i)\b(?:OK|SUCCESS|PASSED|ACTIVE|RUNNING|CONNECTED)\b",
        ansi_color: "\x1b[1;32m", // Bold Green
    },
    HighlightRule {
        name: "info_keyword",
        pattern: r"(?i)\b(?:INFO|NOTICE|DEBUG|TRACE)\b",
        ansi_color: "\x1b[1;36m", // Bold Cyan
    },
    HighlightRule {
        name: "ipv4_address",
        pattern: r"\b(?:25[0-5]|2[0-4]\d|1?\d?\d)(?:\.(?:25[0-5]|2[0-4]\d|1?\d?\d)){3}\b",
        ansi_color: "\x1b[35m", // Magenta
    },
    HighlightRule {
        name: "url",
        pattern: r"https?://[^\s<>'\x22]+",
        ansi_color: "\x1b[4;34m", // Underlined Blue
    },
];

struct CompiledHighlighter {
    set: RegexSet,
    regexes: Vec<(Regex, &'static str)>,
}

static OUTPUT_HIGHLIGHTER: OnceLock<CompiledHighlighter> = OnceLock::new();

fn output_highlighter() -> &'static CompiledHighlighter {
    OUTPUT_HIGHLIGHTER.get_or_init(|| {
        let patterns: Vec<&str> = DEFAULT_OUTPUT_RULES.iter().map(|r| r.pattern).collect();
        let set = RegexSet::new(&patterns).expect("valid RegexSet");
        let regexes = DEFAULT_OUTPUT_RULES
            .iter()
            .map(|r| (Regex::new(r.pattern).expect("valid rule regex"), r.ansi_color))
            .collect();
        CompiledHighlighter { set, regexes }
    })
}

/// Applies single-pass DFA RegexSet filtering followed by targeted ANSI color injection.
/// Skips lines that do not match any pattern in O(n) time without allocating.
pub fn highlight_output_line(line: &str) -> String {
    if line.is_empty() {
        return String::new();
    }
    let h = output_highlighter();
    let matches = h.set.matches(line);
    if !matches.matched_any() {
        return line.to_string();
    }

    let mut result = line.to_string();
    for idx in matches.into_iter() {
        let (re, color) = &h.regexes[idx];
        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                format!("{color}{}\x1b[0m", &caps[0])
            })
            .into_owned();
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellTokenKind {
    Command,
    Flag,
    Variable,
    QuotedString,
    Operator,
    Argument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellToken {
    pub text: String,
    pub kind: ShellTokenKind,
}

/// Tokenizes a shell input line for real-time syntax highlighting.
pub fn tokenize_shell_input(input: &str) -> Vec<ShellToken> {
    let mut tokens = Vec::new();
    let mut expect_command = true;

    for raw in input.split_whitespace() {
        let kind = if matches!(raw, "|" | "&&" | "||" | ";" | ">" | ">>" | "<") {
            expect_command = true;
            ShellTokenKind::Operator
        } else if expect_command {
            if !matches!(raw, "sudo" | "doas" | "env" | "time" | "nohup") {
                expect_command = false;
            }
            ShellTokenKind::Command
        } else if raw.starts_with('-') {
            ShellTokenKind::Flag
        } else if raw.starts_with('$') {
            ShellTokenKind::Variable
        } else if (raw.starts_with('"') && raw.ends_with('"'))
            || (raw.starts_with('\'') && raw.ends_with('\''))
        {
            ShellTokenKind::QuotedString
        } else {
            ShellTokenKind::Argument
        };

        tokens.push(ShellToken {
            text: raw.to_string(),
            kind,
        });
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_highlighter_applies_ansi_codes() {
        let plain = "plain text without matches";
        assert_eq!(highlight_output_line(plain), plain);

        let log_line = "2026-11-04 ERROR Connection failed to 192.168.1.50";
        let highlighted = highlight_output_line(log_line);
        assert!(highlighted.contains("\x1b[1;31mERROR\x1b[0m"));
        assert!(highlighted.contains("\x1b[35m192.168.1.50\x1b[0m"));
    }

    #[test]
    fn test_shell_input_tokenizer() {
        let tokens = tokenize_shell_input("sudo systemctl --now restart $SERVICE | grep active");
        assert_eq!(tokens[0].kind, ShellTokenKind::Command); // sudo
        assert_eq!(tokens[1].kind, ShellTokenKind::Command); // systemctl
        assert_eq!(tokens[2].kind, ShellTokenKind::Flag); // --now
        assert_eq!(tokens[3].kind, ShellTokenKind::Argument); // restart
        assert_eq!(tokens[4].kind, ShellTokenKind::Variable); // $SERVICE
        assert_eq!(tokens[5].kind, ShellTokenKind::Operator); // |
        assert_eq!(tokens[6].kind, ShellTokenKind::Command); // grep
    }

    #[test]
    fn test_builtin_51_profiles_and_engine_highlighting() {
        let profiles = builtin_highlight_profiles();
        assert_eq!(profiles.len(), 51, "Expected all 51 highlight JSON profiles");

        let engine = HighlightEngine::default();
        assert!(engine.should_ignore_command(Some("sudo vim /etc/hosts")));
        assert!(!engine.should_ignore_command(Some("git status")));

        let err_highlighted = engine.highlight_line("FATAL connection lost 10.0.0.1", Some("cat server.log"));
        assert!(err_highlighted.contains("\x1b["));
        assert!(err_highlighted.contains("10.0.0.1"));

        let curl_highlighted = engine.highlight_line("HTTP/1.1 404 Not Found", Some("curl https://example.com"));
        assert!(curl_highlighted.contains("\x1b["));

        let git_diff = engine.highlight_line("@@ -10,4 +10,5 @@", Some("git diff"));
        assert!(git_diff.contains("\x1b["));

        let untouched_tui = engine.highlight_line("ERROR inside vim", Some("vim notes.txt"));
        assert_eq!(untouched_tui, "ERROR inside vim");

        // Verify persistence roundtrip
        let tmp = std::env::temp_dir().join("onyxsh_test_highlights.json");
        let mut cfg = HighlightConfig::default();
        cfg.output_highlighting_enabled = false;
        cfg.save_to_path(&tmp).unwrap();
        let reloaded = HighlightConfig::load_from_path(&tmp);
        assert!(!reloaded.output_highlighting_enabled);
        assert_eq!(reloaded.profiles.len(), 50); // 51 minus global
        let _ = fs::remove_file(&tmp);
    }
}
