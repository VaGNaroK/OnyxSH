//! High-performance DFA regex syntax highlighter for terminal output and shell input.
//! Port of `src/onyxsh/terminal/_highlighter_impl.py` and `src/onyxsh/terminal/highlighter/*`.

use regex::{Regex, RegexSet};
use std::sync::OnceLock;

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
}
