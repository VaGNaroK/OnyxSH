//! Security and input validators (Port of `src/onyxsh/utils/security.py` and `sessions/validation.py`).

use regex::Regex;
use std::net::IpAddr;
use std::sync::OnceLock;

static HOSTNAME_RE: OnceLock<Regex> = OnceLock::new();

fn hostname_regex() -> &'static Regex {
    HOSTNAME_RE.get_or_init(|| {
        Regex::new(r"^(?i:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)(?:\.(?i:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?))*$")
            .expect("valid hostname regex")
    })
}

/// Validates whether a target host is a valid IPv4, IPv6, or RFC-1123 hostname.
pub fn is_valid_host(host: &str) -> bool {
    let trimmed = host.trim();
    if trimmed.is_empty() || trimmed.len() > 253 {
        return false;
    }
    if trimmed.contains(char::is_whitespace)
        || trimmed.contains(';')
        || trimmed.contains('|')
        || trimmed.contains('&')
        || trimmed.contains('`')
        || trimmed.contains('$')
    {
        return false;
    }
    if trimmed.parse::<IpAddr>().is_ok() {
        return true;
    }
    hostname_regex().is_match(trimmed)
}

/// Validates SSH port range (1..=65535).
pub fn is_valid_port(port: u32) -> bool {
    (1..=65535).contains(&port)
}

/// Validates SSH username against command injection characters.
pub fn is_valid_username(username: &str) -> bool {
    let trimmed = username.trim();
    if trimmed.is_empty() || trimmed.len() > 64 {
        return false;
    }
    if trimmed.starts_with('-') {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '@'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_validation() {
        assert!(is_valid_host("192.168.1.10"));
        assert!(is_valid_host("::1"));
        assert!(is_valid_host("srv-prod-01.internal.lan"));
        assert!(!is_valid_host(""));
        assert!(!is_valid_host("bad host; rm -rf /"));
        assert!(!is_valid_host("-invalid.host"));
    }

    #[test]
    fn test_port_and_username_validation() {
        assert!(is_valid_port(22));
        assert!(is_valid_port(65535));
        assert!(!is_valid_port(0));
        assert!(!is_valid_port(70000));

        assert!(is_valid_username("vagnarok"));
        assert!(is_valid_username("deploy_user-01"));
        assert!(!is_valid_username("-oProxyCommand=evil"));
        assert!(!is_valid_username("user;id"));
    }
}
