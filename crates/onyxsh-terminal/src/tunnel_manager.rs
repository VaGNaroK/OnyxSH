//! SSH Tunnel and Port Forwarding Manager (`-L`, `-R`, `-D`).
//! Port of `src/onyxsh/terminal/tunnel_manager.py`.

use onyxsh_core::platform::expand_user_path;
use onyxsh_core::sessions::SessionItem;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SshTunnel {
    pub id: String,
    pub name: String,
    pub session_name: String,
    pub tunnel_type: String, // "local", "remote", "dynamic"
    pub local_host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub auto_start: bool,
    pub status: String, // "stopped", "starting", "active", "error"
    pub error_message: String,
}

impl Default for SshTunnel {
    fn default() -> Self {
        Self {
            id: "tun_0001".to_string(),
            name: String::new(),
            session_name: String::new(),
            tunnel_type: "local".to_string(),
            local_host: "127.0.0.1".to_string(),
            local_port: 8080,
            remote_host: "localhost".to_string(),
            remote_port: 80,
            auto_start: false,
            status: "stopped".to_string(),
            error_message: String::new(),
        }
    }
}

impl SshTunnel {
    pub fn display_source(&self) -> String {
        if self.tunnel_type == "remote" {
            format!("remote:{}", self.remote_port)
        } else {
            format!("{}:{}", self.local_host, self.local_port)
        }
    }

    pub fn display_target(&self) -> String {
        match self.tunnel_type.as_str() {
            "dynamic" => "SOCKS5 Proxy".to_string(),
            "remote" => format!("{}:{}", self.local_host, self.local_port),
            _ => format!("{}:{}", self.remote_host, self.remote_port),
        }
    }

    pub fn type_label(&self) -> &'static str {
        match self.tunnel_type.as_str() {
            "dynamic" => "Dynamic (SOCKS5)",
            "remote" => "Remote (-R)",
            _ => "Local (-L)",
        }
    }

    /// Builds the exact non-interactive OpenSSH command line to establish this tunnel.
    pub fn build_ssh_command(&self, session: &SessionItem, ssh_bin: &str) -> Vec<String> {
        let mut cmd = vec![
            ssh_bin.to_string(),
            "-N".to_string(),
            "-T".to_string(),
            "-o".to_string(),
            "ExitOnForwardFailure=yes".to_string(),
            "-o".to_string(),
            "ServerAliveInterval=15".to_string(),
            "-o".to_string(),
            "ServerAliveCountMax=3".to_string(),
            "-o".to_string(),
            "ConnectTimeout=10".to_string(),
            "-o".to_string(),
            "BatchMode=yes".to_string(),
        ];

        if session.port != 0 && session.port != 22 {
            cmd.push("-p".to_string());
            cmd.push(session.port.to_string());
        }

        if session.uses_key_auth() && !session.auth_value.is_empty() {
            let key_path = expand_user_path(&session.auth_value);
            cmd.push("-i".to_string());
            cmd.push(key_path.to_string_lossy().to_string());
        }

        match self.tunnel_type.as_str() {
            "dynamic" => {
                let spec = if self.local_host.is_empty() {
                    self.local_port.to_string()
                } else {
                    format!("{}:{}", self.local_host, self.local_port)
                };
                cmd.push("-D".to_string());
                cmd.push(spec);
            }
            "remote" => {
                let spec = format!("{}:{}:{}", self.remote_port, self.local_host, self.local_port);
                cmd.push("-R".to_string());
                cmd.push(spec);
            }
            _ => {
                let spec = format!(
                    "{}:{}:{}:{}",
                    self.local_host, self.local_port, self.remote_host, self.remote_port
                );
                cmd.push("-L".to_string());
                cmd.push(spec);
            }
        }

        cmd.push(session.connection_string());
        cmd
    }
}

#[derive(Debug, Default)]
pub struct SshTunnelRegistry {
    tunnels: HashMap<String, SshTunnel>,
}

impl SshTunnelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tunnel: SshTunnel) -> String {
        let id = tunnel.id.clone();
        self.tunnels.insert(id.clone(), tunnel);
        id
    }

    pub fn unregister(&mut self, tunnel_id: &str) -> Option<SshTunnel> {
        self.tunnels.remove(tunnel_id)
    }

    pub fn get(&self, tunnel_id: &str) -> Option<&SshTunnel> {
        self.tunnels.get(tunnel_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_command_generation_local_remote_dynamic() {
        let session = SessionItem {
            name: "Bastion".to_string(),
            session_type: "ssh".to_string(),
            host: "bastion.internal".to_string(),
            user: "devops".to_string(),
            port: 2222,
            auth_type: "key".to_string(),
            auth_value: "/home/devops/.ssh/id_ed25519".to_string(),
            ..Default::default()
        };

        let local_tun = SshTunnel {
            id: "t1".to_string(),
            name: "Postgres".to_string(),
            tunnel_type: "local".to_string(),
            local_host: "127.0.0.1".to_string(),
            local_port: 5432,
            remote_host: "db.internal".to_string(),
            remote_port: 5432,
            ..Default::default()
        };
        let cmd = local_tun.build_ssh_command(&session, "/usr/bin/ssh");
        assert!(cmd.contains(&"-L".to_string()));
        assert!(cmd.contains(&"127.0.0.1:5432:db.internal:5432".to_string()));
        assert!(cmd.contains(&"devops@bastion.internal".to_string()));

        let socks_tun = SshTunnel {
            tunnel_type: "dynamic".to_string(),
            local_host: "127.0.0.1".to_string(),
            local_port: 1080,
            ..Default::default()
        };
        let socks_cmd = socks_tun.build_ssh_command(&session, "/usr/bin/ssh");
        assert!(socks_cmd.contains(&"-D".to_string()));
        assert!(socks_cmd.contains(&"127.0.0.1:1080".to_string()));
    }
}
