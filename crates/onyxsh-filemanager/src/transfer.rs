//! Asynchronous File Transfer Queue models and progress/ETA calculators.
//! Port of `src/onyxsh/filemanager/transfer_manager.py`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

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

/// Validates whether `available_bytes` has room for `required_bytes` before starting a transfer.
pub fn check_sufficient_space(required_bytes: u64, available_bytes: Option<u64>) -> Result<(), String> {
    if let Some(avail) = available_bytes {
        if required_bytes > avail {
            return Err(format!(
                "Espaço insuficiente no destino: necessário {}, disponível {}.",
                crate::format_bytes(required_bytes),
                crate::format_bytes(avail)
            ));
        }
    }
    Ok(())
}

/// Thread-safe asynchronous transfer queue state with per-item cancellation flags.
#[derive(Debug, Default)]
struct TransferQueueInner {
    items: Vec<TransferItem>,
    cancellations: HashMap<String, Arc<AtomicBool>>,
    next_seq: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TransferQueueController {
    inner: Arc<Mutex<TransferQueueInner>>,
}

impl TransferQueueController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(
        &self,
        filename: String,
        source_path: String,
        destination_path: String,
        direction: TransferDirection,
        total_bytes: u64,
    ) -> (String, Arc<AtomicBool>) {
        let mut guard = self.inner.lock().expect("transfer queue lock");
        guard.next_seq += 1;
        let id = format!("tx_{}", guard.next_seq);
        let cancel_flag = Arc::new(AtomicBool::new(false));
        guard.items.push(TransferItem {
            id: id.clone(),
            filename,
            source_path,
            destination_path,
            direction,
            status: TransferStatus::Queued,
            total_bytes,
            transferred_bytes: 0,
            speed_bps: 0.0,
            error_message: None,
        });
        guard.cancellations.insert(id.clone(), Arc::clone(&cancel_flag));
        (id, cancel_flag)
    }

    pub fn mark_in_progress(&self, id: &str) {
        let mut guard = self.inner.lock().expect("transfer queue lock");
        if let Some(item) = guard.items.iter_mut().find(|i| i.id == id) {
            item.status = TransferStatus::InProgress;
        }
    }

    pub fn update_progress(&self, id: &str, transferred_bytes: u64, total_bytes: Option<u64>, speed_bps: f64) {
        let mut guard = self.inner.lock().expect("transfer queue lock");
        if let Some(item) = guard.items.iter_mut().find(|i| i.id == id) {
            if let Some(tot) = total_bytes {
                item.total_bytes = tot;
            }
            item.transferred_bytes = transferred_bytes.min(item.total_bytes.max(transferred_bytes));
            item.speed_bps = speed_bps;
        }
    }

    pub fn mark_completed(&self, id: &str) {
        let mut guard = self.inner.lock().expect("transfer queue lock");
        if let Some(item) = guard.items.iter_mut().find(|i| i.id == id) {
            item.status = TransferStatus::Completed;
            item.transferred_bytes = item.total_bytes;
            item.speed_bps = 0.0;
        }
    }

    pub fn mark_failed(&self, id: &str, err: String) {
        let mut guard = self.inner.lock().expect("transfer queue lock");
        if let Some(item) = guard.items.iter_mut().find(|i| i.id == id) {
            item.status = TransferStatus::Failed;
            item.speed_bps = 0.0;
            item.error_message = Some(err);
        }
    }

    pub fn cancel(&self, id: &str) -> bool {
        let mut guard = self.inner.lock().expect("transfer queue lock");
        if let Some(flag) = guard.cancellations.get(id) {
            flag.store(true, Ordering::SeqCst);
        }
        if let Some(item) = guard.items.iter_mut().find(|i| i.id == id) {
            if matches!(item.status, TransferStatus::Queued | TransferStatus::InProgress) {
                item.status = TransferStatus::Cancelled;
                item.speed_bps = 0.0;
                return true;
            }
        }
        false
    }

    pub fn is_cancelled(&self, id: &str) -> bool {
        let guard = self.inner.lock().expect("transfer queue lock");
        guard
            .cancellations
            .get(id)
            .is_some_and(|f| f.load(Ordering::SeqCst))
    }

    pub fn clear_finished(&self) {
        let mut guard = self.inner.lock().expect("transfer queue lock");
        guard
            .items
            .retain(|t| matches!(t.status, TransferStatus::Queued | TransferStatus::InProgress));
        let active_ids: std::collections::HashSet<String> =
            guard.items.iter().map(|i| i.id.clone()).collect();
        guard.cancellations.retain(|k, _| active_ids.contains(k));
    }

    pub fn snapshot(&self) -> Vec<TransferItem> {
        self.inner.lock().expect("transfer queue lock").items.clone()
    }

    pub fn active_count(&self) -> usize {
        self.inner
            .lock()
            .expect("transfer queue lock")
            .items
            .iter()
            .filter(|i| matches!(i.status, TransferStatus::Queued | TransferStatus::InProgress))
            .count()
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

    #[test]
    fn test_transfer_queue_controller_and_space_check() {
        assert!(check_sufficient_space(1024, Some(2048)).is_ok());
        assert!(check_sufficient_space(4096, Some(1024)).is_err());

        let ctrl = TransferQueueController::new();
        let (id1, flag1) = ctrl.enqueue(
            "a.bin".into(),
            "/tmp/a.bin".into(),
            "/srv/a.bin".into(),
            TransferDirection::Upload,
            1000,
        );
        assert_eq!(ctrl.active_count(), 1);
        ctrl.mark_in_progress(&id1);
        ctrl.update_progress(&id1, 500, None, 100.0);
        assert_eq!(ctrl.snapshot()[0].progress_percent(), 50);

        assert!(ctrl.cancel(&id1));
        assert!(flag1.load(Ordering::SeqCst));
        assert_eq!(ctrl.snapshot()[0].status, TransferStatus::Cancelled);
        assert_eq!(ctrl.active_count(), 0);
    }
}
