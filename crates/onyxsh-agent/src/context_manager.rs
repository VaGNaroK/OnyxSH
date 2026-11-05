//! Context Manager for preparing secure prompts, attachments, and `<untrusted>` boundary tags.
//! Port of `src/onyxsh/agent/context_manager.py`.

use crate::path_guard::PathGuard;
use crate::redactor::redact_secrets;
use std::fs;
use std::path::Path;

/// Wraps untrusted terminal output or file contents in `<untrusted source="...">` safety tags.
pub fn wrap_untrusted(content: &str, source: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    format!("<untrusted source=\"{source}\">\n{trimmed}\n</untrusted>")
}

#[derive(Debug, Clone, Default)]
pub struct ContextManager {
    pub path_guard: PathGuard,
    pub language: String,
}

impl ContextManager {
    pub fn new(path_guard: PathGuard, language: &str) -> Self {
        Self {
            path_guard,
            language: language.to_string(),
        }
    }

    /// Safely prepares a file attachment, enforcing `PathGuard` and redacting secrets.
    pub fn process_attachment(
        &self,
        file_path: &str,
        provider_trust: &str,
        max_bytes: usize,
    ) -> Option<String> {
        if !self.path_guard.can_read(file_path) {
            return None;
        }
        let path = Path::new(file_path);
        if !path.is_file() {
            return None;
        }
        let raw = fs::read(path).ok()?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("attachment");

        let limit = if provider_trust == "local" {
            max_bytes
        } else {
            max_bytes.min(2000)
        };
        let slice = &raw[..raw.len().min(limit)];
        let text = String::from_utf8_lossy(slice);
        let (redacted, _) = redact_secrets(&text);

        if provider_trust == "local" {
            Some(wrap_untrusted(&redacted, &format!("attachment:{name}")))
        } else {
            let meta = format!(
                "Arquivo: {name} (Tamanho: {} bytes)\nTrecho:\n{redacted}",
                raw.len()
            );
            Some(wrap_untrusted(&meta, &format!("attachment:{name}")))
        }
    }

    /// Sanitizes terminal selection text by redacting secrets and wrapping in `<untrusted>`.
    pub fn prepare_terminal_selection(&self, selection: &str) -> String {
        let trimmed = selection.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        let (redacted, _) = redact_secrets(trimmed);
        wrap_untrusted(&redacted, "terminal_selection")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_manager_wraps_untrusted_and_redacts_secrets() {
        let ctx = ContextManager::new(PathGuard::default(), "Portuguese");
        let sel = ctx.prepare_terminal_selection("Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
        assert!(sel.starts_with("<untrusted source=\"terminal_selection\">"));
        assert!(sel.contains("[REDACTED_BEARER_TOKEN]"));
        assert!(sel.ends_with("</untrusted>"));
    }
}
