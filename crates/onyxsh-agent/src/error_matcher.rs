//! Heuristic Error Pattern Matcher and Zero-Latency Quick-Fix Engine.
//! Port of `src/onyxsh/agent/error_matcher.py`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    PermissionDenied,
    CommandNotFound,
    PortInUse,
    NoSpaceLeft,
    FileNotFound,
    ConnectionRefused,
    PackageMissing,
    GitError,
    GenericError,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorMatch {
    pub category: ErrorCategory,
    pub title: String,
    pub description: String,
    pub quick_fix_command: Option<String>,
    pub quick_fix_label: Option<String>,
    pub extracted_target: Option<String>,
    pub exit_code: i32,
    pub command: String,
}

impl ErrorMatch {
    pub fn has_quick_fix(&self) -> bool {
        self.quick_fix_command
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    }
}

struct CompiledErrorRegexes {
    perm_re: Regex,
    cnf_re: Regex,
    port_re: Regex,
    space_re: Regex,
    git_re: Regex,
}

static ERROR_REGEXES: OnceLock<CompiledErrorRegexes> = OnceLock::new();

fn error_regexes() -> &'static CompiledErrorRegexes {
    ERROR_REGEXES.get_or_init(|| CompiledErrorRegexes {
        perm_re: Regex::new(
            r"(?i)\b(permission\s+denied|permiss[ãa]o\s+negada|operation\s+not\s+permitted|eacces|eperm|are\s+you\s+root|must\s+be\s+run\s+as\s+root)\b",
        )
        .unwrap(),
        cnf_re: Regex::new(
            r"(?i)\b(command\s+not\s+found|comando\s+n[ãa]o\s+encontrado)\b",
        )
        .unwrap(),
        port_re: Regex::new(
            r"(?i)(?:address\s+already\s+in\s+use|eaddrinuse|port\s+(\d+)\s+is\s+already\s+allocated|:(\d+):\s+bind:\s+address\s+already\s+in\s+use)",
        )
        .unwrap(),
        space_re: Regex::new(
            r"(?i)\b(no\s+space\s+left\s+on\s+device|enospc|sem\s+espa[çc]o\s+dispon[íi]vel)\b",
        )
        .unwrap(),
        git_re: Regex::new(
            r"(?i)\b(not\s+a\s+git\s+repository|merge\s+conflict|failed\s+to\s+push\s+some\s+refs)\b",
        )
        .unwrap(),
    })
}

