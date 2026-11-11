//! Policy Engine for evaluating and enforcing execution safety in OnyxSH.
//! Port of `src/onyxsh/agent/policy_engine.py`.

use crate::models::{ActionStep, RiskLevel};
use onyxsh_core::i18n::tr;
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

const DENY_PATTERNS_RAW: &[(&str, &str)] = &[
    (
        r"(?i)rm\s+(-[a-zA-Z]*[rf][a-zA-Z]*\s+)+(/|/\*|~|\$HOME)(\s|$)",
        "remoção recursiva de raiz/home",
    ),
    (
        r"(?i)(^|\s)rm\s+(-[a-zA-Z]*[rf][a-zA-Z]*\s+)*(/|/\*|/boot|/etc|/usr|/var|/dev|/sys|/proc)(\s|$)",
        "remoção de diretórios críticos do sistema",
    ),
    (r"(?i)mkfs\.", "formatação de sistema de arquivos"),
    (
        r"(?i)dd\s+.*of=/dev/",
        "escrita direta em dispositivo de bloco",
    ),
    (
        r"(?i)(chmod|chown)\s+(-[a-zA-Z]*R[a-zA-Z]*\s+)?([a-zA-Z0-9_:]+|777|000|a\+rwx)\s+(/|~|\$HOME)(\s|$)",
        "alteração recursiva de permissões ou propriedade na raiz ou home",
    ),
    (
        r"(?i)(curl|wget)\b.*\|\s*(sudo\s+)?(ba)?sh",
        "download e execução arbitrária via pipe",
    ),
    (
        r"(?i)>\s*/etc/(passwd|shadow|sudoers|group|fstab)",
        "sobrescrita direta de arquivos críticos de autenticação e sistema",
    ),
    (
        r"(?i)(^|\s)(shutdown|reboot|poweroff|init\s+0|init\s+6|systemctl\s+(poweroff|reboot|halt))(\s|$)",
        "reinicialização ou desligamento do sistema",
    ),
    (
        r":\(\)\s*\{\s*:\s*\|\s*:\s*&\s*\}\s*;\s*:",
        "fork bomb",
    ),
    (
        r"(?i)(^|\s)(sh|bash|zsh|ksh)\s+-c(\s+|$)",
        "invocação aninhada de interpretador shell arbitrário",
    ),
];

const READ_ONLY_COMMANDS: &[&str] = &[
    "ls", "cat", "df", "du", "head", "tail", "grep", "rg", "find", "pwd", "whoami", "uname",
    "uptime", "free", "ps", "top", "htop", "ip", "ifconfig", "ping", "netstat", "ss", "date",
    "cal", "echo", "which", "whereis", "type", "file", "wc", "stat", "tree", "diff", "git status",
    "git log", "git diff", "git show", "git branch", "journalctl", "dmesg", "lsblk", "lscpu",
    "lspci", "lsusb",
];

struct CompiledPolicyRules {
    denies: Vec<(Regex, &'static str)>,
    read_only_set: HashSet<&'static str>,
}

static POLICY_RULES: OnceLock<CompiledPolicyRules> = OnceLock::new();

fn policy_rules() -> &'static CompiledPolicyRules {
    POLICY_RULES.get_or_init(|| {
        let denies = DENY_PATTERNS_RAW
            .iter()
            .map(|(pat, reason)| (Regex::new(pat).expect("valid deny regex"), *reason))
            .collect();
        let read_only_set = READ_ONLY_COMMANDS.iter().copied().collect();
        CompiledPolicyRules {
            denies,
            read_only_set,
        }
    })
}

#[derive(Debug, Default, Clone)]
pub struct PolicyEngine;

impl PolicyEngine {
    pub fn new() -> Self {
        Self
    }

    /// Checks if `argv` matches any forbidden pattern. Returns `(is_denied, Option<reason>)`.
    pub fn check_deny_patterns(&self, argv: &[String]) -> (bool, Option<String>) {
        if argv.is_empty() {
            return (false, None);
        }
        let cmd_string = argv.join(" ");
        let cmd_trimmed = cmd_string.trim();
        let rules = policy_rules();

        for (pattern, reason) in &rules.denies {
            if pattern.is_match(cmd_trimmed) {
                return (true, Some(tr(reason)));
            }
            for elem in argv {
                if pattern.is_match(elem) {
                    return (true, Some(tr(reason)));
                }
            }
        }
        (false, None)
    }

    /// Classifies the `RiskLevel` of a tool invocation and command argument list.
    pub fn classify(&self, argv: &[String], requires_admin: bool, tool: &str) -> RiskLevel {
        let (denied, _) = self.check_deny_patterns(argv);
        if denied {
            return RiskLevel::Blocked;
        }

        if let Some(first) = argv.first() {
            if matches!(first.as_str(), "sudo" | "su" | "pkexec" | "doas") {
                let sub = &argv[1..];
                let (sub_denied, _) = self.check_deny_patterns(sub);
                if sub_denied {
                    return RiskLevel::Blocked;
                }
                return RiskLevel::Admin;
            }
        }

        if tool.starts_with("fs.") {
            if matches!(
                tool,
                "fs.list_directory" | "fs.metadata" | "fs.read_file" | "fs.search_text" | "fs.disk_usage"
            ) {
                return RiskLevel::ReadOnly;
            }
            return RiskLevel::UserWrite;
        }

        if requires_admin || tool == "admin.run_action" {
            return RiskLevel::Admin;
        }

        if argv.is_empty() {
            return RiskLevel::ReadOnly;
        }

        let rules = policy_rules();
        let base_cmd = argv[0].as_str();
        let full_2cmd = if argv.len() > 1 {
            format!("{} {}", argv[0], argv[1])
        } else {
            base_cmd.to_string()
        };

        if rules.read_only_set.contains(full_2cmd.as_str()) || rules.read_only_set.contains(base_cmd)
        {
            return RiskLevel::ReadOnly;
        }

        RiskLevel::UserWrite
    }

    /// Evaluates and updates an `ActionStep` with its computed `RiskLevel`.
    pub fn evaluate_step(&self, mut step: ActionStep) -> ActionStep {
        step.risk_level = self.classify(&step.argv, step.requires_admin, &step.tool);
        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_engine_blocks_deny_patterns_and_classifies_risk() {
        let engine = PolicyEngine::new();

        let ro = vec!["git".to_string(), "status".to_string()];
        assert_eq!(engine.classify(&ro, false, "shell.run"), RiskLevel::ReadOnly);

        let uw = vec!["mkdir".to_string(), "-p".to_string(), "build".to_string()];
        assert_eq!(engine.classify(&uw, false, "shell.run"), RiskLevel::UserWrite);

        let adm = vec!["sudo".to_string(), "apt".to_string(), "update".to_string()];
        assert_eq!(engine.classify(&adm, false, "shell.run"), RiskLevel::Admin);

        let blocked = vec!["rm".to_string(), "-rf".to_string(), "/".to_string()];
        assert_eq!(engine.classify(&blocked, false, "shell.run"), RiskLevel::Blocked);
    }
}
