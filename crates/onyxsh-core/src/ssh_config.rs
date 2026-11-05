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
                            if candidate.is_file() {
                                self.parse_file(&candidate);
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
}
