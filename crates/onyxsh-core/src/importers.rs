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

/// Summary report for SecureCRT directory import (`import_sessions_from_securecrt_directory`).
#[derive(Debug, Clone, Default)]
pub struct SecureCrtImportReport {
    pub imported_sessions: usize,
    pub created_folders: usize,
    pub warnings: Vec<String>,
}

/// Parses `S:"Key"=Value` and optional `D:"[SSH2] Port"=xxxxxxxx` pairs from a SecureCRT `.ini` file.
pub fn parse_securecrt_ini_str(content: &str) -> std::collections::HashMap<String, String> {
    let mut result = std::collections::HashMap::new();
    for raw_line in content.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if let Some(rest) = line.strip_prefix("S:\"") {
            if let Some(idx) = rest.find("\"=") {
                let key = &rest[..idx];
                let mut val = rest[idx + 2..].trim();
                if val.len() >= 2 && val.starts_with('"') && val.ends_with('"') {
                    val = &val[1..val.len() - 1];
                }
                result.insert(key.to_string(), val.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("D:\"") {
            if let Some(idx) = rest.find("\"=") {
                let key = &rest[..idx];
                let val = rest[idx + 2..].trim();
                result.insert(key.to_string(), val.to_string());
            }
        }
    }
    result
}

fn parse_securecrt_ini_file(path: &std::path::Path) -> std::collections::HashMap<String, String> {
    let Ok(bytes) = std::fs::read(path) else {
        return std::collections::HashMap::new();
    };
    let text = String::from_utf8(bytes)
        .unwrap_or_else(|e| e.into_bytes().into_iter().map(|b| b as char).collect());
    parse_securecrt_ini_str(&text)
}

fn generate_unique_session_name(
    base_name: &str,
    existing: &std::collections::HashSet<String>,
) -> String {
    if !existing.contains(base_name) {
        return base_name.to_string();
    }
    let mut counter = 2usize;
    loop {
        let candidate = format!("{} ({})", base_name, counter);
        if !existing.contains(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

/// Imports SecureCRT folders & `.ini` sessions into `SessionsStorageData` preserving hierarchy and `02:` passwords.
pub fn import_securecrt_directory(
    root_path: &std::path::Path,
    storage: &mut crate::sessions::SessionsStorageData,
) -> Result<SecureCrtImportReport, String> {
    if !root_path.exists() || !root_path.is_dir() {
        return Err(format!("SecureCRT folder not found: {}", root_path.display()));
    }

    let mut report = SecureCrtImportReport::default();
    let mut visited = std::collections::HashSet::<std::path::PathBuf>::new();

    fn walk_dir(
        dir: &std::path::Path,
        folder_path: &str,
        storage: &mut crate::sessions::SessionsStorageData,
        report: &mut SecureCrtImportReport,
        visited: &mut std::collections::HashSet<std::path::PathBuf>,
    ) {
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        if !visited.insert(canonical) {
            return;
        }

        let folder_ini = parse_securecrt_ini_file(&dir.join("__FolderData__.ini"));
        let listed_folders: Vec<String> = folder_ini
            .get("Folder List")
            .map(|s| s.split(':').filter(|i| !i.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();
        let listed_sessions: Vec<String> = folder_ini
            .get("Session List")
            .map(|s| s.split(':').filter(|i| !i.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();

        let mut child_dirs = std::collections::BTreeMap::<String, std::path::PathBuf>::new();
        let mut ini_files = std::collections::BTreeMap::<String, std::path::PathBuf>::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        child_dirs.insert(name.to_string(), p);
                    }
                } else if p.is_file() {
                    let is_ini = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("ini"))
                        .unwrap_or(false);
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if is_ini && name != "__FolderData__.ini" {
                        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                            ini_files.insert(stem.to_string(), p);
                        }
                    }
                }
            }
        }

        let mut ordered_ini = Vec::new();
        for stem in &listed_sessions {
            if let Some(p) = ini_files.remove(stem) {
                ordered_ini.push(p);
            } else if let Some(key) = ini_files
                .keys()
                .find(|k| k.eq_ignore_ascii_case(stem))
                .cloned()
            {
                if let Some(p) = ini_files.remove(&key) {
                    ordered_ini.push(p);
                }
            } else {
                report.warnings.push(format!(
                    "Listed SecureCRT session '{}' was not found under {}",
                    stem,
                    dir.display()
                ));
            }
        }
        ordered_ini.extend(ini_files.into_values());

        let mut existing_names: std::collections::HashSet<String> = storage
            .sessions
            .iter()
            .filter(|s| s.folder_path == folder_path)
            .map(|s| s.name.clone())
            .collect();

        let mut ensured_folder = false;
        for ini_path in ordered_ini {
            let data = parse_securecrt_ini_file(&ini_path);
            let hostname = data.get("Hostname").map(|s| s.trim()).unwrap_or("");
            if hostname.is_empty() {
                report.warnings.push(format!(
                    "Skipped '{}': missing Hostname field.",
                    ini_path.file_name().unwrap_or_default().to_string_lossy()
                ));
                continue;
            }
            if !folder_path.is_empty() && !ensured_folder {
                ensured_folder = true;
                let parts: Vec<&str> = folder_path.trim_start_matches('/').split('/').collect();
                let mut cur_path = String::new();
                for part in parts {
                    if part.is_empty() {
                        continue;
                    }
                    let parent = cur_path.clone();
                    cur_path = format!("{}/{}", cur_path, part);
                    if !storage.folders.iter().any(|f| f.path == cur_path) {
                        storage.folders.push(crate::sessions::SessionFolder {
                            name: part.to_string(),
                            path: cur_path.clone(),
                            parent_path: parent,
                            ..crate::sessions::SessionFolder::default()
                        });
                        report.created_folders += 1;
                    }
                }
            }

            let stem = ini_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Session");
            let session_name = generate_unique_session_name(stem, &existing_names);
            let username = data.get("Username").map(|s| s.trim()).unwrap_or("");
            let password_v2 = data.get("Password V2").map(|s| s.trim()).unwrap_or("");
            let port = data
                .get("[SSH2] Port")
                .and_then(|hex| u16::from_str_radix(hex.trim(), 16).ok())
                .filter(|&p| p > 0)
                .unwrap_or(22);

            let mut item = crate::sessions::SessionItem {
                name: session_name.clone(),
                session_type: "ssh".to_string(),
                host: hostname.to_string(),
                port,
                user: username.to_string(),
                folder_path: folder_path.to_string(),
                source: "securecrt".to_string(),
                ..crate::sessions::SessionItem::default()
            };
            if !password_v2.is_empty() {
                item.auth_type = "password".to_string();
                item.auth_value = password_v2.to_string();
            }
            if item.validation_errors().is_empty() {
                existing_names.insert(session_name);
                storage.sessions.push(item);
                report.imported_sessions += 1;
            }
        }

        let mut ordered_dirs = Vec::new();
        for fname in &listed_folders {
            if let Some(d) = child_dirs.remove(fname) {
                ordered_dirs.push(d);
            } else {
                report.warnings.push(format!(
                    "Listed SecureCRT folder '{}' was not found under {}",
                    fname,
                    dir.display()
                ));
            }
        }
        ordered_dirs.extend(child_dirs.into_values());

        for child in ordered_dirs {
            let child_name = child.file_name().unwrap_or_default().to_string_lossy();
            let child_folder_path = if folder_path.is_empty() {
                format!("/{}", child_name)
            } else {
                format!("{}/{}", folder_path, child_name)
            };
            walk_dir(&child, &child_folder_path, storage, report, visited);
        }
    }

    walk_dir(root_path, "", storage, &mut report, &mut visited);
    Ok(report)
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

    #[test]
    fn test_import_securecrt_directory_hierarchy() {
        let base = std::env::temp_dir().join(format!("onyxsh_scrt_{}", std::process::id()));
        let prod = base.join("Prod");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&prod).unwrap();

        std::fs::write(
            base.join("__FolderData__.ini"),
            "S:\"Folder List\"=Prod\nS:\"Session List\"=Bastion\n",
        )
        .unwrap();
        std::fs::write(
            base.join("Bastion.ini"),
            "S:\"Hostname\"=bastion.corp\nS:\"Username\"=ops\nD:\"[SSH2] Port\"=000008ae\n",
        )
        .unwrap();
        std::fs::write(
            prod.join("DB1.ini"),
            "S:\"Hostname\"=10.10.1.2\nS:\"Username\"=dba\nS:\"Password V2\"=02:abcd\n",
        )
        .unwrap();

        let mut storage = crate::sessions::SessionsStorageData::default();
        let report = import_securecrt_directory(&base, &mut storage).unwrap();
        assert_eq!(report.imported_sessions, 2);
        assert_eq!(report.created_folders, 1);
        assert_eq!(storage.folders.len(), 1);
        assert_eq!(storage.folders[0].path, "/Prod");

        let bastion = storage.sessions.iter().find(|s| s.name == "Bastion").unwrap();
        assert_eq!(bastion.port, 2222);
        assert_eq!(bastion.folder_path, "");
        assert_eq!(bastion.source, "securecrt");

        let db1 = storage.sessions.iter().find(|s| s.name == "DB1").unwrap();
        assert_eq!(db1.folder_path, "/Prod");
        assert_eq!(db1.auth_type, "password");
        assert_eq!(db1.auth_value, "02:abcd");

        let _ = std::fs::remove_dir_all(&base);
    }
}
