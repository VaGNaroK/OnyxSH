//! OpenSSH configuration file parser (`~/.ssh/config`).
//! Port of `src/onyxsh/utils/ssh_config_parser.py`.

use crate::platform::expand_user_path;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SshConfigHost {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub forward_x11: Option<bool>,
}

#[derive(Debug, Default)]
pub struct SshConfigParser {
    entries: Vec<SshConfigHost>,
    visited: HashSet<PathBuf>,
}

impl SshConfigParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses an OpenSSH configuration file and returns all concrete host entries.
    pub fn parse(&mut self, config_path: &Path) -> Vec<SshConfigHost> {
        self.entries.clear();
        self.visited.clear();
        let expanded = expand_user_path(&config_path.to_string_lossy());
        self.parse_file(&expanded);
        self.entries.clone()
    }

    /// Parses raw OpenSSH config text directly (useful for testing and in-memory inspection).
    pub fn parse_str(&mut self, content: &str, base_dir: Option<&Path>) -> Vec<SshConfigHost> {
        self.entries.clear();
        self.parse_lines(content, base_dir);
        self.entries.clone()
    }

    fn parse_file(&mut self, path: &Path) {
        let Ok(resolved) = fs::canonicalize(path) else {
            return;
        };
        if self.visited.contains(&resolved) || !resolved.is_file() {
            return;
        }
        self.visited.insert(resolved.clone());
        let Ok(content) = fs::read_to_string(&resolved) else {
            return;
        };
        self.parse_lines(&content, resolved.parent());
    }

    fn parse_lines(&mut self, content: &str, base_dir: Option<&Path>) {
        let mut current_patterns: Vec<String> = Vec::new();
        let mut current_options: HashMap<String, String> = HashMap::new();

        for raw_line in content.lines() {
            let line_no_comment = match raw_line.find('#') {
                Some(idx) => &raw_line[..idx],
                None => raw_line,
            };
            let line = line_no_comment.trim();
            if line.is_empty() {
                continue;
            }

            let tokens = shlex::split(line).unwrap_or_else(|| {
                line.split_whitespace().map(String::from).collect()
            });
            if tokens.is_empty() {
                continue;
            }

            let keyword = tokens[0].to_lowercase();
            let values = &tokens[1..];

            match keyword.as_str() {
                "match" => {
                    self.flush_hosts(&current_patterns, &current_options);
                    current_patterns.clear();
                    current_options.clear();
                    break;
                }
                "host" => {
                    self.flush_hosts(&current_patterns, &current_options);
                    current_patterns = values.to_vec();
                    current_options.clear();
                }
                "include" => {
                    self.flush_hosts(&current_patterns, &current_options);
                    current_patterns.clear();
                    current_options.clear();
                    if let Some(dir) = base_dir {
                        for pat in values {
                            let candidate = expand_include_path(pat, dir);
                            for expanded in expand_glob_candidates(&candidate) {
                                if expanded.is_file() {
                                    self.parse_file(&expanded);
                                }
                            }
                        }
                    }
                }
                _ => {
                    if !current_patterns.is_empty() && !values.is_empty() {
                        current_options.insert(keyword, values.join(" "));
                    }
                }
            }
        }

        self.flush_hosts(&current_patterns, &current_options);
    }

    fn flush_hosts(&mut self, patterns: &[String], options: &HashMap<String, String>) {
        for alias in patterns {
            if alias.is_empty() || alias.contains(['*', '?', '!']) {
                continue;
            }
            let mut entry = SshConfigHost {
                alias: alias.clone(),
                ..Default::default()
            };
            if let Some(h) = options.get("hostname") {
                entry.hostname = Some(h.clone());
            }
            if let Some(u) = options.get("user") {
                entry.user = Some(u.clone());
            }
            if let Some(p) = options.get("port") {
                if let Ok(port_num) = p.parse::<u16>() {
                    entry.port = Some(port_num);
                }
            }
            if let Some(id) = options.get("identityfile") {
                entry.identity_file = Some(id.clone());
            }
            if let Some(x11) = options.get("forwardx11") {
                entry.forward_x11 = Some(matches!(
                    x11.to_lowercase().as_str(),
                    "yes" | "true" | "on"
                ));
            }
            self.entries.push(entry);
        }
    }
}

