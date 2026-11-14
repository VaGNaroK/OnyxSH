//! Configuration Backup, Restore, and SHA-256 Checksum Utilities
//! (`src/onyxsh/utils/backup.py` and `src/onyxsh/utils/checksum_utils.py`).

use serde::{Deserialize, Serialize};
use sevenz_rust::{compress_to_path_encrypted, decompress_file_with_password, Password};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Computes the lowercase hexadecimal SHA-256 hash of a byte slice.
pub fn compute_sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Computes the lowercase hexadecimal SHA-256 hash of a file on disk.
pub fn compute_sha256_file(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(compute_sha256_bytes(&bytes))
}

/// Verifies whether `path`'s SHA-256 matches `expected_hex` (case-insensitive).
pub fn verify_sha256_checksum(path: &Path, expected_hex: &str) -> io::Result<bool> {
    let actual = compute_sha256_file(path)?;
    Ok(actual.eq_ignore_ascii_case(expected_hex.trim()))
}

/// Portable JSON backup bundle containing OnyxSH configuration files and per-file SHA-256 digests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigBackupBundle {
    pub version: String,
    pub app_id: String,
    pub files: BTreeMap<String, String>,
    pub checksums: BTreeMap<String, String>,
}

///Tracked configuration filenames included in an OnyxSH backup.
pub const BACKUP_CONFIG_FILES: &[&str] = &[
    "settings.json",
    "sessions.json",
    "command_buttons.json",
    "ai_history.json",
    "window_state.json",
];

/// Creates a [`ConfigBackupBundle`] from `config_dir` and writes it atomically to `output_file`.
pub fn create_config_backup(config_dir: &Path, output_file: &Path) -> io::Result<ConfigBackupBundle> {
    let mut files = BTreeMap::new();
    let mut checksums = BTreeMap::new();

    for &name in BACKUP_CONFIG_FILES {
        let p = config_dir.join(name);
        if p.is_file() {
            let content = fs::read_to_string(&p)?;
            let sha = compute_sha256_bytes(content.as_bytes());
            files.insert(name.to_string(), content);
            checksums.insert(name.to_string(), sha);
        }
    }

    let bundle = ConfigBackupBundle {
        version: crate::APP_VERSION.to_string(),
        app_id: crate::APP_ID.to_string(),
        files,
        checksums,
    };

    if let Some(parent) = output_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(&bundle).map_err(io::Error::other)?;
    let tmp = output_file.with_extension("onyxbackup.tmp");
    fs::write(&tmp, serialized)?;
    fs::rename(&tmp, output_file)?;

    Ok(bundle)
}

/// Validates all per-file SHA-256 checksums inside a [`ConfigBackupBundle`] and restores them into `target_config_dir`.
pub fn restore_config_backup(backup_file: &Path, target_config_dir: &Path) -> io::Result<usize> {
    let raw = fs::read_to_string(backup_file)?;
    let bundle: ConfigBackupBundle = serde_json::from_str(&raw).map_err(io::Error::other)?;

    // 1. Verify integrity of all files before writing anything
    for (name, content) in &bundle.files {
        if !BACKUP_CONFIG_FILES.contains(&name.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Untrusted file name in backup bundle: {name}"),
            ));
        }
        let expected_sha = bundle.checksums.get(name).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Missing SHA-256 checksum for {name}"),
            )
        })?;
        let actual_sha = compute_sha256_bytes(content.as_bytes());
        if !actual_sha.eq_ignore_ascii_case(expected_sha) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("SHA-256 mismatch for {name}: backup is corrupted or tampered"),
            ));
        }
    }

    // 2. Restore atomically
    fs::create_dir_all(target_config_dir)?;
    let mut restored_count = 0usize;
    for (name, content) in &bundle.files {
        let dest = target_config_dir.join(name);
        let tmp = dest.with_extension("restore.tmp");
        fs::write(&tmp, content)?;
        fs::rename(&tmp, &dest)?;
        restored_count += 1;
    }

    Ok(restored_count)
}

