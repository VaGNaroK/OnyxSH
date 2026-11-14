//! Session models (`SessionItem`, `SessionFolder`, `PortForwarding`) and JSON storage manager.
//! Port of `src/onyxsh/sessions/models.py` and `src/onyxsh/sessions/storage.py`.

use crate::crypto::{decrypt_session_password, encrypt_session_password, is_securecrt_v2_password};
use crate::i18n::{tr, tr_fmt};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;

const MAX_SESSIONS_FILE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PortForwarding {
    #[serde(rename = "type")]
    pub tunnel_type: String,
    pub name: String,
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub auto_start: bool,
}

impl Default for PortForwarding {
    fn default() -> Self {
        Self {
            tunnel_type: "local".to_string(),
            name: "Tunnel".to_string(),
            local_host: "localhost".to_string(),
            local_port: 0,
            remote_host: String::new(),
            remote_port: 0,
            auto_start: true,
        }
    }
}

/// Data model for a terminal session (local or SSH), 100% JSON-compatible with `~/.config/onyxsh/sessions.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionItem {
    pub name: String,
    pub session_type: String,
    pub host: String,
    pub user: String,
    pub auth_type: String,
    pub auth_value: String,
    pub folder_path: String,
    pub port: u16,
    pub tab_color: Option<String>,
    pub post_login_command_enabled: bool,
    pub post_login_command: String,
    pub sftp_session_enabled: bool,
    pub sftp_local_directory: String,
    pub sftp_remote_directory: String,
    pub port_forwardings: Vec<PortForwarding>,
    pub x11_forwarding: bool,
    pub ssh_connection_mode: String,
    pub local_working_directory: String,
    pub local_startup_command: String,
    pub is_production: bool,
    pub output_highlighting: Option<bool>,
    pub command_specific_highlighting: Option<bool>,
    pub cat_colorization: Option<bool>,
    pub shell_input_highlighting: Option<bool>,
    pub created_at: f64,
    pub modified_at: f64,
    pub source: String,
}

impl Default for SessionItem {
    fn default() -> Self {
        Self {
            name: "Unnamed Session".to_string(),
            session_type: "ssh".to_string(),
            host: String::new(),
            user: String::new(),
            auth_type: "key".to_string(),
            auth_value: String::new(),
            folder_path: String::new(),
            port: 22,
            tab_color: None,
            post_login_command_enabled: false,
            post_login_command: String::new(),
            sftp_session_enabled: false,
            sftp_local_directory: String::new(),
            sftp_remote_directory: String::new(),
            port_forwardings: Vec::new(),
            x11_forwarding: false,
            ssh_connection_mode: "shell".to_string(),
            local_working_directory: String::new(),
            local_startup_command: String::new(),
            is_production: false,
            output_highlighting: None,
            command_specific_highlighting: None,
            cat_colorization: None,
            shell_input_highlighting: None,
            created_at: 0.0,
            modified_at: 0.0,
            source: "user".to_string(),
        }
    }
}

impl SessionItem {
    pub fn is_local(&self) -> bool {
        self.session_type == "local"
    }

    pub fn is_ssh(&self) -> bool {
        self.session_type == "ssh"
    }

    pub fn uses_key_auth(&self) -> bool {
        self.is_ssh() && self.auth_type == "key"
    }

    pub fn uses_password_auth(&self) -> bool {
        self.is_ssh() && self.auth_type == "password"
    }

    /// Returns `port` or defaults to `22` when unset (`0`).
    pub fn effective_port(&self) -> u16 {
        if self.port == 0 {
            22
        } else {
            self.port
        }
    }

    pub fn connection_string(&self) -> String {
        if self.is_local() {
            return tr("Local Terminal");
        }
        if self.user.is_empty() {
            self.host.clone()
        } else {
            format!("{}@{}", self.user, self.host)
        }
    }

    /// Sets the session password, encrypting it in SecureCRT V2 format (`02:<hex>`).
    pub fn set_password(&mut self, plaintext: &str) {
        if plaintext.is_empty() {
            self.auth_value.clear();
        } else if let Ok(enc) = encrypt_session_password(plaintext, "") {
            self.auth_value = enc;
        }
    }

    /// Resolves the plaintext password (decrypting `02:<hex>` if encrypted) or returns the key path.
    pub fn resolved_auth_value(&self) -> String {
        if self.uses_password_auth() && is_securecrt_v2_password(&self.auth_value) {
            return decrypt_session_password(&self.auth_value, "").unwrap_or_default();
        }
        self.auth_value.clone()
    }