fn expand_include_path(pattern: &str, base_dir: &Path) -> PathBuf {
    let expanded = expand_user_path(pattern);
    if expanded.is_absolute() {
        expanded
    } else {
        base_dir.join(expanded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_config_parser_skips_wildcards_and_parses_hosts() {
        let sample = r#"
Host *
    ServerAliveInterval 60

Host prod-web prod-api
    HostName 10.20.30.40
    User ubuntu
    Port 2222
    IdentityFile ~/.ssh/id_ed25519
    ForwardX11 yes

Host *.internal
    User admin

Match host 192.168.*
    User root
"#;
        let mut parser = SshConfigParser::new();
        let hosts = parser.parse_str(sample, None);
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts[0].alias, "prod-web");
        assert_eq!(hosts[0].hostname.as_deref(), Some("10.20.30.40"));
        assert_eq!(hosts[0].user.as_deref(), Some("ubuntu"));
        assert_eq!(hosts[0].port, Some(2222));
        assert_eq!(hosts[0].forward_x11, Some(true));
        assert_eq!(hosts[1].alias, "prod-api");
    }

    #[test]
    fn test_include_glob_expansion() {
        let base = std::env::temp_dir().join(format!("onyxsh_ssh_glob_{}", std::process::id()));
        let conf_d = base.join("conf.d");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&conf_d).unwrap();

        std::fs::write(
            base.join("config"),
            "Include conf.d/*.conf\n\nHost root-box\n    HostName 10.0.0.1\n",
        )
        .unwrap();
        std::fs::write(
            conf_d.join("01-prod.conf"),
            "Host prod-db\n    HostName 10.0.1.5\n    User postgres\n",
        )
        .unwrap();
        std::fs::write(
            conf_d.join("02-stage.conf"),
            "Host stage-app\n    HostName 10.0.2.9\n    Port 2200\n",
        )
        .unwrap();
        std::fs::write(
            conf_d.join("ignored.txt"),
            "Host should-ignore\n    HostName 127.0.0.1\n",
        )
        .unwrap();

        let mut parser = SshConfigParser::new();
        let hosts = parser.parse(&base.join("config"));
        let aliases: Vec<&str> = hosts.iter().map(|h| h.alias.as_str()).collect();
        assert_eq!(aliases, vec!["prod-db", "stage-app", "root-box"]);
        assert_eq!(hosts[0].user.as_deref(), Some("postgres"));
        assert_eq!(hosts[1].port, Some(2200));

        let _ = std::fs::remove_dir_all(&base);
    }
}

fn expand_glob_candidates(pattern_path: &Path) -> Vec<PathBuf> {
    let raw = pattern_path.to_string_lossy();
    if !raw.contains('*') && !raw.contains('?') {
        return vec![pattern_path.to_path_buf()];
    }
    let Some(parent) = pattern_path.parent() else {
        return Vec::new();
    };
    let Some(file_pat) = pattern_path.file_name().and_then(|s| s.to_str()) else {
        return Vec::new();
    };
    let parent_dirs = if parent.to_string_lossy().contains('*') || parent.to_string_lossy().contains('?') {
        expand_glob_candidates(parent)
    } else {
        vec![parent.to_path_buf()]
    };

    let mut matches = Vec::new();
    for dir in parent_dirs {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if wildcard_match(file_pat, &name_str) {
                    matches.push(entry.path());
                }
            }
        }
    }
    matches.sort();
    matches
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_pi, mut match_ti) = (None, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star_pi = Some(pi);
            match_ti = ti;
            pi += 1;
        } else if let Some(sp) = star_pi {
            pi = sp + 1;
            match_ti += 1;
            ti = match_ti;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}
