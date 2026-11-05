//! Internationalization utilities (Port of `src/onyxsh/utils/translation_utils.py`).

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

static TRANSLATION_CACHE: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<String, String>> {
    TRANSLATION_CACHE.get_or_init(|| RwLock::new(HashMap::with_capacity(256)))
}

/// Translates a message key with thread-safe memoization.
pub fn tr(msgid: &str) -> String {
    if msgid.is_empty() {
        return String::new();
    }
    if let Ok(guard) = cache().read() {
        if let Some(cached) = guard.get(msgid) {
            return cached.clone();
        }
    }
    let translated = msgid.to_string();
    if let Ok(mut guard) = cache().write() {
        if guard.len() < 1024 {
            guard.insert(msgid.to_string(), translated.clone());
        }
    }
    translated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tr_memoization() {
        assert_eq!(tr("Terminal"), "Terminal");
        assert_eq!(tr(""), "");
        assert_eq!(tr("File System"), "File System");
    }
}
