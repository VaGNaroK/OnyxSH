//! Structured logging subsystem (`tracing` port of `src/onyxsh/utils/logger.py`).

use std::sync::Once;
use tracing_subscriber::{fmt, EnvFilter};

static INIT_LOGGER: Once = Once::new();

/// Initializes the global thread-safe tracing subscriber.
pub fn init_logger(default_level: &str) {
    INIT_LOGGER.call_once(|| {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(default_level));

        let _ = fmt()
            .with_env_filter(filter)
            .with_target(true)
            .with_thread_ids(false)
            .try_init();
    });
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
}