/// Configuration files included in password-protected `.7z` archives (compatible with Python OnyxSH).
pub const ENCRYPTED_BACKUP_FILES: &[&str] = &[
    "settings.json",
    "sessions.json",
    "state.json",
    "window_state.json",
    "tunnels.json",
    "command_buttons.json",
    "custom_schemes.json",
    "ai_history.json",
    "ai_prompts.json",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EncryptedBackupReport {
    pub restored_files: Vec<String>,
    pub passwords: BTreeMap<String, String>,
}

fn make_temp_staging_dir(prefix: &str) -> io::Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), seq));
    if path.exists() {
        fs::remove_dir_all(&path)?;
    }
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Creates a password-protected `.7z` backup compatible with Python `py7zr` backups
/// (storing primary config JSON files, `layouts/`, and optional `passwords.json`).
pub fn create_encrypted_7z_backup(
    config_dir: &Path,
    output_7z_file: &Path,
    password: &str,
    passwords: Option<&BTreeMap<String, String>>,
) -> io::Result<()> {
    if password.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Backup password must not be empty",
        ));
    }

    let staging = make_temp_staging_dir("onyxsh_7z_stage")?;
    let result = (|| -> io::Result<()> {
        for &name in ENCRYPTED_BACKUP_FILES {
            let src = config_dir.join(name);
            if src.is_file() {
                fs::copy(&src, staging.join(name))?;
            }
        }

        let layouts_src = config_dir.join("layouts");
        if layouts_src.is_dir() {
            copy_dir_recursive(&layouts_src, &staging.join("layouts"))?;
        }

        if let Some(pw_map) = passwords.filter(|m| !m.is_empty()) {
            let pw_json = serde_json::to_string_pretty(pw_map).map_err(io::Error::other)?;
            fs::write(staging.join("passwords.json"), pw_json)?;
        }

        if let Some(parent) = output_7z_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_out = output_7z_file.with_extension("7z.tmp");
        compress_to_path_encrypted(&staging, &tmp_out, Password::from(password))
            .map_err(|e| io::Error::other(format!("7z encryption failed: {e}")))?;
        fs::rename(&tmp_out, output_7z_file)?;
        Ok(())
    })();

    let _ = fs::remove_dir_all(&staging);
    result
}

