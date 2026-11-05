//! Configuration Backup, Restore, and SHA-256 Checksum Utilities
//! (`src/onyxsh/utils/backup.py` and `src/onyxsh/utils/checksum_utils.py`).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

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
}