    /// Validates the session configuration and returns any error messages.
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push(tr("Session name is required."));
        }
        if self.is_ssh() {
            if self.host.trim().is_empty() {
                errors.push(tr("Host is required for SSH sessions."));
            }
            if self.port == 0 {
                errors.push(tr("Port must be between 1 and 65535."));
            }
            if self.post_login_command_enabled && self.post_login_command.trim().is_empty() {
                errors.push(tr("Post-login command cannot be empty when enabled."));
            }
            for tunnel in &self.port_forwardings {
                if tunnel.local_port <= 1024 {
                    errors.push(tr_fmt(
                        "Port forward '{name}' has an invalid local port (must be between 1025 and 65535).",
                        &[("name", &tunnel.name)],
                    ));
                }
                if tunnel.tunnel_type != "dynamic" {
                    if tunnel.remote_port == 0 {
                        errors.push(tr_fmt(
                            "Port forward '{name}' has an invalid remote port.",
                            &[("name", &tunnel.name)],
                        ));
                    }
                    if tunnel.remote_host.trim().is_empty() {
                        errors.push(tr_fmt(
                            "Port forward '{name}' requires a remote host.",
                            &[("name", &tunnel.name)],
                        ));
                    }
                }
            }
        }
        errors
    }

    pub fn validate(&self) -> bool {
        self.validation_errors().is_empty()
    }
}

/// Data model for a session folder in the sidebar tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionFolder {
    pub name: String,
    pub path: String,
    pub parent_path: String,
    pub is_production: bool,
    pub created_at: f64,
    pub modified_at: f64,
}

impl Default for SessionFolder {
    fn default() -> Self {
        Self {
            name: "Unnamed Folder".to_string(),
            path: String::new(),
            parent_path: String::new(),
            is_production: false,
            created_at: 0.0,
            modified_at: 0.0,
        }
    }
}

impl SessionFolder {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.name.trim().is_empty() {
            errors.push(tr("Folder name is required."));
        }
        if !self.path.is_empty()
            && !self.parent_path.is_empty()
            && !self.path.starts_with(&format!("{}/", self.parent_path))
        {
            errors.push(tr("Folder path is not consistent with its parent path."));
        }
        errors
    }

    pub fn validate(&self) -> bool {
        self.validation_errors().is_empty()
    }
}

/// Root storage container for `~/.config/onyxsh/sessions.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionsStorageData {
    pub sessions: Vec<SessionItem>,
    pub folders: Vec<SessionFolder>,
}

impl SessionsStorageData {
    /// Safely loads and validates sessions and folders from `~/.config/onyxsh/sessions.json`.
    pub fn load_from_path(path: &Path) -> std::io::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let meta = fs::metadata(path)?;
        if meta.len() == 0 {
            return Ok(Self::default());
        }
        if meta.len() > MAX_SESSIONS_FILE_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Sessions file too large (>50MB)",
            ));
        }
        let content = fs::read_to_string(path)?;
        let raw: Self = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let sessions = raw.sessions.into_iter().filter(|s| s.validate()).collect();
        let folders = raw.folders.into_iter().filter(|f| f.validate()).collect();
        Ok(Self { sessions, folders })
    }

    /// Atomically saves sessions and folders to disk via a temporary file + `fsync` + `rename`.
    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_path = path.with_extension("tmp");
        let serialized = serde_json::to_string_pretty(self)?;
        {
            let mut file = fs::File::create(&tmp_path)?;
            file.write_all(serialized.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_item_validation_and_password_encryption() {
        let mut session = SessionItem {
            name: "Prod Cluster".to_string(),
            session_type: "ssh".to_string(),
            host: "10.0.1.50".to_string(),
            user: "admin".to_string(),
            auth_type: "password".to_string(),
            port: 2222,
            is_production: true,
            ..Default::default()
        };
        session.set_password("Sup3rS3cret!");

        assert!(session.validate());
        assert!(is_securecrt_v2_password(&session.auth_value));
        assert_eq!(session.resolved_auth_value(), "Sup3rS3cret!");
        assert_eq!(session.connection_string(), "admin@10.0.1.50");
    }

    #[test]
    fn test_invalid_ssh_session_detected() {
        let invalid = SessionItem {
            name: "Broken".to_string(),
            session_type: "ssh".to_string(),
            host: "".to_string(),
            ..Default::default()
        };
        assert!(!invalid.validate());
    }
}
