//! Proactive SSH Connection Health Monitor & Latency (RTT) Tracker.
//! Port of `src/onyxsh/terminal/ssh_health_monitor.py`.

use onyxsh_core::i18n::{tr, tr_fmt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SshHealthStatus {
    Unknown,
    Healthy,      // RTT < 150ms
    Degraded,     // 150ms <= RTT < 350ms
    Poor,         // RTT >= 350ms
    Unreachable,  // Socket down / timeout
    Reconnecting, // Auto-reconnect countdown active
}

impl SshHealthStatus {
    /// Classifies an RTT measurement in milliseconds into a health status.
    pub fn from_rtt_ms(rtt_ms: f64) -> Self {
        if rtt_ms < 150.0 {
            Self::Healthy
        } else if rtt_ms < 350.0 {
            Self::Degraded
        } else {
            Self::Poor
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SshHealthRecord {
    pub terminal_id: u64,
    pub session_name: String,
    pub host: String,
    pub port: u16,
    pub status: SshHealthStatus,
    pub rtt_ms: Option<f64>,
    pub consecutive_failures: u32,
    pub error_message: Option<String>,
    pub is_auto_reconnecting: bool,
    pub countdown_seconds: u32,
    pub reconnect_attempt: u32,
    pub max_reconnect_attempts: u32,
}

impl SshHealthRecord {
    pub fn new(terminal_id: u64, session_name: &str, host: &str, port: u16) -> Self {
        Self {
            terminal_id,
            session_name: session_name.to_string(),
            host: host.to_string(),
            port,
            status: SshHealthStatus::Unknown,
            rtt_ms: None,
            consecutive_failures: 0,
            error_message: None,
            is_auto_reconnecting: false,
            countdown_seconds: 0,
            reconnect_attempt: 0,
            max_reconnect_attempts: 5,
        }
    }

    pub fn badge_label(&self) -> String {
        if self.is_auto_reconnecting && self.countdown_seconds > 0 {
            return tr_fmt(
                "🔄 {secs}s",
                &[("secs", &self.countdown_seconds.to_string())],
            );
        }
        match (self.status, self.rtt_ms) {
            (SshHealthStatus::Healthy, Some(rtt)) => {
                tr_fmt("🟢 {rtt}ms", &[("rtt", &(rtt as u64).to_string())])
            }
            (SshHealthStatus::Degraded, Some(rtt)) => {
                tr_fmt("🟡 {rtt}ms", &[("rtt", &(rtt as u64).to_string())])
            }
            (SshHealthStatus::Poor, Some(rtt)) => {
                tr_fmt("🟠 {rtt}ms", &[("rtt", &(rtt as u64).to_string())])
            }
            (SshHealthStatus::Unreachable, _) => tr("🔴 Inalcançável"),
            (SshHealthStatus::Reconnecting, _) => tr("🔄 Reconectando..."),
            _ => tr("⚪ Verificando..."),
        }
    }

    pub fn css_class(&self) -> &'static str {
        if self.is_auto_reconnecting {
            return "latency-reconnecting";
        }
        match self.status {
            SshHealthStatus::Healthy => "latency-healthy",
            SshHealthStatus::Degraded => "latency-degraded",
            SshHealthStatus::Poor => "latency-poor",
            SshHealthStatus::Unreachable => "latency-unreachable",
            SshHealthStatus::Reconnecting => "latency-reconnecting",
            SshHealthStatus::Unknown => "latency-unknown",
        }
    }

    pub fn record_probe_success(&mut self, rtt_ms: f64) {
        self.rtt_ms = Some(rtt_ms);
        self.status = SshHealthStatus::from_rtt_ms(rtt_ms);
        self.consecutive_failures = 0;
        self.error_message = None;
        self.is_auto_reconnecting = false;
        self.countdown_seconds = 0;
        self.reconnect_attempt = 0;
    }

    pub fn record_probe_failure(&mut self, error: &str, auto_reconnect: bool, delay_secs: u32) {
        self.rtt_ms = None;
        self.consecutive_failures += 1;
        self.error_message = Some(error.to_string());

        if auto_reconnect && self.reconnect_attempt < self.max_reconnect_attempts {
            self.reconnect_attempt += 1;
            self.is_auto_reconnecting = true;
            self.countdown_seconds = delay_secs;
            self.status = SshHealthStatus::Reconnecting;
        } else {
            self.is_auto_reconnecting = false;
            self.status = SshHealthStatus::Unreachable;
        }
    }
}

#[derive(Debug, Default)]
pub struct SshHealthRegistry {
    records: HashMap<u64, SshHealthRecord>,
}

impl SshHealthRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, terminal_id: u64, session_name: &str, host: &str, port: u16) {
        self.records.insert(
            terminal_id,
            SshHealthRecord::new(terminal_id, session_name, host, port),
        );
    }

    pub fn unregister(&mut self, terminal_id: u64) -> Option<SshHealthRecord> {
        self.records.remove(&terminal_id)
    }

    pub fn get(&self, terminal_id: u64) -> Option<&SshHealthRecord> {
        self.records.get(&terminal_id)
    }

    pub fn get_mut(&mut self, terminal_id: u64) -> Option<&mut SshHealthRecord> {
        self.records.get_mut(&terminal_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_health_rtt_classification_and_reconnect() {
        let mut rec = SshHealthRecord::new(10, "Web-Prod", "10.0.0.5", 22);
        assert_eq!(rec.css_class(), "latency-unknown");

        rec.record_probe_success(42.0);
        assert_eq!(rec.status, SshHealthStatus::Healthy);
        assert_eq!(rec.badge_label(), tr_fmt("🟢 {rtt}ms", &[("rtt", "42")]));
        assert_eq!(rec.css_class(), "latency-healthy");

        rec.record_probe_success(220.0);
        assert_eq!(rec.status, SshHealthStatus::Degraded);
        assert_eq!(rec.badge_label(), tr_fmt("🟡 {rtt}ms", &[("rtt", "220")]));

        rec.record_probe_failure("Connection timed out", true, 5);
        assert_eq!(rec.status, SshHealthStatus::Reconnecting);
        assert!(rec.is_auto_reconnecting);
        assert_eq!(rec.reconnect_attempt, 1);
        assert_eq!(rec.badge_label(), tr_fmt("🔄 {secs}s", &[("secs", "5")]));
    }
}
