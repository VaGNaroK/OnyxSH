//! Asynchronous File Transfer Queue models and progress/ETA calculators.
//! Port of `src/onyxsh/filemanager/transfer_manager.py`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    Queued,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransferItem {
    pub id: String,
    pub filename: String,
    pub source_path: String,
    pub destination_path: String,
    pub direction: TransferDirection,
    pub status: TransferStatus,
    pub total_bytes: u64,
    pub transferred_bytes: u64,
    pub speed_bps: f64,
    pub error_message: Option<String>,
}

impl TransferItem {
    pub fn progress_fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            return if self.status == TransferStatus::Completed {
                1.0
            } else {
                0.0
            };
        }
        (self.transferred_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
    }

    pub fn progress_percent(&self) -> u8 {
        (self.progress_fraction() * 100.0).round() as u8
    }

    pub fn eta_seconds(&self) -> Option<u64> {
        if self.status != TransferStatus::InProgress
            || self.speed_bps <= 1.0
            || self.transferred_bytes >= self.total_bytes
        {
            return None;
        }
        let remaining = self.total_bytes - self.transferred_bytes;
        Some((remaining as f64 / self.speed_bps).ceil() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transfer_item_progress_and_eta() {
        let item = TransferItem {
            id: "tx_1".to_string(),
            filename: "backup.tar.gz".to_string(),
            source_path: "/local/backup.tar.gz".to_string(),
            destination_path: "/remote/backup.tar.gz".to_string(),
            direction: TransferDirection::Upload,
            status: TransferStatus::InProgress,
            total_bytes: 1000,
            transferred_bytes: 250,
            speed_bps: 250.0,
            error_message: None,
        };
        assert_eq!(item.progress_percent(), 25);
        assert_eq!(item.eta_seconds(), Some(3));
    }
}
