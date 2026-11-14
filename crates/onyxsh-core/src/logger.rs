//! Structured logging subsystem (`tracing` port of `src/onyxsh/utils/logger.py`).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once};
use tracing_subscriber::{fmt, EnvFilter};

static INIT_LOGGER: Once = Once::new();
const MAX_LOG_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone)]
struct TeeLogWriter {
    file: Option<Arc<Mutex<(PathBuf, File)>>>,
}

impl TeeLogWriter {
    fn open(path: &Path) -> Option<Self> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(meta) = fs::metadata(path) {
            if meta.len() >= MAX_LOG_FILE_BYTES {
                let rotated = path.with_extension("log.1");
                let _ = fs::rename(path, rotated);
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()?;
        Some(Self {
            file: Some(Arc::new(Mutex::new((path.to_path_buf(), file)))),
        })
    }

    fn console_only() -> Self {
        Self { file: None }
    }
}

struct TeeGuard {
    file: Option<Arc<Mutex<(PathBuf, File)>>>,
}

impl Write for TeeGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut stderr = io::stderr().lock();
        let _ = stderr.write_all(buf);
        if let Some(ref arc) = self.file {
            if let Ok(mut guard) = arc.lock() {
                let line = String::from_utf8_lossy(buf);
                let safe = sanitize_log_text(&line);
                let _ = guard.1.write_all(safe.as_bytes());
                if guard.1.metadata().map(|m| m.len()).unwrap_or(0) >= MAX_LOG_FILE_BYTES {
                    let path = guard.0.clone();
                    let rotated = path.with_extension("log.1");
                    let _ = fs::rename(&path, rotated);
                    if let Ok(new_file) = OpenOptions::new().create(true).append(true).open(&path) {
                        guard.1 = new_file;
                    }
                }
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = io::stderr().flush();
        if let Some(ref arc) = self.file {
            if let Ok(mut guard) = arc.lock() {
                let _ = guard.1.flush();
            }
        }
        Ok(())
    }
}

impl<'a> fmt::MakeWriter<'a> for TeeLogWriter {
    type Writer = TeeGuard;

    fn make_writer(&'a self) -> Self::Writer {
        TeeGuard {
            file: self.file.clone(),
        }
    }
}

/// Redacts sensitive credential tokens, API keys, and passwords from log output.
pub fn sanitize_log_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.split_inclusive('\n') {
        let mut redacted = line.to_string();
        for marker in ["SSHPASS=", "Authorization: Bearer ", "api_key=", "password="] {
            if let Some(idx) = redacted.find(marker) {
                let start = idx + marker.len();
                let end = redacted[start..]
                    .find(|c: char| c.is_whitespace() || c == '&' || c == '"' || c == '\'')
                    .map(|off| start + off)
                    .unwrap_or_else(|| redacted.trim_end_matches(['\r', '\n']).len());
                if end > start {
                    redacted.replace_range(start..end, "***");
                }
            }
        }
        out.push_str(&redacted);
    }
    out
}

/// Initializes the global thread-safe tracing subscriber (console only).
pub fn init_logger(default_level: &str) {
    init_logger_with_file(default_level, None);
}

/// Initializes the global thread-safe tracing subscriber with optional rotating log file output.
pub fn init_logger_with_file(default_level: &str, log_file: Option<&Path>) {
    INIT_LOGGER.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(default_level));
        let writer = log_file
            .and_then(TeeLogWriter::open)
            .unwrap_or_else(TeeLogWriter::console_only);

        let _ = fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_thread_ids(false)
            .with_ansi(log_file.is_none())
            .with_writer(writer)
            .try_init();
    });
}

/// Default log file path (`~/.config/onyxsh/logs/onyxsh.log`).
pub fn default_log_file_path() -> PathBuf {
    crate::platform::config_dir().join("logs").join("onyxsh.log")
}

/// Reads the last `max_lines` sanitized lines from `log_file`.
pub fn read_recent_sanitized_logs(log_file: &Path, max_lines: usize) -> Vec<String> {
    let Ok(content) = fs::read_to_string(log_file) else {
        return Vec::new();
    };
    let lines: Vec<String> = content.lines().map(sanitize_log_text).collect();
    if lines.len() <= max_lines {
        lines
    } else {
        lines[lines.len() - max_lines..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_idempotent_initialization() {
        init_logger("info");
        init_logger("debug");
        tracing::info!("Logger initialized cleanly");
    }

    #[test]
    fn test_log_sanitization_and_recent_reader() {
        let raw = "Connecting SSHPASS=supersecret123 host=10.0.0.1\nAuthorization: Bearer sk-abc123456 ok\n";
        let sanitized = sanitize_log_text(raw);
        assert!(!sanitized.contains("supersecret123"));
        assert!(!sanitized.contains("sk-abc123456"));
        assert!(sanitized.contains("SSHPASS=***"));
        assert!(sanitized.contains("Authorization: Bearer ***"));

        let tmp = std::env::temp_dir().join(format!("onyxsh_log_test_{}.log", std::process::id()));
        fs::write(&tmp, "line1\nline2\npassword=my_secret\n").unwrap();
        let recent = read_recent_sanitized_logs(&tmp, 2);
        assert_eq!(recent, vec!["line2".to_string(), "password=***".to_string()]);
        let _ = fs::remove_file(tmp);
    }
}
