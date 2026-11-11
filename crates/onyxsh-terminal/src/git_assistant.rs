//! Git Status Detector, Conventional Commit Generator, and Background Command Desktop Notifier
//! (`src/onyxsh/terminal/git_assistant.py`, `src/onyxsh/utils/git_utils.py`,
//! and `src/onyxsh/terminal/desktop_notifier.py`).

use onyxsh_core::i18n::{tr, tr_fmt};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatusSummary {
    pub branch: String,
    pub staged_files: Vec<String>,
    pub modified_files: Vec<String>,
    pub untracked_files: Vec<String>,
    pub deleted_files: Vec<String>,
}

impl GitStatusSummary {
    pub fn is_clean(&self) -> bool {
        self.staged_files.is_empty()
            && self.modified_files.is_empty()
            && self.untracked_files.is_empty()
            && self.deleted_files.is_empty()
    }

    pub fn total_changed_count(&self) -> usize {
        self.staged_files.len()
            + self.modified_files.len()
            + self.untracked_files.len()
            + self.deleted_files.len()
    }
}

/// Parses `git status --porcelain=v1 -b` output into a structured [`GitStatusSummary`].
pub fn parse_git_status_porcelain(output: &str) -> GitStatusSummary {
    let mut summary = GitStatusSummary {
        branch: "main".to_string(),
        ..Default::default()
    };

    for line in output.lines() {
        if let Some(branch_info) = line.strip_prefix("## ") {
            let clean_branch = branch_info
                .split("...")
                .next()
                .unwrap_or(branch_info)
                .split_whitespace()
                .next()
                .unwrap_or("HEAD");
            summary.branch = clean_branch.to_string();
            continue;
        }
        if line.len() < 3 {
            continue;
        }
        let xy = &line[..2];
        let file_path = line[3..].trim().to_string();
        if file_path.is_empty() {
            continue;
        }

        let x = xy.chars().next().unwrap_or(' ');
        let y = xy.chars().nth(1).unwrap_or(' ');

        if xy == "??" {
            summary.untracked_files.push(file_path);
        } else {
            if x != ' ' && x != '?' {
                summary.staged_files.push(file_path.clone());
            }
            if x == 'D' || y == 'D' {
                summary.deleted_files.push(file_path);
            } else if y != ' ' && y != '?' {
                summary.modified_files.push(file_path);
            }
        }
    }

    summary
}

/// Inspects a local Git repository at `repo_dir` using `git status --porcelain=v1 -b`.
pub fn inspect_git_repository(repo_dir: &Path) -> Option<GitStatusSummary> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["status", "--porcelain=v1", "-b"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(parse_git_status_porcelain(&String::from_utf8_lossy(
        &out.stdout,
    )))
}

/// Generates a Conventional Commit message suggestion based on modified/staged files.
pub fn suggest_conventional_commit(summary: &GitStatusSummary) -> String {
    if summary.is_clean() {
        return "chore: working tree clean".to_string();
    }

    let mut all_paths = Vec::new();
    all_paths.extend(summary.staged_files.iter());
    all_paths.extend(summary.modified_files.iter());
    all_paths.extend(summary.untracked_files.iter());
    all_paths.extend(summary.deleted_files.iter());

    let all_docs = all_paths
        .iter()
        .all(|p| p.ends_with(".md") || p.starts_with("docs/"));
    let all_tests = all_paths
        .iter()
        .all(|p| p.contains("test") || p.starts_with("tests/"));
    let has_cargo_or_deps = all_paths
        .iter()
        .any(|p| p.ends_with("Cargo.toml") || p.ends_with("pyproject.toml"));

    let prefix = if all_docs {
        "docs"
    } else if all_tests {
        "test"
    } else if has_cargo_or_deps && all_paths.len() <= 2 {
        "build"
    } else if !summary.untracked_files.is_empty() {
        "feat"
    } else {
        "fix"
    };

    let sample_files: Vec<&str> = all_paths
        .iter()
        .take(3)
        .map(|s| {
            Path::new(s.as_str())
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(s.as_str())
        })
        .collect();

    format!(
        "{prefix}: update {} ({} file(s) changed on {})",
        sample_files.join(", "),
        summary.total_changed_count(),
        summary.branch
    )
}

/// Evaluates whether a finished command in a background tab/window should trigger a desktop notification
/// (`src/onyxsh/terminal/desktop_notifier.py`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopNotificationPayload {
    pub title: String,
    pub body: String,
    pub is_error: bool,
}

const INTERACTIVE_TUI_BINARIES: &[&str] = &[
    "vim", "nvim", "nano", "htop", "btop", "top", "less", "more", "man", "tmux", "screen", "ssh",
];

pub fn evaluate_command_notification(
    command: &str,
    exit_code: i32,
    elapsed_secs: u64,
    threshold_secs: u64,
    terminal_is_focused: bool,
) -> Option<DesktopNotificationPayload> {
    if terminal_is_focused || elapsed_secs < threshold_secs.max(1) {
        return None;
    }
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return None;
    }
    let first_bin = trimmed
        .split_whitespace()
        .find(|tok| *tok != "sudo" && !tok.starts_with('-'))
        .unwrap_or("");
    let bin_name = Path::new(first_bin)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(first_bin);
    if INTERACTIVE_TUI_BINARIES.contains(&bin_name) {
        return None;
    }

    let is_error = exit_code != 0;
    let title = if is_error {
        tr_fmt(
            "❌ Comando Falhou (Exit {code})",
            &[("code", &exit_code.to_string())],
        )
    } else {
        tr("✅ Comando Concluído")
    };
    let body = tr_fmt(
        "`{cmd}` finalizou em {secs}s.",
        &[("cmd", trimmed), ("secs", &elapsed_secs.to_string())],
    );

    Some(DesktopNotificationPayload {
        title,
        body,
        is_error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_porcelain_parser_and_conventional_commit() {
        let sample = "\
## rewrite-rust...origin/rewrite-rust
M  crates/onyxsh-app/src/window.rs
 M crates/onyxsh-core/src/lib.rs
?? crates/onyxsh-core/src/backup.rs
";
        let status = parse_git_status_porcelain(sample);
        assert_eq!(status.branch, "rewrite-rust");
        assert_eq!(status.staged_files, vec!["crates/onyxsh-app/src/window.rs"]);
        assert_eq!(status.modified_files, vec!["crates/onyxsh-core/src/lib.rs"]);
        assert_eq!(
            status.untracked_files,
            vec!["crates/onyxsh-core/src/backup.rs"]
        );

        let msg = suggest_conventional_commit(&status);
        assert!(msg.starts_with("feat: update "));
        assert!(msg.contains("rewrite-rust"));
    }

    #[test]
    fn test_desktop_notifier_rules() {
        // Focused terminal never notifies
        assert!(evaluate_command_notification("cargo build", 0, 25, 10, true).is_none());
        // Interactive TUI (vim/htop) never notifies
        assert!(evaluate_command_notification("nvim src/main.rs", 0, 120, 10, false).is_none());
        // Long background build notifies
        let notif =
            evaluate_command_notification("cargo build --release", 0, 15, 10, false).unwrap();
        assert!(!notif.is_error);
        assert_eq!(
            notif.body,
            tr_fmt(
                "`{cmd}` finalizou em {secs}s.",
                &[("cmd", "cargo build --release"), ("secs", "15")],
            )
        );
    }
}
