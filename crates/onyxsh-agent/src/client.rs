//! Multi-Provider LLM HTTP Client and Streaming SSE Parser (`src/onyxsh/agent/client.py`).
//!
//! Supports OpenAI, Groq, Local (Ollama / LM Studio), Google Gemini, and Anthropic Claude.

use crate::router::RouteDecision;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

/// HTTP request specification prepared for a specific AI provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LlmHttpRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body_json: String,
}

/// Builds a provider-specific streaming HTTP request from a [`RouteDecision`] and message list.
pub fn build_provider_request(
    decision: &RouteDecision,
    api_key: &str,
    local_endpoint: Option<&str>,
    messages: &[ChatMessage],
) -> LlmHttpRequest {
    match decision.provider.as_str() {
        "gemini" => {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
                decision.model, api_key
            );

            let mut system_instruction: Option<Value> = None;
            let mut contents: Vec<Value> = Vec::new();
            for m in messages {
                match m.role {
                    MessageRole::System => {
                        system_instruction = Some(json!({
                            "parts": [{ "text": m.content }]
                        }));
                    }
                    MessageRole::User => {
                        contents.push(json!({
                            "role": "user",
                            "parts": [{ "text": m.content }]
                        }));
                    }
                    MessageRole::Assistant => {
                        contents.push(json!({
                            "role": "model",
                            "parts": [{ "text": m.content }]
                        }));
                    }
                }
            }

            let mut body = json!({
                "contents": contents,
                "generationConfig": { "temperature": 0.2 }
            });
            if let Some(sys) = system_instruction {
                body["systemInstruction"] = sys;
            }

            LlmHttpRequest {
                url,
                headers: vec![("Content-Type".to_string(), "application/json".to_string())],
                body_json: body.to_string(),
            }
        }
        "anthropic" | "claude" => {
            let url = "https://api.anthropic.com/v1/messages".to_string();
            let mut system_prompt = String::new();
            let mut claude_messages: Vec<Value> = Vec::new();

            for m in messages {
                match m.role {
                    MessageRole::System => {
                        if !system_prompt.is_empty() {
                            system_prompt.push('\n');
                        }
                        system_prompt.push_str(&m.content);
                    }
                    MessageRole::User => claude_messages.push(json!({
                        "role": "user",
                        "content": m.content,
                    })),
                    MessageRole::Assistant => claude_messages.push(json!({
                        "role": "assistant",
                        "content": m.content,
                    })),
                }
            }

            let body = json!({
                "model": decision.model,
                "max_tokens": 4096,
                "system": system_prompt,
                "messages": claude_messages,
                "stream": true,
            });

            LlmHttpRequest {
                url,
                headers: vec![
                    ("Content-Type".to_string(), "application/json".to_string()),
                    ("x-api-key".to_string(), api_key.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                ],
                body_json: body.to_string(),
            }
        }
        provider => {
            // Default OpenAI-compatible format (OpenAI, Groq, Ollama, LM Studio)
            let base_url = match provider {
                "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
                "groq" => "https://api.groq.com/openai/v1/chat/completions".to_string(),
                "openrouter" => "https://openrouter.ai/api/v1/chat/completions".to_string(),
                _ => {
                    let base = local_endpoint
                        .unwrap_or("http://127.0.0.1:11434")
                        .trim_end_matches('/');
                    if base.ends_with("/v1/chat/completions") {
                        base.to_string()
                    } else {
                        format!("{base}/v1/chat/completions")
                    }
                }
            };

            let oai_messages: Vec<Value> = messages
                .iter()
                .map(|m| {
                    let role = match m.role {
                        MessageRole::System => "system",
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                    };
                    json!({
                        "role": role,
                        "content": m.content,
                    })
                })
                .collect();

            let mut headers = vec![("Content-Type".to_string(), "application/json".to_string())];
            if !api_key.is_empty() {
                headers.push(("Authorization".to_string(), format!("Bearer {api_key}")));
            }

            let body = json!({
                "model": decision.model,
                "messages": oai_messages,
                "stream": true,
                "temperature": 0.2,
            });

            LlmHttpRequest {
                url: base_url,
                headers,
                body_json: body.to_string(),
            }
        }
    }
}

