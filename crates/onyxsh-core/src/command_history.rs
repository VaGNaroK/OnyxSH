//! Structured and enriched command history persistence using SQLite.
//! Port of `src/onyxsh/data/command_history_manager.py`.

use crate::platform::{config_dir, home_dir};
use rusqlite::{params, Connection, OptionalExtension, ToSql};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT_ID_SEQ: AtomicU64 = AtomicU64::new(1);

fn now_epoch_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn generate_entry_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = NEXT_ID_SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id() as u128;
    // Format as canonical 8-4-4-4-12 UUID layout for full compatibility with Python rows.
    let raw = nanos ^ (pid << 64) ^ ((seq as u128) << 96);
    let bytes = raw.to_be_bytes();
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandHistoryItem {
    pub id: String,
    pub command: String,
    pub cwd: String,
    pub host: String,
    pub session_name: String,
    pub exit_code: Option<i32>,
    pub duration_ms: i64,
    pub timestamp: f64,
    pub is_pinned: bool,
    pub execution_count: i64,
    pub last_executed: f64,
}

impl CommandHistoryItem {
    /// Human-readable relative elapsed time (`"5s"`, `"2m"`, `"3h"`, `"1d"`, `"2w"`).
    pub fn formatted_relative_time(&self) -> String {
        let ref_ts = if self.last_executed > 0.0 {
            self.last_executed
        } else {
            self.timestamp
        };
        let diff = (now_epoch_secs() - ref_ts).max(0.0) as u64;
        if diff < 60 {
            format!("{diff}s")
        } else if diff < 3600 {
            format!("{}m", diff / 60)
        } else if diff < 86_400 {
            format!("{}h", diff / 3600)
        } else if diff < 604_800 {
            format!("{}d", diff / 86_400)
        } else {
            format!("{}w", diff / 604_800)
        }
    }

    /// Human-readable command duration (`"250ms"`, `"1.5s"`, `"2m 10s"`).
    pub fn formatted_duration(&self) -> String {
        if self.duration_ms <= 0 {
            return String::new();
        }
        if self.duration_ms < 1000 {
            return format!("{}ms", self.duration_ms);
        }
        let secs = self.duration_ms as f64 / 1000.0;
        if secs < 60.0 {
            return format!("{secs:.1}s");
        }
        let mins = (secs / 60.0).floor() as u64;
        let rem = (secs % 60.0) as u64;
        format!("{mins}m {rem}s")
    }

