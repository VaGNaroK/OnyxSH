//! Secret Redactor to prevent leaking credentials to external LLMs.
//! Port of `src/onyxsh/agent/redactor.py`.

use regex::Regex;
use std::sync::OnceLock;

static SECRET_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

fn patterns() -> &'static Vec<(Regex, &'static str)> {
    SECRET_PATTERNS.get_or_init(|| {
        let raw: &[(&str, &'static str)] = &[
            (r"\b(AKIA[0-9A-Z]{16})\b", "[REDACTED_AWS_KEY]"),
            (
                r#"(?i)\b(aws_secret_access_key|aws_session_token)\s*[:=]\s*['"]?([A-Za-z0-9/+=]{40})['"]?"#,
                "$1=[REDACTED_AWS_SECRET]",
            ),
            (
                r"-----BEGIN [A-Z0-9_\-\s]+PRIVATE KEY-----[\s\S]*?-----END [A-Z0-9_\-\s]+PRIVATE KEY-----",
                "[REDACTED_PRIVATE_KEY]",
            ),
            (
                r"-----BEGIN PGP PRIVATE KEY BLOCK-----[\s\S]*?-----END PGP PRIVATE KEY BLOCK-----",
                "[REDACTED_PGP_PRIVATE_KEY]",
            ),
            (
                r"(?i)\bBearer\s+([a-zA-Z0-9_\-\.]{20,})\b",
                "Bearer [REDACTED_BEARER_TOKEN]",
            ),
            (
                r"\b(sk-[a-zA-Z0-9]{20,}|gsk_[a-zA-Z0-9]{20,}|ghp_[a-zA-Z0-9]{30,}|github_pat_[a-zA-Z0-9_]{40,}|AIzaSy[a-zA-Z0-9_\-]{33})\b",
                "[REDACTED_API_KEY]",
            ),
            (r"\b(glpat-[a-zA-Z0-9_\-]{20,})\b", "[REDACTED_GITLAB_TOKEN]"),
            (r"\b(xox[bpas]-[a-zA-Z0-9\-]{10,})\b", "[REDACTED_SLACK_TOKEN]"),
            (r"\b(vercel_[a-zA-Z0-9_\-]{20,})\b", "[REDACTED_VERCEL_TOKEN]"),
            (r"\b(hvs\.[a-zA-Z0-9_\-]{20,})\b", "[REDACTED_VAULT_TOKEN]"),
            (
                r"(?i)\b((?:postgres|postgresql|mysql|mongodb|redis)://[^:]+:)([^@]+)(@)",
                "$1[REDACTED]$3",
            ),
        ];
        raw.iter()
            .map(|(pat, rep)| (Regex::new(pat).expect("valid redactor regex"), *rep))
            .collect()
    })
}

/// Scans text and replaces sensitive credentials with safe placeholders.
/// Returns `(redacted_text, count_of_redactions)`.
pub fn redact_secrets(text: &str) -> (String, usize) {
    if text.is_empty() {
        return (String::new(), 0);
    }

    let mut current = text.to_string();
    let mut total_count = 0;

    for (re, replacement) in patterns() {
        let count = re.find_iter(&current).count();
        if count > 0 {
            current = re.replace_all(&current, *replacement).into_owned();
            total_count += count;
        }
    }

    (current, total_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redacts_aws_github_and_db_urls() {
        let input = "export AWS_KEY=AKIAIOSFODNN7EXAMPLE and token ghp_1234567890abcdefghijklmnopqrstuvwx and postgres://admin:s3cr3t@db.internal:5432/prod";
        let (redacted, count) = redact_secrets(input);
        assert_eq!(count, 3);
        assert!(redacted.contains("[REDACTED_AWS_KEY]"));
        assert!(redacted.contains("[REDACTED_API_KEY]"));
        assert!(redacted.contains("postgres://admin:[REDACTED]@db.internal:5432/prod"));
        assert!(!redacted.contains("s3cr3t"));
    }
}
