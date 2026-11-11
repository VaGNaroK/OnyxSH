//! Post-Execution Sanity Verifier for the Zero-Trust Agent.
//! Port of `src/onyxsh/agent/verifier.py`.

use onyxsh_core::i18n::{tr, tr_fmt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub target_command: String,
    pub check_command: String,
    pub check_type: String,
    pub description: String,
    pub expected_exit_code: i32,
    pub failure_diagnostic_command: Option<String>,
}

struct VerifierRegexes {
    sudo_prefix: Regex,
    systemctl_re: Regex,
}

static VERIFIER_RE: OnceLock<VerifierRegexes> = OnceLock::new();

fn verifier_regexes() -> &'static VerifierRegexes {
    VERIFIER_RE.get_or_init(|| VerifierRegexes {
        sudo_prefix: Regex::new(r"^(?:sudo|pkexec|doas)\s+(?:-[a-zA-Z]+\s+)*").unwrap(),
        systemctl_re: Regex::new(
            r"^systemctl\s+(start|restart|reload|try-restart|enable|disable|stop)\s+([a-zA-Z0-9@_\.\-]+)",
        )
        .unwrap(),
    })
}

/// Infers automated post-execution sanity checks from a list of executed commands.
pub fn infer_verifications(commands: &[String]) -> Vec<VerificationCheck> {
    let mut checks = Vec::new();
    let mut seen = HashSet::new();
    let re = verifier_regexes();

    for raw_cmd in commands {
        let trimmed = raw_cmd.trim();
        if trimmed.is_empty() {
            continue;
        }
        let clean_cmd = re.sudo_prefix.replace(trimmed, "").trim().to_string();

        // 1. Systemd service lifecycle checks
        if let Some(caps) = re.systemctl_re.captures(&clean_cmd) {
            let action = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let raw_svc = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
            let svc = if raw_svc.ends_with(".service")
                || raw_svc.ends_with(".socket")
                || raw_svc.ends_with(".timer")
            {
                raw_svc.to_string()
            } else {
                format!("{raw_svc}.service")
            };

            let check = match action {
                "start" | "restart" | "reload" | "try-restart" => Some(VerificationCheck {
                    target_command: trimmed.to_string(),
                    check_command: format!("systemctl is-active {svc}"),
                    check_type: "service_active".to_string(),
                    description: tr_fmt(
                        "Verificar se o serviço '{svc}' está ativo",
                        &[("svc", &svc)],
                    ),
                    expected_exit_code: 0,
                    failure_diagnostic_command: Some(format!(
                        "journalctl -u {svc} -n 25 --no-pager"
                    )),
                }),
                "enable" => Some(VerificationCheck {
                    target_command: trimmed.to_string(),
                    check_command: format!("systemctl is-enabled {svc}"),
                    check_type: "service_enabled".to_string(),
                    description: tr_fmt(
                        "Verificar se o serviço '{svc}' foi habilitado no boot",
                        &[("svc", &svc)],
                    ),
                    expected_exit_code: 0,
                    failure_diagnostic_command: Some(format!(
                        "systemctl status {svc} --no-pager"
                    )),
                }),
                "stop" | "disable" => Some(VerificationCheck {
                    target_command: trimmed.to_string(),
                    check_command: format!("systemctl is-active {svc}"),
                    check_type: "service_stopped".to_string(),
                    description: tr_fmt(
                        "Verificar se o serviço '{svc}' foi parado",
                        &[("svc", &svc)],
                    ),
                    expected_exit_code: 3,
                    failure_diagnostic_command: Some(format!(
                        "systemctl status {svc} --no-pager"
                    )),
                }),
                _ => None,
            };

            if let Some(chk) = check {
                let key = format!("{}:{}", chk.check_type, chk.check_command);
                if seen.insert(key) {
                    checks.push(chk);
                }
            }
            continue;
        }

        // 2. Nginx configuration check
        if clean_cmd.contains("nginx") && clean_cmd.contains("/etc/nginx") {
            let chk = VerificationCheck {
                target_command: trimmed.to_string(),
                check_command: "nginx -t".to_string(),
                check_type: "syntax_test".to_string(),
                description: tr("Testar integridade da sintaxe de configuração do Nginx"),
                expected_exit_code: 0,
                failure_diagnostic_command: Some("nginx -t".to_string()),
            };
            let key = format!("{}:{}", chk.check_type, chk.check_command);
            if seen.insert(key) {
                checks.push(chk);
            }
        }
    }

    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_verifications_for_systemd_and_nginx() {
        let cmds = vec![
            "sudo systemctl restart nginx".to_string(),
            "sudo systemctl stop redis".to_string(),
            "cp nginx.conf /etc/nginx/nginx.conf".to_string(),
        ];
        let checks = infer_verifications(&cmds);
        assert_eq!(checks.len(), 3);
        assert_eq!(checks[0].check_command, "systemctl is-active nginx.service");
        assert_eq!(checks[0].expected_exit_code, 0);
        assert_eq!(checks[1].check_command, "systemctl is-active redis.service");
        assert_eq!(checks[1].expected_exit_code, 3);
        assert_eq!(checks[2].check_command, "nginx -t");
    }
}