    /// Simplified CWD replacing leading `localhost` prefix and user's home directory with `~`.
    pub fn display_cwd(&self) -> String {
        if self.cwd.is_empty() {
            return String::new();
        }
        let clean = if let Some(rest) = self.cwd.strip_prefix("localhost/") {
            format!("/{rest}")
        } else if let Some(rest) = self.cwd.strip_prefix("localhost") {
            rest.to_string()
        } else {
            self.cwd.clone()
        };
        let home = home_dir().to_string_lossy().into_owned();
        if clean == home {
            "~".to_string()
        } else if let Some(rest) = clean.strip_prefix(&format!("{home}/")) {
            format!("~/{rest}")
        } else {
            clean
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandStatSummary {
    pub command: String,
    pub count: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandHistoryStats {
    pub total_entries: i64,
    pub pinned_entries: i64,
    pub top_commands: Vec<CommandStatSummary>,
}

/// Scope options for [`CommandHistoryStore::clear_history`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryClearScope {
    NonPinned,
    Failed,
    Everything,
}

/// Thread-safe SQLite store compatible with `~/.config/onyxsh/command_history.db`.
pub struct CommandHistoryStore {
    db_path: PathBuf,
    lock: Mutex<()>,
}

impl CommandHistoryStore {
    pub fn open(db_path: impl Into<PathBuf>) -> rusqlite::Result<Self> {
        let store = Self {
            db_path: db_path.into(),
            lock: Mutex::new(()),
        };
        store.init_db()?;
        Ok(store)
    }

    pub fn open_default() -> rusqlite::Result<Self> {
        Self::open(config_dir().join("command_history.db"))
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        if let Some(parent) = self.db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&self.db_path)?;
        conn.busy_timeout(Duration::from_secs(10))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        Ok(conn)
    }

    fn init_db(&self) -> rusqlite::Result<()> {
        let _guard = self.lock.lock().expect("history store lock");
        let conn = self.connect()?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS command_history (
                id TEXT PRIMARY KEY,
                command TEXT NOT NULL,
                cwd TEXT NOT NULL DEFAULT '',
                host TEXT NOT NULL DEFAULT 'localhost',
                session_name TEXT NOT NULL DEFAULT '',
                exit_code INTEGER,
                duration_ms INTEGER DEFAULT 0,
                timestamp REAL NOT NULL,
                is_pinned INTEGER DEFAULT 0,
                execution_count INTEGER DEFAULT 1,
                last_executed REAL NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_cmd_history_timestamp ON command_history(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_cmd_history_last_executed ON command_history(last_executed DESC);
            CREATE INDEX IF NOT EXISTS idx_cmd_history_cwd ON command_history(cwd);
            CREATE INDEX IF NOT EXISTS idx_cmd_history_host ON command_history(host);
            CREATE INDEX IF NOT EXISTS idx_cmd_history_pinned ON command_history(is_pinned);
            CREATE INDEX IF NOT EXISTS idx_cmd_history_command ON command_history(command);
            ",
        )
    }

    /// Records a command; deduplicates by `(command, cwd, host)` and increments `execution_count`.
    pub fn record_command(
        &self,
        command: &str,
        cwd: &str,
        host: &str,
        session_name: &str,
        exit_code: Option<i32>,
        duration_ms: i64,
    ) -> rusqlite::Result<Option<CommandHistoryItem>> {
        let cleaned_cmd = command.trim();
        if cleaned_cmd.is_empty() {
            return Ok(None);
        }
        let host_val = if host.trim().is_empty() {
            "localhost"
        } else {
            host.trim()
        };
        let now = now_epoch_secs();

        let _guard = self.lock.lock().expect("history store lock");
        let conn = self.connect()?;

        let existing: Option<(String, i64, i64, f64)> = conn
            .query_row(
                "SELECT id, execution_count, is_pinned, timestamp FROM command_history WHERE command = ?1 AND cwd = ?2 AND host = ?3 LIMIT 1;",
                params![cleaned_cmd, cwd, host_val],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        if let Some((entry_id, count, pinned_int, ts)) = existing {
            let new_count = count + 1;
            conn.execute(
                "UPDATE command_history SET execution_count = ?1, last_executed = ?2, exit_code = ?3, duration_ms = ?4, session_name = ?5 WHERE id = ?6;",
                params![new_count, now, exit_code, duration_ms, session_name, entry_id],
            )?;
            Ok(Some(CommandHistoryItem {
                id: entry_id,
                command: cleaned_cmd.to_string(),
                cwd: cwd.to_string(),
                host: host_val.to_string(),
                session_name: session_name.to_string(),
                exit_code,
                duration_ms,
                timestamp: ts,
                is_pinned: pinned_int != 0,
                execution_count: new_count,
                last_executed: now,
            }))
        } else {
            let entry_id = generate_entry_id();
            conn.execute(
                "INSERT INTO command_history (id, command, cwd, host, session_name, exit_code, duration_ms, timestamp, is_pinned, execution_count, last_executed) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 1, ?9);",
                params![
                    entry_id,
                    cleaned_cmd,
                    cwd,
                    host_val,
                    session_name,
                    exit_code,
                    duration_ms,
                    now,
                    now
                ],
            )?;
            Ok(Some(CommandHistoryItem {
                id: entry_id,
                command: cleaned_cmd.to_string(),
                cwd: cwd.to_string(),
                host: host_val.to_string(),
                session_name: session_name.to_string(),
                exit_code,
                duration_ms,
                timestamp: now,
                is_pinned: false,
                execution_count: 1,
                last_executed: now,
            }))
        }
    }

    pub fn search_history(
        &self,
        query: &str,
        cwd: Option<&str>,
        host: Option<&str>,
        only_pinned: bool,
        limit: usize,
        offset: usize,
    ) -> rusqlite::Result<Vec<CommandHistoryItem>> {
        let _guard = self.lock.lock().expect("history store lock");
        let conn = self.connect()?;

        let mut conditions = Vec::new();
        let mut owned_strings: Vec<String> = Vec::new();
        let trimmed_q = query.trim();
        if !trimmed_q.is_empty() {
            conditions.push("command LIKE ?");
            owned_strings.push(format!("%{trimmed_q}%"));
        }
        if let Some(c) = cwd.filter(|s| !s.is_empty()) {
            conditions.push("cwd = ?");
            owned_strings.push(c.to_string());
        }
        if let Some(h) = host.filter(|s| !s.is_empty()) {
            conditions.push("host = ?");
            owned_strings.push(h.to_string());
        }
        if only_pinned {
            conditions.push("is_pinned = 1");
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            "SELECT id, command, cwd, host, session_name, exit_code, duration_ms, timestamp, is_pinned, execution_count, last_executed
             FROM command_history
             {where_clause}
             ORDER BY is_pinned DESC, last_executed DESC
             LIMIT ? OFFSET ?;"
        );

        let limit_i64 = limit.max(1) as i64;
        let offset_i64 = offset as i64;
        let mut param_refs: Vec<&dyn ToSql> = owned_strings.iter().map(|s| s as &dyn ToSql).collect();
        param_refs.push(&limit_i64);
        param_refs.push(&offset_i64);

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let pinned: i64 = row.get(8)?;
            Ok(CommandHistoryItem {
                id: row.get(0)?,
                command: row.get(1)?,
                cwd: row.get(2)?,
                host: row.get(3)?,
                session_name: row.get(4)?,
                exit_code: row.get(5)?,
                duration_ms: row.get(6)?,
                timestamp: row.get(7)?,
                is_pinned: pinned != 0,
                execution_count: row.get(9)?,
                last_executed: row.get(10)?,
            })
        })?;

        let mut items: Vec<CommandHistoryItem> = rows.collect::<Result<_, _>>()?;
        if !trimmed_q.is_empty() {
            let q_lower = trimmed_q.to_lowercase();
            items.sort_by(|a, b| {
                let key = |it: &CommandHistoryItem| {
                    let cmd_lower = it.command.to_lowercase();
                    let exact = u8::from(cmd_lower == q_lower);
                    let starts = u8::from(cmd_lower.starts_with(&q_lower));
                    let pinned = u8::from(it.is_pinned);
                    (exact, starts, pinned, it.last_executed.to_bits())
                };
                key(b).cmp(&key(a))
            });
        }
        Ok(items)
    }

    pub fn toggle_pin(&self, entry_id: &str) -> rusqlite::Result<Option<bool>> {
        let _guard = self.lock.lock().expect("history store lock");
        let conn = self.connect()?;
        let current: Option<i64> = conn
            .query_row(
                "SELECT is_pinned FROM command_history WHERE id = ?1;",
                params![entry_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(cur_val) = current else {
            return Ok(None);
        };
        let next_val = if cur_val == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE command_history SET is_pinned = ?1 WHERE id = ?2;",
            params![next_val, entry_id],
        )?;
        Ok(Some(next_val != 0))
    }

    pub fn delete_entry(&self, entry_id: &str) -> rusqlite::Result<usize> {
        let _guard = self.lock.lock().expect("history store lock");
        let conn = self.connect()?;
        conn.execute("DELETE FROM command_history WHERE id = ?1;", params![entry_id])
    }

    pub fn clear_history(&self, scope: HistoryClearScope) -> rusqlite::Result<usize> {
        let _guard = self.lock.lock().expect("history store lock");
        let conn = self.connect()?;
        match scope {
            HistoryClearScope::Everything => conn.execute("DELETE FROM command_history;", []),
            HistoryClearScope::Failed => conn.execute(
                "DELETE FROM command_history WHERE exit_code != 0 AND is_pinned = 0;",
                [],
            ),
            HistoryClearScope::NonPinned => {
                conn.execute("DELETE FROM command_history WHERE is_pinned = 0;", [])
            }
        }
    }

    pub fn get_stats(&self) -> rusqlite::Result<CommandHistoryStats> {
        let _guard = self.lock.lock().expect("history store lock");
        let conn = self.connect()?;
        let total_entries: i64 =
            conn.query_row("SELECT COUNT(*) FROM command_history;", [], |r| r.get(0))?;
        let pinned_entries: i64 = conn.query_row(
            "SELECT COUNT(*) FROM command_history WHERE is_pinned = 1;",
            [],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT command, SUM(execution_count) AS total_execs
             FROM command_history
             GROUP BY command
             ORDER BY total_execs DESC
             LIMIT 5;",
        )?;
        let top_commands = stmt
            .query_map([], |row| {
                Ok(CommandStatSummary {
                    command: row.get(0)?,
                    count: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(CommandHistoryStats {
            total_entries,
            pinned_entries,
            top_commands,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_history_sqlite_crud_dedup_pin_and_search() {
        let tmp = std::env::temp_dir().join(format!("onyxsh_hist_db_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let db_file = tmp.join("command_history.db");

        let store = CommandHistoryStore::open(&db_file).unwrap();
        assert_eq!(store.record_command("   ", "/tmp", "localhost", "", Some(0), 10).unwrap(), None);

        let item1 = store
            .record_command("systemctl status nginx", "/etc/nginx", "prod-web", "Prod", Some(0), 120)
            .unwrap()
            .unwrap();
        assert_eq!(item1.execution_count, 1);

        // Recording identical command + cwd + host increments execution_count
        let item1_again = store
            .record_command("systemctl status nginx", "/etc/nginx", "prod-web", "Prod", Some(0), 95)
            .unwrap()
            .unwrap();
        assert_eq!(item1_again.id, item1.id);
        assert_eq!(item1_again.execution_count, 2);

        let item2 = store
            .record_command("journalctl -xe", "/var/log", "prod-web", "Prod", Some(1), 450)
            .unwrap()
            .unwrap();
        assert_eq!(store.toggle_pin(&item2.id).unwrap(), Some(true));

        let pinned_only = store
            .search_history("", None, None, true, 50, 0)
            .unwrap();
        assert_eq!(pinned_only.len(), 1);
        assert_eq!(pinned_only[0].command, "journalctl -xe");

        let searched = store
            .search_history("systemctl", None, Some("prod-web"), false, 50, 0)
            .unwrap();
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].execution_count, 2);

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.pinned_entries, 1);
        assert_eq!(stats.top_commands[0].command, "systemctl status nginx");
        assert_eq!(stats.top_commands[0].count, 2);

        // Clearing NonPinned keeps pinned item2
        let removed = store.clear_history(HistoryClearScope::NonPinned).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.get_stats().unwrap().total_entries, 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