/// Restores a password-protected `.7z` backup into `target_config_dir` and returns any extracted
/// `passwords.json` entries so the caller can persist them into the Keyring.
pub fn restore_encrypted_7z_backup(
    source_7z_file: &Path,
    target_config_dir: &Path,
    password: &str,
) -> io::Result<EncryptedBackupReport> {
    if password.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Backup password must not be empty",
        ));
    }

    let staging = make_temp_staging_dir("onyxsh_7z_restore")?;
    let result = (|| -> io::Result<EncryptedBackupReport> {
        decompress_file_with_password(source_7z_file, &staging, Password::from(password))
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("7z decryption failed (wrong password or corrupt archive): {e}"),
                )
            })?;

        fs::create_dir_all(target_config_dir)?;
        let mut report = EncryptedBackupReport::default();

        for entry in fs::read_dir(&staging)? {
            let entry = entry?;
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy();
            let src_path = entry.path();

            if fname_str == "passwords.json" && src_path.is_file() {
                let raw = fs::read_to_string(&src_path)?;
                report.passwords = serde_json::from_str(&raw).map_err(io::Error::other)?;
                continue;
            }

            if fname_str == "layouts" && src_path.is_dir() {
                let target_layouts = target_config_dir.join("layouts");
                if target_layouts.exists() {
                    let _ = fs::remove_dir_all(&target_layouts);
                }
                copy_dir_recursive(&src_path, &target_layouts)?;
                report.restored_files.push("layouts/".to_string());
                continue;
            }

            if src_path.is_file() && ENCRYPTED_BACKUP_FILES.contains(&fname_str.as_ref()) {
                let dest = target_config_dir.join(&fname);
                let tmp = dest.with_extension("restore.tmp");
                fs::copy(&src_path, &tmp)?;
                fs::rename(&tmp, &dest)?;
                report.restored_files.push(fname_str.into_owned());
            }
        }

        report.restored_files.sort();
        Ok(report)
    })();

    let _ = fs::remove_dir_all(&staging);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backup_create_restore_and_tamper_detection() {
        let tmp = std::env::temp_dir().join(format!("onyxsh_backup_test_{}", std::process::id()));
        let src_dir = tmp.join("src_cfg");
        let dst_dir = tmp.join("dst_cfg");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&src_dir).unwrap();

        fs::write(src_dir.join("settings.json"), r#"{"font":"Monospace 13"}"#).unwrap();
        fs::write(src_dir.join("sessions.json"), r#"{"sessions":[],"folders":[]}"#).unwrap();

        let backup_path = tmp.join("backup.onyxsh.json");
        let bundle = create_config_backup(&src_dir, &backup_path).unwrap();
        assert_eq!(bundle.files.len(), 2);

        let restored = restore_config_backup(&backup_path, &dst_dir).unwrap();
        assert_eq!(restored, 2);
        assert_eq!(
            fs::read_to_string(dst_dir.join("settings.json")).unwrap(),
            r#"{"font":"Monospace 13"}"#
        );

        // Tamper with the backup content without updating its SHA-256
        let mut tampered = bundle.clone();
        tampered
            .files
            .insert("settings.json".to_string(), r#"{"font":"Hacked"}"#.to_string());
        let tampered_path = tmp.join("tampered.onyxsh.json");
        fs::write(&tampered_path, serde_json::to_string(&tampered).unwrap()).unwrap();
        assert!(restore_config_backup(&tampered_path, &dst_dir).is_err());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_encrypted_7z_backup_roundtrip_wrong_password_and_empty_password() {
        let tmp = std::env::temp_dir().join(format!("onyxsh_7z_test_{}", std::process::id()));
        let src_dir = tmp.join("src");
        let dst_dir = tmp.join("dst");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(src_dir.join("layouts")).unwrap();

        fs::write(src_dir.join("settings.json"), r#"{"color_scheme":"onyx_dark"}"#).unwrap();
        fs::write(src_dir.join("state.json"), r#"{"tabs":[]}"#).unwrap();
        fs::write(src_dir.join("layouts").join("devops.json"), r#"{"tabs":[1]}"#).unwrap();

        let mut passwords = BTreeMap::new();
        passwords.insert("prod-db".to_string(), "super-secret-pass".to_string());

        let archive = tmp.join("backup.7z");
        // Empty password must be rejected
        assert!(create_encrypted_7z_backup(&src_dir, &archive, "", Some(&passwords)).is_err());

        create_encrypted_7z_backup(&src_dir, &archive, "MasterPass#2026", Some(&passwords)).unwrap();
        assert!(archive.is_file());

        // Wrong password must fail restore
        assert!(restore_encrypted_7z_backup(&archive, &dst_dir, "WrongPass").is_err());

        // Correct password restores files, layouts/, and passwords map
        let report =
            restore_encrypted_7z_backup(&archive, &dst_dir, "MasterPass#2026").unwrap();
        assert!(report.restored_files.contains(&"settings.json".to_string()));
        assert!(report.restored_files.contains(&"state.json".to_string()));
        assert!(report.restored_files.contains(&"layouts/".to_string()));
        assert_eq!(
            report.passwords.get("prod-db").map(String::as_str),
            Some("super-secret-pass")
        );
        assert_eq!(
            fs::read_to_string(dst_dir.join("layouts").join("devops.json")).unwrap(),
            r#"{"tabs":[1]}"#
        );

        let _ = fs::remove_dir_all(&tmp);
    }
}
