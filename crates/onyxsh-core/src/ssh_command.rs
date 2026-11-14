//! Unified OpenSSH (`ssh`, `sftp`, `scp`), `ControlMaster`, `sshpass`, and Flatpak host wrapper builder.
//! Shared foundation for terminal spawning, SFTP backend, tunnels, and remote resource monitoring.

use crate::platform::{expand_user_path, CommandBuilder};
use crate::sessions::SessionItem;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshToolKind {
    Ssh,
    Sftp,
    Scp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCommandOptions {
    pub keepalive_interval: u32,
    pub keepalive_count_max: u32,
    pub control_persist_secs: u32,
    pub use_control_master: bool,
    pub batch_mode: bool,
    pub connect_timeout_secs: u32,
    pub resolved_password: Option<String>,
    pub remote_command: Option<String>,
}

impl Default for RemoteCommandOptions {
    fn default() -> Self {
        Self {
            keepalive_interval: 15,
            keepalive_count_max: 3,
            control_persist_secs: 600,
            use_control_master: true,
            batch_mode: false,
            connect_timeout_secs: 15,
            resolved_password: None,
            remote_command: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltRemoteCommand {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub control_socket_path: Option<PathBuf>,
}

/// Computes a short deterministic Unix socket path under `/tmp` to avoid the 107-byte `sun_path` cap.
pub fn control_socket_path_for(session: &SessionItem) -> PathBuf {
    let mut hasher = Sha256::new();
    let uid = libc_uid();
    hasher.update(uid.to_le_bytes());
    hasher.update(session.user.as_bytes());
    hasher.update(b"@");
    hasher.update(session.host.as_bytes());
    hasher.update(b":");
    hasher.update(session.effective_port().to_le_bytes());
    let digest = hasher.finalize();
    let short_hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
    PathBuf::from(format!("/tmp/onyxsh-cm-{short_hex}.sock"))
}

fn libc_uid() -> u32 {
    std::env::var("UID")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1000)
}

/// Builds a complete OpenSSH command line (with optional `sshpass` and Flatpak `host-spawn` wrapping).
pub fn build_remote_command(
    kind: SshToolKind,
    session: &SessionItem,
    opts: &RemoteCommandOptions,
    platform_builder: Option<&CommandBuilder>,
) -> BuiltRemoteCommand {
    let mut env = HashMap::new();
    let mut cmd = Vec::new();

    if session.uses_password_auth() {
        if let Some(ref pw) = opts.resolved_password {
            if !pw.is_empty() {
                env.insert("SSHPASS".to_string(), pw.clone());
                cmd.push("sshpass".to_string());
                cmd.push("-e".to_string());
            }
        }
    }

    let bin = match kind {
        SshToolKind::Ssh => "ssh",
        SshToolKind::Sftp => "sftp",
        SshToolKind::Scp => "scp",
    };
    cmd.push(bin.to_string());

    let port = session.effective_port();
    if port != 22 {
        match kind {
            SshToolKind::Ssh => {
                cmd.push("-p".to_string());
                cmd.push(port.to_string());
            }
            SshToolKind::Sftp | SshToolKind::Scp => {
                cmd.push("-P".to_string());
                cmd.push(port.to_string());
            }
        }
    }

    if kind == SshToolKind::Ssh && session.x11_forwarding {
        cmd.push("-X".to_string());
    }

    if session.uses_key_auth() && !session.auth_value.trim().is_empty() {
        let key_path = expand_user_path(session.auth_value.trim());
        cmd.push("-i".to_string());
        cmd.push(key_path.to_string_lossy().into_owned());
    }

    if opts.batch_mode {
        cmd.push("-o".to_string());
        cmd.push("BatchMode=yes".to_string());
    }

    if opts.connect_timeout_secs > 0 {
        cmd.push("-o".to_string());
        cmd.push(format!("ConnectTimeout={}", opts.connect_timeout_secs));
    }

    if opts.keepalive_interval > 0 {
        cmd.push("-o".to_string());
        cmd.push(format!("ServerAliveInterval={}", opts.keepalive_interval));
        cmd.push("-o".to_string());
        cmd.push(format!("ServerAliveCountMax={}", opts.keepalive_count_max.max(1)));
    }

    let control_socket_path = if opts.use_control_master && session.is_ssh() {
        let sock = control_socket_path_for(session);
        cmd.push("-o".to_string());
        cmd.push("ControlMaster=auto".to_string());
        cmd.push("-o".to_string());
        cmd.push(format!("ControlPath={}", sock.to_string_lossy()));
        cmd.push("-o".to_string());
        cmd.push(format!("ControlPersist={}", opts.control_persist_secs));
        Some(sock)
    } else {
        None
    };

    let target = if session.user.trim().is_empty() {
        session.host.trim().to_string()
    } else {
        format!("{}@{}", session.user.trim(), session.host.trim())
    };

    match kind {
        SshToolKind::Ssh => {
            if !target.is_empty() {
                cmd.push(target);
            }
            if let Some(ref remote_cmd) = opts.remote_command {
                if !remote_cmd.trim().is_empty() {
                    cmd.push(remote_cmd.clone());
                }
            }
        }
        SshToolKind::Sftp => {
            if !target.is_empty() {
                cmd.push(target);
            }
        }
        SshToolKind::Scp => {}
    }

    let final_argv = if let Some(builder) = platform_builder {
        let env_opt = if env.is_empty() { None } else { Some(&env) };
        builder.wrap_host_command(&cmd, None, env_opt)
    } else {
        cmd
    };

    BuiltRemoteCommand {
        argv: final_argv,
        env,
        control_socket_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_ssh_key_and_password_commands() {
        let mut key_sess = SessionItem::default();
        key_sess.session_type = "ssh".to_string();
        key_sess.host = "prod.example.com".to_string();
        key_sess.user = "devops".to_string();
        key_sess.port = 2222;
        key_sess.x11_forwarding = true;
        key_sess.auth_type = "key".to_string();
        key_sess.auth_value = "~/.ssh/id_ed25519".to_string();

        let built = build_remote_command(
            SshToolKind::Ssh,
            &key_sess,
            &RemoteCommandOptions {
                remote_command: Some("df -k /".to_string()),
                ..Default::default()
            },
            None,
        );
        assert!(built.argv.contains(&"-p".to_string()));
        assert!(built.argv.contains(&"2222".to_string()));
        assert!(built.argv.contains(&"-X".to_string()));
        assert!(built.argv.contains(&"-i".to_string()));
        assert!(built.argv.contains(&"ControlMaster=auto".to_string()));
        assert!(built.argv.contains(&"devops@prod.example.com".to_string()));
        assert!(built.argv.contains(&"df -k /".to_string()));
        let sock = built.control_socket_path.unwrap();
        assert!(sock.to_string_lossy().len() < 80);

        let mut pw_sess = key_sess.clone();
        pw_sess.auth_type = "password".to_string();
        let flatpak_builder = CommandBuilder {
            force_flatpak: Some(true),
            host_spawn_path: Some(PathBuf::from("/app/bin/host-spawn")),
            flatpak_spawn_available: Some(false),
        };
        let sftp_built = build_remote_command(
            SshToolKind::Sftp,
            &pw_sess,
            &RemoteCommandOptions {
                resolved_password: Some("s3cr3t!".to_string()),
                ..Default::default()
            },
            Some(&flatpak_builder),
        );
        assert_eq!(sftp_built.argv[0], "/app/bin/host-spawn");
        assert!(sftp_built.argv.contains(&"sshpass".to_string()));
        assert!(sftp_built.argv.contains(&"sftp".to_string()));
        assert!(sftp_built.argv.contains(&"-P".to_string()));
        assert_eq!(sftp_built.env.get("SSHPASS").map(String::as_str), Some("s3cr3t!"));
    }
}

