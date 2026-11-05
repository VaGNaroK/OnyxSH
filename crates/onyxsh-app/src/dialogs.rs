//! Relm4 Modal Dialog State Models and Validators
//! (`src/onyxsh/ui/quick_connect_dialog.py`, `session_dialog.py`, `tunnels_dialog.py`,
//! `snippet_dialog.py`, `preferences.py`).

use onyxsh_core::security::{is_valid_host, is_valid_port, is_valid_username};
use onyxsh_core::snippets::{extract_variables, resolve_template, SnippetVariable};
use onyxsh_terminal::tunnel_manager::SshTunnel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Quick Connect dialog state (`user@host:port` parser + validator).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickConnectDialogState {
    pub raw_target: String,
    pub protocol: String,
    pub parsed_user: Option<String>,
    pub parsed_host: String,
    pub parsed_port: u16,
}

impl QuickConnectDialogState {
    /// Parses `[user@]host[:port]` and validates according to RFC-1123 and SSH username rules.
    pub fn parse_and_validate(target: &str, default_port: u16) -> Result<Self, &'static str> {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return Err("Host cannot be empty");
        }

        let (user_part, host_port_part) = match trimmed.rsplit_once('@') {
            Some((u, hp)) => (Some(u.trim()), hp.trim()),
            None => (None, trimmed),
        };

        if let Some(u) = user_part {
            if !is_valid_username(u) {
                return Err("Invalid SSH username");
            }
        }

        let (host_str, port) = match host_port_part.rsplit_once(':') {
            Some((h, p_str)) if !h.contains(':') => {
                let p = p_str.parse::<u16>().map_err(|_| "Invalid port number")?;
                if !is_valid_port(p as u32) {
                    return Err("Port out of valid range (1..=65535)");
                }
                (h.trim(), p)
            }
            _ => (host_port_part, default_port),
        };

        if !is_valid_host(host_str) {
            return Err("Invalid hostname or IP address");
        }

        Ok(Self {
            raw_target: trimmed.to_string(),
            protocol: "ssh".to_string(),
            parsed_user: user_part.map(ToOwned::to_owned),
            parsed_host: host_str.to_string(),
            parsed_port: port,
        })
    }
}

/// Snippet Variable Prompt Dialog State (`src/onyxsh/ui/snippet_variable_dialog.py`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnippetVariableDialogState {
    pub template: String,
    pub variables: Vec<SnippetVariable>,
    pub values: HashMap<String, String>,
}

impl SnippetVariableDialogState {
    pub fn from_template(template: &str) -> Self {
        let vars = extract_variables(template);
        let mut values = HashMap::new();
        for v in &vars {
            values.insert(v.name.clone(), v.default_value.clone());
        }
        Self {
            template: template.to_string(),
            variables: vars,
            values,
        }
    }

    pub fn set_value(&mut self, key: &str, val: impl Into<String>) {
        self.values.insert(key.to_string(), val.into());
    }

    pub fn render(&self) -> String {
        resolve_template(&self.template, &self.values, &HashMap::new())
    }
}

/// SSH Tunnel Builder Dialog State (`src/onyxsh/ui/tunnels_dialog.py`).
#[derive(Debug, Clone)]
pub struct TunnelDialogState {
    pub name: String,
    pub session_name: String,
    pub tunnel_type: String,
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

impl TunnelDialogState {
    pub fn build_tunnel(&self, id: &str) -> Result<SshTunnel, &'static str> {
        if !is_valid_host(&self.remote_host) {
            return Err("Invalid remote host");
        }
        if !is_valid_port(self.local_port as u32) {
            return Err("Invalid local port");
        }
        Ok(SshTunnel {
            id: id.to_string(),
            name: self.name.clone(),
            session_name: self.session_name.clone(),
            tunnel_type: self.tunnel_type.clone(),
            local_host: self.local_host.clone(),
            local_port: self.local_port,
            remote_host: self.remote_host.clone(),
            remote_port: self.remote_port,
            auto_start: false,
            status: "stopped".to_string(),
            error_message: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_connect_dialog_parse_and_validate() {
        let qc = QuickConnectDialogState::parse_and_validate("admin@10.0.0.25:2222", 22).unwrap();
        assert_eq!(qc.parsed_user.as_deref(), Some("admin"));
        assert_eq!(qc.parsed_host, "10.0.0.25");
        assert_eq!(qc.parsed_port, 2222);

        assert!(QuickConnectDialogState::parse_and_validate("bad_user;id@10.0.0.1", 22).is_err());
    }

    #[test]
    fn test_snippet_variable_dialog_state() {
        let mut dlg = SnippetVariableDialogState::from_template(
            "kubectl logs -n {{namespace=default}} {{pod}}",
        );
        dlg.set_value("pod", "api-server-7f8b");
        assert_eq!(dlg.render(), "kubectl logs -n default api-server-7f8b");
    }
}