/// Parses a single Server-Sent Events (`data: ...`) line from any supported provider and
/// returns the incremental text delta if present.
pub fn parse_sse_line(provider: &str, line: &str) -> Option<String> {
    let trimmed = line.trim();
    let payload = trimmed.strip_prefix("data:")?.trim();
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }

    let val: Value = serde_json::from_str(payload).ok()?;
    match provider {
        "gemini" => val
            .get("candidates")?
            .get(0)?
            .get("content")?
            .get("parts")?
            .get(0)?
            .get("text")?
            .as_str()
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
        "anthropic" | "claude" => {
            if val.get("type")?.as_str()? == "content_block_delta" {
                val.get("delta")?
                    .get("text")?
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(ToOwned::to_owned)
            } else {
                None
            }
        }
        _ => val
            .get("choices")?
            .get(0)?
            .get("delta")?
            .get("content")?
            .as_str()
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
    }
}

/// Builds an Ollama `/api/generate` preload request (`keep_alive: -1`) to pin `model` in GPU VRAM.
pub fn build_ollama_preload_request(base_endpoint: &str, model: &str) -> LlmHttpRequest {
    let base = base_endpoint.trim_end_matches('/');
    LlmHttpRequest {
        url: format!("{base}/api/generate"),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body_json: json!({
            "model": model,
            "prompt": "",
            "keep_alive": -1
        })
        .to_string(),
    }
}

/// Builds an Ollama `/api/generate` unload request (`keep_alive: 0`) to evict `model` from GPU VRAM.
pub fn build_ollama_unload_request(base_endpoint: &str, model: &str) -> LlmHttpRequest {
    let base = base_endpoint.trim_end_matches('/');
    LlmHttpRequest {
        url: format!("{base}/api/generate"),
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body_json: json!({
            "model": model,
            "prompt": "",
            "keep_alive": 0
        })
        .to_string(),
    }
}

/// Parses Ollama `/api/ps` JSON output and returns the names of loaded models that differ from `active_model`.
pub fn parse_ollama_ps_models_to_evict(ps_json: &str, active_model: &str) -> Vec<String> {
    let Ok(val) = serde_json::from_str::<Value>(ps_json) else {
        return Vec::new();
    };
    let Some(models) = val.get("models").and_then(|m| m.as_array()) else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|item| item.get("name").and_then(|n| n.as_str()))
        .filter(|&name| name != active_model && !name.starts_with(&format!("{active_model}:")))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{RoutingProfile, TaskComplexity};

    #[test]
    fn test_ollama_vram_lifecycle_preload_evict_and_unload() {
        let preload = build_ollama_preload_request("http://127.0.0.1:11434/", "llama3.2");
        assert_eq!(preload.url, "http://127.0.0.1:11434/api/generate");
        assert!(preload.body_json.contains(r#""keep_alive":-1"#));

        let unload = build_ollama_unload_request("http://127.0.0.1:11434", "qwen2.5-coder");
        assert!(unload.body_json.contains(r#""keep_alive":0"#));

        let ps_sample = r#"{"models":[{"name":"llama3.2:latest"},{"name":"mistral:7b"},{"name":"deepseek-r1:8b"}]}"#;
        let to_evict = parse_ollama_ps_models_to_evict(ps_sample, "llama3.2");
        assert_eq!(to_evict, vec!["mistral:7b", "deepseek-r1:8b"]);
    }

    #[test]
    fn test_build_requests_and_parse_sse_across_providers() {
        let msgs = vec![
            ChatMessage {
                role: MessageRole::System,
                content: "You are OnyxSH Agent.".to_string(),
            },
            ChatMessage {
                role: MessageRole::User,
                content: "Check disk usage".to_string(),
            },
        ];

        // 1. OpenAI / Groq SSE
        let oai_route = RouteDecision {
            provider: "groq".to_string(),
            model: "llama-3.1-8b-instant".to_string(),
            profile_used: RoutingProfile::Fast,
            complexity_inferred: TaskComplexity::Simple,
            offline: false,
            reason: "Fast".to_string(),
        };
        let req = build_provider_request(&oai_route, "gsk_test", None, &msgs);
        assert!(req.url.contains("api.groq.com"));
        let delta = parse_sse_line(
            "groq",
            r#"data: {"choices":[{"delta":{"content":"df -h"}}]}"#,
        );
        assert_eq!(delta.as_deref(), Some("df -h"));
        assert_eq!(parse_sse_line("groq", "data: [DONE]"), None);

        // 2. Gemini SSE
        let gemini_delta = parse_sse_line(
            "gemini",
            r#"data: {"candidates":[{"content":{"parts":[{"text":"ls -la"}]}}]}"#,
        );
        assert_eq!(gemini_delta.as_deref(), Some("ls -la"));

        // 3. Anthropic SSE
        let claude_delta = parse_sse_line(
            "anthropic",
            r#"data: {"type":"content_block_delta","delta":{"type":"text_delta","text":"systemctl status"}}"#,
        );
        assert_eq!(claude_delta.as_deref(), Some("systemctl status"));
    }
}
