//! Secret Service / Keyring Manager with AES-256-CBC V2 Fallback
//! (`src/onyxsh/core/keyring_manager.py`).

use crate::crypto::{
    decrypt_session_password, encrypt_session_password, is_securecrt_v2_password,
};
use crate::platform::CommandBuilder;
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Trait abstracting secret storage backends (Libsecret D-Bus / `secret-tool` or in-memory mock).
pub trait SecretBackend: Send + Sync {
    fn store(&self, service: &str, account: &str, secret: &str) -> Result<(), String>;
    fn lookup(&self, service: &str, account: &str) -> Result<Option<String>, String>;
    fn clear(&self, service: &str, account: &str) -> Result<(), String>;
}

/// System keyring backend invoking `secret-tool` (libsecret CLI over `org.freedesktop.secrets`).
#[derive(Debug, Default, Clone)]
pub struct SecretToolBackend;

impl SecretBackend for SecretToolBackend {
    fn store(&self, service: &str, account: &str, secret: &str) -> Result<(), String> {
        let raw_cmd = vec![
            "secret-tool".to_string(),
            "store".to_string(),
            "--label".to_string(),
            format!("OnyxSH ({service}:{account})"),
            "service".to_string(),
            service.to_string(),
            "account".to_string(),
            account.to_string(),
        ];
        let wrapped = CommandBuilder::new().wrap_host_command(&raw_cmd, None, None);

        let mut child = std::process::Command::new(&wrapped[0])
            .args(&wrapped[1..])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn secret-tool: {e}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(secret.as_bytes())
                .map_err(|e| format!("Failed to write secret to stdin: {e}"))?;
        }

        let out = child
            .wait_with_output()
            .map_err(|e| format!("secret-tool wait error: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).into_owned())
        }
    }

    fn lookup(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        let raw_cmd = vec![
            "secret-tool".to_string(),
            "lookup".to_string(),
            "service".to_string(),
            service.to_string(),
            "account".to_string(),
            account.to_string(),
        ];
        let wrapped = CommandBuilder::new().wrap_host_command(&raw_cmd, None, None);

        let out = std::process::Command::new(&wrapped[0])
            .args(&wrapped[1..])
            .output()
            .map_err(|e| format!("Failed to run secret-tool lookup: {e}"))?;

        if out.status.success() {
            let value = String::from_utf8_lossy(&out.stdout)
                .trim_end_matches(&['\r', '\n'][..])
                .to_string();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        } else {
            Ok(None)
        }
    }

    fn clear(&self, service: &str, account: &str) -> Result<(), String> {
        let raw_cmd = vec![
            "secret-tool".to_string(),
            "clear".to_string(),
            "service".to_string(),
            service.to_string(),
            "account".to_string(),
            account.to_string(),
        ];
        let wrapped = CommandBuilder::new().wrap_host_command(&raw_cmd, None, None);

        let _ = std::process::Command::new(&wrapped[0])
            .args(&wrapped[1..])
            .output();
        Ok(())
    }
}

/// Thread-safe in-memory keyring backend used for unit testing and headless environments.
#[derive(Debug, Default, Clone)]
pub struct MemorySecretBackend {
    entries: Arc<Mutex<HashMap<(String, String), String>>>,
}

impl SecretBackend for MemorySecretBackend {
    fn store(&self, service: &str, account: &str, secret: &str) -> Result<(), String> {
        self.entries
            .lock()
            .unwrap()
            .insert((service.to_string(), account.to_string()), secret.to_string());
        Ok(())
    }

    fn lookup(&self, service: &str, account: &str) -> Result<Option<String>, String> {
        Ok(self
            .entries
            .lock()
            .unwrap()
            .get(&(service.to_string(), account.to_string()))
            .cloned())
    }

    fn clear(&self, service: &str, account: &str) -> Result<(), String> {
        self.entries
            .lock()
            .unwrap()
            .remove(&(service.to_string(), account.to_string()));
        Ok(())
    }
}

/// High-level Keyring Manager that stores secrets in the keyring when available and
/// transparently falls back to SecureCRT V2 AES-256-CBC (`02:<hex>`) payloads.
pub struct KeyringManager<B: SecretBackend> {
    backend: B,
    service_name: String,
}

impl<B: SecretBackend> KeyringManager<B> {
    pub fn new(backend: B, service_name: impl Into<String>) -> Self {
        Self {
            backend,
            service_name: service_name.into(),
        }
    }

    /// Saves a secret into the keyring or returns an AES-256-CBC `02:<hex>` fallback token.
    pub fn store_or_encrypt(&self, account: &str, plaintext: &str) -> String {
        if plaintext.is_empty() {
            return String::new();
        }
        if self
            .backend
            .store(&self.service_name, account, plaintext)
            .is_ok()
        {
            format!("keyring:{account}")
        } else {
            encrypt_session_password(plaintext, "").unwrap_or_default()
        }
    }

    /// Resolves a secret from either a `keyring:<account>` reference, an AES-256-CBC `02:<hex>`
    /// ciphertext, or a legacy plaintext string.
    pub fn resolve_secret(&self, account: &str, stored_value: &str) -> String {
        if let Some(key_account) = stored_value.strip_prefix("keyring:") {
            if let Ok(Some(secret)) = self.backend.lookup(&self.service_name, key_account) {
                return secret;
            }
        }
        if is_securecrt_v2_password(stored_value) {
            return decrypt_session_password(stored_value, "").unwrap_or_default();
        }
        if stored_value.is_empty() {
            if let Ok(Some(secret)) = self.backend.lookup(&self.service_name, account) {
                return secret;
            }
        }
        stored_value.to_string()
    }

    pub fn delete_secret(&self, account: &str) {
        let _ = self.backend.clear(&self.service_name, account);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyring_manager_store_and_resolve() {
        let mem = MemorySecretBackend::default();
        let mgr = KeyringManager::new(mem, "io.github.vagnarok.OnyxSH");

        let token = mgr.store_or_encrypt("ssh-prod-01", "s3cr3t-pass!");
        assert_eq!(token, "keyring:ssh-prod-01");
        assert_eq!(mgr.resolve_secret("ssh-prod-01", &token), "s3cr3t-pass!");

        mgr.delete_secret("ssh-prod-01");
        assert_eq!(mgr.resolve_secret("ssh-prod-01", ""), "");
    }

    #[test]
    fn test_keyring_manager_resolves_v2_encrypted_fallback() {
        let mem = MemorySecretBackend::default();
        let mgr = KeyringManager::new(mem, "io.github.vagnarok.OnyxSH");

        let encrypted = encrypt_session_password("fallback-secret", "").unwrap();
        assert_eq!(
            mgr.resolve_secret("any-account", &encrypted),
            "fallback-secret"
        );
    }
}
