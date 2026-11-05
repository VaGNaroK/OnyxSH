//! Platform detection, XDG directories, Flatpak sandbox detection, and host-spawn command builder.
//! Port of `src/onyxsh/utils/platform.py`.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Returns true if the current process is running inside a Flatpak sandbox.
pub fn is_flatpak_sandbox() -> bool {
    Path::new("/.flatpak-info").exists() || env::var_os("FLATPAK_ID").is_some()
}

/// Resolves the user's home directory.
pub fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Resolves the OnyxSH XDG configuration directory (`~/.config/onyxsh`).
pub fn config_dir() -> PathBuf {
    if let Some(xdg) = env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("onyxsh");
        }
    }
    home_dir().join(".config").join("onyxsh")
}

/// Resolves the OnyxSH XDG data directory (`~/.local/share/onyxsh`).
pub fn data_dir() -> PathBuf {
    if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("onyxsh");
        }
    }
    home_dir().join(".local").join("share").join("onyxsh")
}

/// Resolves the OnyxSH XDG cache directory (`~/.cache/onyxsh`).
pub fn cache_dir() -> PathBuf {
    if let Some(xdg) = env::var_os("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("onyxsh");
        }
    }
    home_dir().join(".cache").join("onyxsh")
}

/// Expands leading `~` to the user's home directory.
pub fn expand_user_path(raw: &str) -> PathBuf {
    if raw == "~" {
        return home_dir();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return home_dir().join(rest);
    }
    PathBuf::from(raw)
}

/// Parses `/etc/os-release` content into a key-value map.
pub fn parse_os_release_str(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let clean_val = v.trim().trim_matches('"').trim_matches('\'').to_string();
            map.insert(k.trim().to_string(), clean_val);
        }
    }
    map
}

/// Reads host or system `/etc/os-release`.
pub fn read_os_release() -> HashMap<String, String> {
    let candidates = [
        "/var/run/host/os-release",
        "/run/host/os-release",
        "/run/host/usr/lib/os-release",
        "/run/host/etc/os-release",
        "/etc/os-release",
        "/usr/lib/os-release",
    ];
    for candidate in candidates {
        let p = Path::new(candidate);
        if !p.exists() {
            continue;
        }
        if let Ok(content) = fs::read_to_string(p) {
            let map = parse_os_release_str(&content);
            let name_lower = map.get("NAME").map(|s| s.to_lowercase()).unwrap_or_default();
            if !map.is_empty() && (!name_lower.contains("flatpak") || candidate == "/usr/lib/os-release") {
                return map;
            }
        }
    }
    HashMap::new()
}

/// Detects the human-readable OS context string for AI prompts.
pub fn detect_os_context() -> String {
    let info = read_os_release();
    let os_name = info
        .get("PRETTY_NAME")
        .or_else(|| info.get("NAME"))
        .cloned()
        .unwrap_or_else(|| "Linux".to_string());
    let base_distro = info
        .get("ID_LIKE")
        .or_else(|| info.get("ID"))
        .cloned()
        .unwrap_or_default();

    if !base_distro.is_empty()
        && !os_name.to_lowercase().contains(&base_distro.to_lowercase())
        && !base_distro.to_lowercase().contains("freedesktop")
    {
        format!("{os_name} (based on {base_distro})")
    } else {
        os_name
    }
}

/// Command builder capable of wrapping commands for Flatpak host execution (`host-spawn` / `flatpak-spawn`).
#[derive(Debug, Clone, Default)]
pub struct CommandBuilder {
    pub force_flatpak: Option<bool>,
    pub host_spawn_path: Option<PathBuf>,
    pub flatpak_spawn_available: Option<bool>,
}

impl CommandBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_sandboxed(&self) -> bool {
        self.force_flatpak.unwrap_or_else(is_flatpak_sandbox)
    }

    /// Wraps a command with `host-spawn` or `flatpak-spawn --host` when running inside Flatpak.
    pub fn wrap_host_command(
        &self,
        command: &[String],
        working_directory: Option<&str>,
        env_vars: Option<&HashMap<String, String>>,
    ) -> Vec<String> {
        if !self.is_sandboxed() {
            return command.to_vec();
        }

        let host_spawn = self
            .host_spawn_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("/app/bin/host-spawn"));

        if host_spawn.exists() || self.host_spawn_path.is_some() {
            let mut cmd = vec![host_spawn.to_string_lossy().to_string()];
            if let Some(cwd) = working_directory {
                cmd.push("-cwd".to_string());
                cmd.push(cwd.to_string());
            }
            if let Some(env_map) = env_vars {
                let mut keys: Vec<_> = env_map.keys().collect();
                keys.sort();
                for k in keys {
                    if let Some(v) = env_map.get(k) {
                        cmd.push("--env".to_string());
                        cmd.push(format!("{k}={v}"));
                    }
                }
            }
            cmd.extend_from_slice(command);
            return cmd;
        }

        if self.flatpak_spawn_available.unwrap_or(false) {
            let mut cmd = vec![
                "flatpak-spawn".to_string(),
                "--host".to_string(),
                "--watch-bus".to_string(),
            ];
            if let Some(cwd) = working_directory {
                cmd.push(format!("--directory={cwd}"));
            }
            if let Some(env_map) = env_vars {
                let mut keys: Vec<_> = env_map.keys().collect();
                keys.sort();
                for k in keys {
                    if let Some(v) = env_map.get(k) {
                        cmd.push(format!("--env={k}={v}"));
                    }
                }
            }
            cmd.extend_from_slice(command);
            return cmd;
        }

        command.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_os_release() {
        let sample = r#"
NAME="Ubuntu"
VERSION_ID="24.04"
PRETTY_NAME="Ubuntu 24.04.1 LTS"
ID=ubuntu
ID_LIKE=debian
"#;
        let parsed = parse_os_release_str(sample);
        assert_eq!(parsed.get("ID").map(String::as_str), Some("ubuntu"));
        assert_eq!(
            parsed.get("PRETTY_NAME").map(String::as_str),
            Some("Ubuntu 24.04.1 LTS")
        );
    }

    #[test]
    fn test_wrap_host_command_native_vs_flatpak() {
        let native_builder = CommandBuilder {
            force_flatpak: Some(false),
            host_spawn_path: None,
            flatpak_spawn_available: None,
        };
        let base_cmd = vec!["/bin/bash".to_string(), "-l".to_string()];
        assert_eq!(
            native_builder.wrap_host_command(&base_cmd, Some("/home/user"), None),
            base_cmd
        );

        let flatpak_builder = CommandBuilder {
            force_flatpak: Some(true),
            host_spawn_path: Some(PathBuf::from("/app/bin/host-spawn")),
            flatpak_spawn_available: Some(true),
        };
        let mut env_map = HashMap::new();
        env_map.insert("TERM".to_string(), "xterm-256color".to_string());
        let wrapped = flatpak_builder.wrap_host_command(&base_cmd, Some("/home/user"), Some(&env_map));
        assert_eq!(
            wrapped,
            vec![
                "/app/bin/host-spawn",
                "-cwd",
                "/home/user",
                "--env",
                "TERM=xterm-256color",
                "/bin/bash",
                "-l"
            ]
        );
    }
}
