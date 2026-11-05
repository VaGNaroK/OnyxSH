//! Isolated Polkit Privilege Elevation Helper (`src/onyxsh/admin/helper.py`).

use serde::{Deserialize, Serialize};
use std::process::Command;

/// Strict allowlist of system binaries permitted through the privileged helper.
pub const ALLOWED_ADMIN_BINARIES: &[&str] = &[
    "/usr/bin/systemctl",
    "/bin/systemctl",
    "/usr/bin/apt",
    "/usr/bin/apt-get",
    "/usr/bin/dnf",
    "/usr/bin/pacman",
    "/usr/bin/journalctl",
    "/usr/sbin/ufw",
    "/usr/bin/docker",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminRequest {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Validates that an admin helper request uses an absolute path, has no shell metacharacters,
/// and is in the strict binary allowlist.
pub fn validate_admin_request(req: &AdminRequest) -> Result<(), &'static str> {
    if !req.program.starts_with('/') {
        return Err("Program path must be absolute");
    }
    if req.program.contains("..")
        || req.program.contains(';')
        || req.program.contains('|')
        || req.program.contains('&')
        || req.program.contains('`')
    {
        return Err("Invalid characters in program path");
    }
    if !ALLOWED_ADMIN_BINARIES.contains(&req.program.as_str()) {
        return Err("Program is not in the privileged helper allowlist");
    }
    if req.args.iter().any(|arg| arg.contains('\0')) {
        return Err("Null byte detected in argument list");
    }
    Ok(())
}

/// Executes a validated [`AdminRequest`] directly via `execve` without invoking a shell.
pub fn execute_admin_request(req: &AdminRequest) -> Result<AdminResponse, String> {
    validate_admin_request(req).map_err(|e| e.to_string())?;

    let output = Command::new(&req.program)
        .args(&req.args)
        .output()
        .map_err(|e| format!("Failed to spawn {}: {e}", req.program))?;

    Ok(AdminResponse {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn main() {
    let mut args = std::env::args().skip(1);
    if let Some(json_payload) = args.next() {
        match serde_json::from_str::<AdminRequest>(&json_payload) {
            Ok(req) => match execute_admin_request(&req) {
                Ok(resp) => {
                    if let Ok(out) = serde_json::to_string(&resp) {
                        println!("{out}");
                    }
                    std::process::exit(resp.exit_code);
                }
                Err(err) => {
                    eprintln!("onyxsh-admin-helper rejected request: {err}");
                    std::process::exit(126);
                }
            },
            Err(err) => {
                eprintln!("onyxsh-admin-helper invalid JSON payload: {err}");
                std::process::exit(2);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admin_request_validation() {
        let valid = AdminRequest {
            program: "/usr/bin/systemctl".to_string(),
            args: vec!["restart".to_string(), "nginx".to_string()],
        };
        assert!(validate_admin_request(&valid).is_ok());

        let relative = AdminRequest {
            program: "systemctl".to_string(),
            args: vec![],
        };
        assert!(validate_admin_request(&relative).is_err());

        let unallowed = AdminRequest {
            program: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), "id".to_string()],
        };
        assert!(validate_admin_request(&unallowed).is_err());
    }
}
