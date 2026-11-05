//! Append-only JSONL Audit Logger and Atomic Rollback Manager.
//! Port of `src/onyxsh/agent/audit.py`.

use crate::models::AuditRecord;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug)]
pub struct AuditLogger {
    log_path: PathBuf,
    write_lock: Mutex<()>,
}

impl AuditLogger {
    pub fn new(log_path: PathBuf) -> std::io::Result<Self> {
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(Self {
            log_path,
            write_lock: Mutex::new(()),
        })
    }

    /// Appends an `AuditRecord` in JSONL format.
    pub fn append(&self, record: &AuditRecord) -> std::io::Result<()> {
        let line = serde_json::to_string(record)?;
        let _guard = self.write_lock.lock().unwrap();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        writeln!(file, "{line}")?;
        file.sync_data()?;
        Ok(())
    }

    /// Reads recorded audits from newest to oldest, up to `limit`.
    pub fn get_records(&self, limit: usize) -> Vec<AuditRecord> {
        let _guard = self.write_lock.lock().unwrap();
        let Ok(content) = fs::read_to_string(&self.log_path) else {
            return Vec::new();
        };
        let mut records: Vec<AuditRecord> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        records.reverse();
        records.truncate(limit);
        records
    }

    /// Creates a backup of `target_path` inside `backup_dir` and returns the backup path.
    pub fn create_file_backup(target_path: &Path, backup_dir: &Path, record_id: &str) -> std::io::Result<PathBuf> {
        fs::create_dir_all(backup_dir)?;
        let file_name = target_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let backup_path = backup_dir.join(format!("{record_id}_{file_name}.bak"));
        fs::copy(target_path, &backup_path)?;
        Ok(backup_path)
    }

    /// Atomically rolls back `target_path` from `backup_path`.
    pub fn rollback_file(backup_path: &Path, target_path: &Path) -> std::io::Result<()> {
        let tmp_restore = target_path.with_extension("rollback.tmp");
        fs::copy(backup_path, &tmp_restore)?;
        fs::rename(&tmp_restore, target_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RiskLevel;

    #[test]
    fn test_audit_logger_append_read_and_atomic_rollback() {
        let tmp_dir = std::env::temp_dir().join(format!("onyxsh_audit_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        let target_file = tmp_dir.join("config.conf");
        fs::write(&target_file, "PORT=8080\n").unwrap();

        let backup_path =
            AuditLogger::create_file_backup(&target_file, &tmp_dir.join("backups"), "rec_01")
                .unwrap();

        // Mutate target file
        fs::write(&target_file, "PORT=9090\n").unwrap();

        // Log audit record
        let logger = AuditLogger::new(tmp_dir.join("audit.jsonl")).unwrap();
        logger
            .append(&AuditRecord {
                id: "rec_01".to_string(),
                timestamp: "2026-11-04T12:00:00Z".to_string(),
                tool: "fs.write_staged_file".to_string(),
                command_or_path: target_file.to_string_lossy().to_string(),
                risk_level: RiskLevel::UserWrite,
                exit_code: 0,
                backup_path: Some(backup_path.to_string_lossy().to_string()),
                target_path: Some(target_file.to_string_lossy().to_string()),
            })
            .unwrap();

        let records = logger.get_records(10);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, "rec_01");

        // Rollback and verify original content restored
        AuditLogger::rollback_file(&backup_path, &target_file).unwrap();
        assert_eq!(fs::read_to_string(&target_file).unwrap(), "PORT=8080\n");

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
