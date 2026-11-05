//! OSC 52 Zero-Trust Clipboard Bridge (Port of `src/onyxsh/terminal/osc52_bridge.py`).

use base64::Engine;
use regex::bytes::Regex;
use std::sync::OnceLock;

/// Maximum allowed base64 payload size per OSC 52 sequence (10 MB).
pub const MAX_PAYLOAD_BYTES: usize = 10 * 1024 * 1024;

static OSC52_RE: OnceLock<Regex> = OnceLock::new();

fn osc52_regex() -> &'static Regex {
    OSC52_RE.get_or_init(|| {
        Regex::new(r"\x1b\]52;([a-zA-Z0-9]*);([A-Za-z0-9+/=\s]*|\?)(?:\x07|\x1b\\)")
            .expect("valid OSC52 regex")
    })
}

/// Extracts and decodes valid OSC 52 write sequences from raw PTY bytes.
/// Rejects `?` read queries (Zero-Trust) and payloads exceeding `max_payload_bytes`.
pub fn parse_osc52_sequences(data: &[u8], max_payload_bytes: usize) -> Vec<(String, String)> {
    let mut results = Vec::new();
    if !data.windows(5).any(|w| w == b"\x1b]52;") {
        return results;
    }

    for cap in osc52_regex().captures_iter(data) {
        let raw_target = cap
            .get(1)
            .map(|m| String::from_utf8_lossy(m.as_bytes()).to_lowercase())
            .unwrap_or_default();
        let Some(payload_match) = cap.get(2) else {
            continue;
        };
        let payload = payload_match.as_bytes();

        // 1. Zero-Trust rule: reject clipboard read queries ('?')
        if payload == b"?" {
            continue;
        }

        // 2. DoS protection: enforce max payload size
        if payload.len() > max_payload_bytes {
            tracing::warn!(
                "OSC 52: Payload exceeded maximum safe limit ({} > {} bytes). Ignored.",
                payload.len(),
                max_payload_bytes
            );
            continue;
        }

        // 3. Strip whitespace inside base64 payload
        let clean_payload: Vec<u8> = payload
            .iter()
            .copied()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        if clean_payload.is_empty() {
            continue;
        }

        // 4. Decode Base64 safely
        if let Ok(decoded_bytes) = base64::engine::general_purpose::STANDARD.decode(&clean_payload)
        {
            let text = String::from_utf8_lossy(&decoded_bytes).into_owned();
            let target = if raw_target.is_empty() {
                "c".to_string()
            } else {
                raw_target
            };
            results.push((target, text));
        }
    }

    results
}

/// Removes all complete OSC 52 sequences from a byte slice.
pub fn strip_osc52_sequences(data: &[u8]) -> Vec<u8> {
    if !data.windows(5).any(|w| w == b"\x1b]52;") {
        return data.to_vec();
    }
    osc52_regex().replace_all(data, &b""[..]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osc52_parse_and_strip() {
        let raw = b"before\x1b]52;c;SGVsbG8gT255eFNI\x07after";
        let parsed = parse_osc52_sequences(raw, MAX_PAYLOAD_BYTES);
        assert_eq!(parsed, vec![("c".to_string(), "Hello OnyxSH".to_string())]);

        let stripped = strip_osc52_sequences(raw);
        assert_eq!(stripped, b"beforeafter");
    }

    #[test]
    fn test_osc52_rejects_read_query_and_oversized_payload() {
        let query = b"\x1b]52;c;?\x07";
        assert!(parse_osc52_sequences(query, MAX_PAYLOAD_BYTES).is_empty());

        let oversized = b"\x1b]52;c;SGVsbG8gT255eFNI\x07";
        assert!(parse_osc52_sequences(oversized, 4).is_empty());
    }
}
