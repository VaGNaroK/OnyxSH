//! PathGuard for restricting filesystem access by the AI Agent.
//! Port of `src/onyxsh/agent/path_guard.py`.

use onyxsh_core::platform::{expand_user_path, home_dir};
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_READ_DENYLIST: &[&str] = &[
    "~/.ssh",
    "~/.gnupg",
    "~/.aws",
    "~/.kube",
    "~/.docker",
    "~/.netrc",
    "~/.pypirc",
    "~/.npmrc",
    "~/.git-credentials",
    "~/.bash_history",
    "~/.zsh_history",
    "~/.local/share/keyrings",
    "~/.config/gcloud",
    "/etc/shadow",
    "/etc/gshadow",
    "/etc/sudoers",
    "/etc/sudoers.d",
    "/root",
];

pub const DEFAULT_WRITE_DENYLIST: &[&str] = &[
    "~/.bashrc",
    "~/.zshrc",
    "~/.profile",
    "~/.bash_profile",
    "~/.bash_login",
    "~/.bash_logout",
    "~/.config/autostart",
    "~/.config/systemd/user",
    "~/.ssh",
    "~/.gnupg",
    "/etc",
    "/usr",
    "/bin",
    "/sbin",
    "/lib",
    "/lib64",
    "/boot",
    "/dev",
    "/proc",
    "/sys",
    "/root",
    "/var",
];

/// Normalizes a path lexically (resolving `.` and `..`) and follows symlinks if the file exists.
pub fn normalize_and_resolve(raw: &str) -> PathBuf {
    let expanded = expand_user_path(raw);
    if let Ok(canon) = std::fs::canonicalize(&expanded) {
        return canon;
    }
    let mut out = PathBuf::new();
    for comp in expanded.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn is_env_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    let lower = name.to_lowercase();
    lower == ".env" || lower.starts_with(".env.") || lower.ends_with(".env")
}

#[derive(Debug, Clone)]
pub struct PathGuard {
    home: PathBuf,
    allowed_roots: Vec<PathBuf>,
    read_denylist: Vec<PathBuf>,
    write_denylist: Vec<PathBuf>,
}

impl Default for PathGuard {
    fn default() -> Self {
        Self::new(None)
    }
}

impl PathGuard {
    pub fn new(allowed_roots: Option<Vec<PathBuf>>) -> Self {
        let home = normalize_and_resolve(&home_dir().to_string_lossy());
        let roots = allowed_roots.unwrap_or_else(|| vec![home.clone()]);
        let read_denylist = DEFAULT_READ_DENYLIST
            .iter()
            .map(|e| normalize_and_resolve(e))
            .collect();
        let write_denylist = DEFAULT_WRITE_DENYLIST
            .iter()
            .map(|e| normalize_and_resolve(e))
            .collect();

        Self {
            home,
            allowed_roots: roots,
            read_denylist,
            write_denylist,
        }
    }

    fn matches_denylist(target: &Path, denylist: &[PathBuf]) -> bool {
        if is_env_file(target) {
            return true;
        }
        denylist.iter().any(|denied| target.starts_with(denied))
    }

    pub fn can_read(&self, raw_path: &str) -> bool {
        if raw_path.trim().is_empty() {
            return false;
        }
        let resolved = normalize_and_resolve(raw_path);
        if Self::matches_denylist(&resolved, &self.read_denylist) {
            return false;
        }
        if self.allowed_roots.iter().any(|r| resolved.starts_with(r)) {
            return true;
        }
        let safe_system = [Path::new("/tmp"), Path::new("/var/log"), Path::new("/usr/share")];
        safe_system.iter().any(|s| resolved.starts_with(s))
    }

    pub fn can_write(&self, raw_path: &str) -> bool {
        if raw_path.trim().is_empty() {
            return false;
        }
        let resolved = normalize_and_resolve(raw_path);
        if Self::matches_denylist(&resolved, &self.write_denylist)
            || Self::matches_denylist(&resolved, &self.read_denylist)
        {
            return false;
        }
        if !resolved.starts_with(&self.home) && !resolved.starts_with("/tmp") {
            return false;
        }
        self.allowed_roots.iter().any(|r| resolved.starts_with(r))
            || resolved.starts_with("/tmp")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_guard_blocks_ssh_bashrc_shadow_and_env() {
        let guard = PathGuard::default();
        assert!(!guard.can_read("~/.ssh/id_ed25519"));
        assert!(!guard.can_read("/etc/shadow"));
        assert!(!guard.can_read("~/project/.env.production"));
        assert!(!guard.can_write("~/.bashrc"));
        assert!(!guard.can_write("/etc/hosts"));

        assert!(guard.can_read("~/project/main.rs"));
        assert!(guard.can_write("~/project/main.rs"));
    }
}
