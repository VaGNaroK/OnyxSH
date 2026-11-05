//! Production Guard Engine for OnyxSH (Port of `src/onyxsh/terminal/production_guard.py`).
//! Analyzes terminal command lines in real time to detect potentially destructive
//! or high-risk actions executed against production environments.

use onyxsh_core::i18n::tr;
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardViolation {
    pub command: String,
    pub category: String,
    pub risk_summary: String,
    pub severity: String,
}

struct CompiledRuleSet {
    categories: Vec<(&'static str, &'static str, Vec<(Regex, &'static str)>)>,
    wrapper_regexes: Vec<Regex>,
    xargs_regex: Regex,
    split_regex: Regex,
    subshell_double_re: Regex,
    subshell_single_re: Regex,
    eval_double_re: Regex,
    eval_single_re: Regex,
    assign_double_re: Regex,
    assign_single_re: Regex,
}

static RULES: OnceLock<CompiledRuleSet> = OnceLock::new();

fn rules() -> &'static CompiledRuleSet {
    RULES.get_or_init(|| {
        let compile_list = |items: &[(&str, &'static str)]| -> Vec<(Regex, &'static str)> {
            items
                .iter()
                .map(|(pat, desc)| (Regex::new(pat).expect("valid guard regex"), *desc))
                .collect()
        };

        let file_delete = compile_list(&[
            (
                r"(?i)\brm\s+-[a-zA-Z]*r[a-zA-Z]*f|\brm\s+-[a-zA-Z]*f[a-zA-Z]*r|\brm\s+.*--recursive.*--force|\brm\s+.*--force.*--recursive",
                "Recursive force deletion (rm -rf)",
            ),
            (
                r"(?i)\brm\s+-[a-zA-Z]*r[a-zA-Z]*|\brm\s+--recursive",
                "Recursive directory deletion (rm -r)",
            ),
            (
                r"(?i)\bxargs\s+.*\brm\b",
                "Batch deletion via xargs (xargs rm)",
            ),
            (
                r"(?i)\bshred\s+-[a-zA-Z]*u|\bwipefs\b",
                "Permanent file or filesystem wipe (shred/wipefs)",
            ),
        ]);

        let disk = compile_list(&[
            (
                r"(?i)\bmkfs(?:\.[a-zA-Z0-9_-]+)?\b",
                "Filesystem creation / format (mkfs)",
            ),
            (
                r"(?i)\bdd\s+.*(?:of=/dev/|if=/dev/zero|if=/dev/urandom)",
                "Raw disk block overwrite (dd of=/dev/...)",
            ),
            (
                r"(?i)\b(?:fdisk|gdisk|parted|sfdisk)\s+/dev/",
                "Partition table modification (fdisk/parted)",
            ),
        ]);

        let power = compile_list(&[
            (
                r"(?i)\b(?:shutdown|reboot|poweroff|halt)\b",
                "Host shutdown or reboot operation",
            ),
            (
                r"\b(?:init|telinit)\s+[06]\b",
                "Runlevel switch to halt or reboot (init 0/6)",
            ),
        ]);

        let service = compile_list(&[
            (
                r"(?i)\bsystemctl\s+(?:stop|disable|mask|isolate)\b",
                "Critical systemd service stop or disable",
            ),
            (
                r"(?i)\bservice\s+[\w.-]+\s+(?:stop|restart|reload)\b",
                "System service stoppage or restart",
            ),
        ]);

        let git = compile_list(&[
            (
                r"(?i)\bgit\s+reset\s+--hard\b",
                "Hard git reset discarding uncommitted changes",
            ),
            (
                r"(?i)\bgit\s+clean\s+-[a-zA-Z]*[fdx]",
                "Untracked files deletion (git clean)",
            ),
            (
                r"(?i)\bgit\s+push\s+.*(?:--force|-f\b|\+)",
                "Force push overwriting remote Git history",
            ),
        ]);

        let db = compile_list(&[(
            r"(?i)\b(?:DROP|TRUNCATE)\s+(?:DATABASE|SCHEMA|TABLE)\b",
            "Database drop or truncate operation",
        )]);

        let evasion = compile_list(&[
            (
                r"(?i)\|\s*(?:sudo\s+|doas\s+|pkexec\s+)?(?:bash|sh|zsh|dash|ksh)\b",
                "Piped execution to shell interpreter",
            ),
            (
                r"(?i)\bbase64\s+(?:-[a-zA-Z]*d|--decode)\s*\|",
                "Obfuscated / Base64 decoded execution pipeline",
            ),
        ]);

        let wrappers = [
            "sudo", "doas", "pkexec", "nohup", "env", "time", "exec", "builtin", "command",
        ];
        let wrapper_regexes = wrappers
            .iter()
            .map(|w| Regex::new(&format!(r"(?i)^{w}\s+(-[a-zA-Z0-9_\-]+\s+)*")).unwrap())
            .collect();

        CompiledRuleSet {
            categories: vec![
                ("File System", "critical", file_delete),
                ("Storage & Partitions", "critical", disk),
                ("System Power", "critical", power),
                ("Services & Daemons", "high", service),
                ("Version Control", "high", git),
                ("Database", "critical", db),
                ("Script Execution", "critical", evasion),
            ],
            wrapper_regexes,
            xargs_regex: Regex::new(
                r"(?i)^xargs\s+(?:-[InsdEPL]\s*\S*\s+|-[a-zA-Z0-9_]+\s+|--[a-zA-Z0-9_\-]+(?:=\S+)?\s+)*",
            )
            .unwrap(),
            split_regex: Regex::new(r"[\n;]|&&|\|\||\|").unwrap(),
            subshell_double_re: Regex::new(
                r#"(?i)\b(?:bash|sh|zsh|dash|ksh)\s+(?:-[a-zA-Z0-9_\-]+\s+)*-c\s+"([\s\S]*?)""#,
            )
            .unwrap(),
            subshell_single_re: Regex::new(
                r"(?i)\b(?:bash|sh|zsh|dash|ksh)\s+(?:-[a-zA-Z0-9_\-]+\s+)*-c\s+'([\s\S]*?)'",
            )
            .unwrap(),
            eval_double_re: Regex::new(r#"(?i)\beval\s+"([\s\S]*?)""#).unwrap(),
            eval_single_re: Regex::new(r"(?i)\beval\s+'([\s\S]*?)'").unwrap(),
            assign_double_re: Regex::new(r#"(?:^|\s)[a-zA-Z_][a-zA-Z0-9_]*\s*=\s*"([\s\S]*?)""#)
                .unwrap(),
            assign_single_re: Regex::new(r"(?:^|\s)[a-zA-Z_][a-zA-Z0-9_]*\s*=\s*'([\s\S]*?)'")
                .unwrap(),
        }
    })
}

#[derive(Debug, Default, Clone)]
pub struct ProductionGuard;

impl ProductionGuard {
    pub fn new() -> Self {
        Self
    }

    fn strip_wrapper_commands(&self, cmd_line: &str) -> String {
        let r = rules();
        let mut stripped = cmd_line.trim().to_string();
        let mut changed = true;

        while changed {
            changed = false;
            for wre in &r.wrapper_regexes {
                if let Some(m) = wre.find(&stripped) {
                    stripped = stripped[m.end()..].trim().to_string();
                    changed = true;
                    break;
                }
            }
            if let Some(m) = r.xargs_regex.find(&stripped) {
                stripped = stripped[m.end()..].trim().to_string();
                changed = true;
            }
        }
        stripped
    }

    fn split_subcommands(&self, cmd_line: &str) -> Vec<String> {
        rules()
            .split_regex
            .split(cmd_line)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    fn extract_subshell_and_eval_commands(&self, cmd_line: &str) -> Vec<String> {
        let r = rules();
        let mut extracted = Vec::new();
        for re in [
            &r.subshell_double_re,
            &r.subshell_single_re,
            &r.eval_double_re,
            &r.eval_single_re,
            &r.assign_double_re,
            &r.assign_single_re,
        ] {
            for cap in re.captures_iter(cmd_line) {
                if let Some(m) = cap.get(1) {
                    let inner = m.as_str().trim();
                    if !inner.is_empty() {
                        extracted.push(inner.to_string());
                    }
                }
            }
        }
        extracted
    }

    fn collect_candidates(&self, raw_command: &str) -> Vec<String> {
        let clean_cmd = raw_command.trim();
        if clean_cmd.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::new();
        let mut to_process = vec![clean_cmd.to_string()];
        let mut seen = HashSet::new();

        let mut depth = 0;
        while !to_process.is_empty() && depth < 5 {
            depth += 1;
            let current_batch = std::mem::take(&mut to_process);

            for expr in current_batch {
                let expr_clean = expr.trim().to_string();
                if expr_clean.is_empty() || seen.contains(&expr_clean) {
                    continue;
                }
                seen.insert(expr_clean.clone());
                candidates.push(expr_clean.clone());

                let stripped = self.strip_wrapper_commands(&expr_clean);
                if !stripped.is_empty() && !seen.contains(&stripped) {
                    to_process.push(stripped);
                }

                for subcmd in self.split_subcommands(&expr_clean) {
                    if !seen.contains(&subcmd) {
                        to_process.push(subcmd);
                    }
                }

                for inner in self.extract_subshell_and_eval_commands(&expr_clean) {
                    if !seen.contains(&inner) {
                        to_process.push(inner);
                    }
                }
            }
        }

        candidates
    }

    /// Evaluates a shell command string to determine if it is a destructive action in production.
    pub fn evaluate_command(&self, raw_command: &str) -> Option<GuardViolation> {
        let clean_cmd = raw_command.trim();
        if clean_cmd.is_empty() {
            return None;
        }

        let candidates = self.collect_candidates(clean_cmd);
        let r = rules();

        for (category, severity, pattern_list) in &r.categories {
            for (pattern, description) in pattern_list {
                for candidate in &candidates {
                    if pattern.is_match(candidate) {
                        return Some(GuardViolation {
                            command: clean_cmd.to_string(),
                            category: tr(category),
                            risk_summary: tr(description),
                            severity: (*severity).to_string(),
                        });
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_commands_pass() {
        let guard = ProductionGuard::new();
        assert!(guard.evaluate_command("ls -la /var/log").is_none());
        assert!(guard.evaluate_command("git status && git diff").is_none());
        assert!(guard.evaluate_command("systemctl status nginx").is_none());
    }

    #[test]
    fn test_destructive_rm_and_disk_commands_blocked() {
        let guard = ProductionGuard::new();
        assert!(guard.evaluate_command("sudo rm -rf /var/log/*").is_some());
        assert!(guard.evaluate_command("mkfs.ext4 /dev/nvme0n1p1").is_some());
        assert!(guard.evaluate_command("dd if=/dev/zero of=/dev/sda bs=1M").is_some());
    }

    #[test]
    fn test_subshell_eval_xargs_and_variable_bypass_blocked() {
        let guard = ProductionGuard::new();
        assert!(guard.evaluate_command(r#"sudo bash -c "rm -rf /data""#).is_some());
        assert!(guard.evaluate_command("eval 'shutdown -h now'").is_some());
        assert!(guard.evaluate_command("CMD=\"rm -rf /\" ; $CMD").is_some());
        assert!(guard.evaluate_command("find /tmp -type f | sudo xargs -0 rm -f").is_some());
        assert!(guard.evaluate_command("echo 'cm0gLXJmIC8=' | base64 -d | bash").is_some());
        assert!(guard.evaluate_command("DROP DATABASE production_db;").is_some());
    }
}
