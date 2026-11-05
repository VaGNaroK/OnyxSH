//! Terminal Spawner, Shell Integration Snippet Generator (OSC 133 & OSC 7), and ProcessTracker.
//! Port of `src/onyxsh/terminal/spawner.py`.

use std::collections::HashMap;
use std::path::PathBuf;

/// Generates the Bash integration script emitting OSC 133 (A/B/C/D) and OSC 7 directory hooks.
pub fn bash_integration_script() -> &'static str {
    r#"# OnyxSH Bash Integration (OSC 133 & OSC 7)
__onyxsh_osc7() {
    local host="${HOSTNAME:-$(hostname 2>/dev/null || uname -n)}"
    printf '\033]7;file://%s%s\007' "$host" "$PWD"
}
__onyxsh_prompt_cmd() {
    local exit_code="$?"
    printf '\033]133;D;%s\007' "$exit_code"
    __onyxsh_osc7
    printf '\033]133;A\007'
}
PROMPT_COMMAND="__onyxsh_prompt_cmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
PS0='\[\033]133;C\007\]'
"#
}

/// Generates the Zsh integration script emitting OSC 133 (A/B/C/D) and OSC 7 directory hooks.
pub fn zsh_integration_script() -> &'static str {
    r#"# OnyxSH Zsh Integration (OSC 133 & OSC 7)
__onyxsh_precmd() {
    local exit_code="$?"
    printf '\033]133;D;%s\007' "$exit_code"
    printf '\033]7;file://%s%s\007' "${HOST:-localhost}" "$PWD"
    printf '\033]133;A\007'
}
__onyxsh_preexec() {
    printf '\033]133;C\007'
}
precmd_functions+=(__onyxsh_precmd)
preexec_functions+=(__onyxsh_preexec)
"#
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedProcess {
    pub pid: i32,
    pub command_line: String,
    pub temp_dir: Option<PathBuf>,
}

/// Tracks active child PTY processes and their temporary shell integration directories.
#[derive(Debug, Default)]
pub struct ProcessTracker {
    processes: HashMap<i32, TrackedProcess>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, pid: i32, command_line: &str, temp_dir: Option<PathBuf>) {
        self.processes.insert(
            pid,
            TrackedProcess {
                pid,
                command_line: command_line.to_string(),
                temp_dir,
            },
        );
    }

    pub fn unregister(&mut self, pid: i32) -> Option<TrackedProcess> {
        let removed = self.processes.remove(&pid);
        if let Some(ref proc_info) = removed {
            if let Some(ref dir) = proc_info.temp_dir {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
        removed
    }

    pub fn active_count(&self) -> usize {
        self.processes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_integration_scripts_contain_osc133_and_osc7() {
        let bash = bash_integration_script();
        assert!(bash.contains("133;D;"));
        assert!(bash.contains("133;A"));
        assert!(bash.contains("133;C"));
        assert!(bash.contains("]7;file://"));

        let zsh = zsh_integration_script();
        assert!(zsh.contains("__onyxsh_precmd"));
        assert!(zsh.contains("__onyxsh_preexec"));
    }

    #[test]
    fn test_process_tracker_lifecycle() {
        let mut tracker = ProcessTracker::new();
        tracker.register(20001, "/bin/bash", None);
        assert_eq!(tracker.active_count(), 1);
        let removed = tracker.unregister(20001);
        assert!(removed.is_some());
        assert_eq!(tracker.active_count(), 0);
    }
}