/// Evaluates an executed command, its exit code, and output text to suggest 1-click fixes.
pub fn match_terminal_error(
    command: &str,
    exit_code: i32,
    output: &str,
    package_manager: &str,
) -> Option<ErrorMatch> {
    if exit_code == 0 {
        return None;
    }

    let cmd = command.trim();
    let out = output.trim();
    let combined = format!("{cmd}\n{out}");
    let re = error_regexes();

    // 1. Permission Denied
    let has_sudo = cmd.starts_with("sudo ") || cmd.starts_with("pkexec ") || cmd.starts_with("doas ");
    if re.perm_re.is_match(&combined) {
        let (qf_cmd, qf_label) = if !has_sudo && !cmd.is_empty() {
            (
                Some(format!("sudo {cmd}")),
                Some("⚡ Executar com sudo".to_string()),
            )
        } else {
            (None, Some("🤖 Diagnosticar com IA".to_string()))
        };
        return Some(ErrorMatch {
            category: ErrorCategory::PermissionDenied,
            title: "Permissão Negada".to_string(),
            description: "O comando falhou por falta de privilégios de superusuário (root).".to_string(),
            quick_fix_command: qf_cmd,
            quick_fix_label: qf_label,
            extracted_target: None,
            exit_code,
            command: cmd.to_string(),
        });
    }

    // 2. Command Not Found (exit 127)
    if exit_code == 127 || re.cnf_re.is_match(&combined) {
        let missing_bin = cmd
            .split_whitespace()
            .find(|tok| !tok.contains('='))
            .unwrap_or("pacote");
        let install_cmd = match package_manager {
            "pacman" => format!("sudo pacman -S {missing_bin}"),
            "dnf" => format!("sudo dnf install -y {missing_bin}"),
            _ => format!("sudo apt install -y {missing_bin}"),
        };
        return Some(ErrorMatch {
            category: ErrorCategory::CommandNotFound,
            title: "Comando Não Encontrado".to_string(),
            description: format!("O binário '{missing_bin}' não está instalado ou fora do $PATH."),
            quick_fix_command: Some(install_cmd),
            quick_fix_label: Some(format!("📦 Instalar {missing_bin}")),
            extracted_target: Some(missing_bin.to_string()),
            exit_code,
            command: cmd.to_string(),
        });
    }

    // 3. Port Already in Use
    if let Some(caps) = re.port_re.captures(&combined) {
        let port = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or("8080");
        return Some(ErrorMatch {
            category: ErrorCategory::PortInUse,
            title: "Porta Já em Uso".to_string(),
            description: format!("A porta {port} já está alocada por outro processo."),
            quick_fix_command: Some(format!("sudo ss -tulpn | grep :{port}")),
            quick_fix_label: Some(format!("🔍 Inspecionar porta {port}")),
            extracted_target: Some(port.to_string()),
            exit_code,
            command: cmd.to_string(),
        });
    }

    // 4. No Space Left on Device
    if re.space_re.is_match(&combined) {
        return Some(ErrorMatch {
            category: ErrorCategory::NoSpaceLeft,
            title: "Disco Cheio (ENOSPC)".to_string(),
            description: "Não há espaço livre suficiente no sistema de arquivos.".to_string(),
            quick_fix_command: Some("df -h && du -sh * 2>/dev/null | sort -hr | head -n 10".to_string()),
            quick_fix_label: Some("📊 Analisar uso de disco".to_string()),
            extracted_target: None,
            exit_code,
            command: cmd.to_string(),
        });
    }

    // 5. Git Errors
    if re.git_re.is_match(&combined) {
        return Some(ErrorMatch {
            category: ErrorCategory::GitError,
            title: "Erro de Repositório Git".to_string(),
            description: "Conflito ou falha de sincronização no repositório Git.".to_string(),
            quick_fix_command: Some("git status".to_string()),
            quick_fix_label: Some("🔍 Verificar git status".to_string()),
            extracted_target: None,
            exit_code,
            command: cmd.to_string(),
        });
    }

    Some(ErrorMatch {
        category: ErrorCategory::GenericError,
        title: format!("Comando falhou (código {exit_code})"),
        description: "O comando encerrou com código de erro.".to_string(),
        quick_fix_command: None,
        quick_fix_label: Some("🤖 Diagnosticar com IA".to_string()),
        extracted_target: None,
        exit_code,
        command: cmd.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_matcher_permission_command_not_found_and_port() {
        assert!(match_terminal_error("ls", 0, "", "apt").is_none());

        let perm = match_terminal_error("apt update", 100, "E: Permission denied", "apt").unwrap();
        assert_eq!(perm.category, ErrorCategory::PermissionDenied);
        assert_eq!(perm.quick_fix_command.as_deref(), Some("sudo apt update"));

        let cnf = match_terminal_error("htop", 127, "bash: htop: command not found", "apt").unwrap();
        assert_eq!(cnf.category, ErrorCategory::CommandNotFound);
        assert_eq!(
            cnf.quick_fix_command.as_deref(),
            Some("sudo apt install -y htop")
        );

        let port = match_terminal_error(
            "python3 -m http.server 8000",
            1,
            "OSError: [Errno 98] Address already in use: :8000: bind: address already in use",
            "apt",
        )
        .unwrap();
        assert_eq!(port.category, ErrorCategory::PortInUse);
    }
}
