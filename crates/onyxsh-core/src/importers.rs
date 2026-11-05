//! Session URI & External Config Importers (`ssh://`, `telnet://`, `serial://`, MobaXterm INI).
//! Port of session import/URI parsing logic in `src/onyxsh/sessions/`.

use crate::security::{is_valid_host, is_valid_port, is_valid_username};
use crate::sessions::SessionItem;

/// Parses a connection URI (`ssh://[user@]host[:port]`, `telnet://host[:port]`, `serial:///dev/ttyUSB0`)
/// into a validated [`SessionItem`].
pub fn parse_connection_uri(uri: &str) -> Result<SessionItem, &'static str> {
    let trimmed = uri.trim();
    let (scheme, rest) = trimmed
        .split_once("://")
        .ok_or("Missing URI scheme (expected ssh://, telnet://, or serial://)")?;

    let protocol = scheme.to_ascii_lowercase();
    match protocol.as_str() {
        "ssh" | "telnet" => {
            let default_port = if protocol == "ssh" { 22 } else { 23 };
            let authority = rest.split('/').next().unwrap_or(rest);
            let (user_opt, host_port) = match authority.rsplit_once('@') {
                Some((u, hp)) => (Some(u.trim()), hp.trim()),
                None => (None, authority.trim()),
            };

            if let Some(u) = user_opt {
                if !is_valid_username(u) {
                    return Err("Invalid username in URI");
                }
            }

            let (host, port) = match host_port.rsplit_once(':') {
                Some((h, p_str)) if !h.contains(':') => {
                    let p = p_str.parse::<u16>().map_err(|_| "Invalid port in URI")?;
                    if !is_valid_port(p as u32) {
                        return Err("Port out of range in URI");
                    }
                    (h.trim(), p)
                }
                _ => (host_port, default_port),
            };

            if !is_valid_host(host) {
                return Err("Invalid host in URI");
            }

            let mut item = SessionItem::default();
            item.name = format!("{protocol}://{host}:{port}");
            item.session_type = protocol;
            item.host = host.to_string();
            item.port = port;
            item.user = user_opt.unwrap_or("").to_string();
            Ok(item)
        }
        "serial" => {
            let device_path = rest.split('?').next().unwrap_or(rest).trim();
            if !device_path.starts_with("/dev/") || device_path.contains("..") {
                return Err("Serial URI must point to a valid /dev/ device");
            }
            let mut item = SessionItem::default();
            item.name = format!("Serial ({device_path})");
            item.session_type = "serial".to_string();
            item.host = device_path.to_string();
            Ok(item)
        }
        _ => Err("Unsupported URI scheme"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_telnet_and_serial_uris() {
        let ssh = parse_connection_uri("ssh://devops@192.168.10.50:2222").unwrap();
        assert_eq!(ssh.session_type, "ssh");
        assert_eq!(ssh.user, "devops");
        assert_eq!(ssh.host, "192.168.10.50");
        assert_eq!(ssh.port, 2222);

        let serial = parse_connection_uri("serial:///dev/ttyUSB0?baud=115200").unwrap();
        assert_eq!(serial.session_type, "serial");
        assert_eq!(serial.host, "/dev/ttyUSB0");

        assert!(parse_connection_uri("ssh://bad;user@10.0.0.1").is_err());
        assert!(parse_connection_uri("serial:///etc/passwd").is_err());
    }
}
