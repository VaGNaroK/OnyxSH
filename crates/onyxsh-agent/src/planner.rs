//! Plan Parser and Validator for converting LLM output into `ActionPlan` instances.
//! Port of `src/onyxsh/agent/planner.py`.

use crate::models::{ActionPlan, ActionStep, RiskLevel};
use crate::policy_engine::PolicyEngine;
use regex::Regex;
use std::sync::OnceLock;

static JSON_FENCE_RE: OnceLock<Regex> = OnceLock::new();

fn json_fence_regex() -> &'static Regex {
    JSON_FENCE_RE.get_or_init(|| {
        Regex::new(r"(?i)```(?:json)?\s*(\{[\s\S]*?\})\s*```").expect("valid json fence regex")
    })
}

/// Splits a shell command string into an `argv` vector while preserving heredocs (`<<`).
pub fn split_command_to_argv(cmd_str: &str) -> Vec<String> {
    let cleaned = cmd_str.trim();
    if cleaned.is_empty() {
        return Vec::new();
    }
    if cleaned.contains("<<") {
        return vec![cleaned.to_string()];
    }
    shlex::split(cleaned).unwrap_or_else(|| cleaned.split_whitespace().map(String::from).collect())
}

/// Extracts a JSON `serde_json::Value::Object` from raw LLM text (supporting markdown fences and outer braces).
pub fn extract_json_object(raw_text: &str) -> Option<serde_json::Value> {
    let trimmed = raw_text.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Direct JSON parse
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if val.is_object() {
            return Some(val);
        }
    }

    // 2. Markdown ```json ... ``` fence
    if let Some(cap) = json_fence_regex().captures(trimmed) {
        if let Some(m) = cap.get(1) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(m.as_str()) {
                if val.is_object() {
                    return Some(val);
                }
            }
        }
    }

    // 3. Outermost `{ ... }`
    let first_brace = trimmed.find('{')?;
    let last_brace = trimmed.rfind('}')?;
    if last_brace > first_brace {
        let slice = &trimmed[first_brace..=last_brace];
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(slice) {
            if val.is_object() {
                return Some(val);
            }
        }
    }

    None
}

/// Parses raw LLM output into a policy-evaluated `ActionPlan`.
pub fn parse_action_plan(raw_text: &str, policy: &PolicyEngine) -> Option<ActionPlan> {
    let obj = extract_json_object(raw_text)?;
    let goal = obj
        .get("goal")
        .or_else(|| obj.get("intent"))
        .or_else(|| obj.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or("Executar plano do agente")
        .to_string();

    let mut steps = Vec::new();
    if let Some(raw_steps) = obj.get("steps").and_then(|v| v.as_array()) {
        for (idx, item) in raw_steps.iter().enumerate() {
            let tool = item
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("shell.run")
                .to_string();
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("Executar passo")
                .to_string();
            let requires_admin = item
                .get("requires_admin")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let argv: Vec<String> = if let Some(arr) = item.get("argv").and_then(|v| v.as_array()) {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            } else if let Some(cmd_str) = item.get("command").and_then(|v| v.as_str()) {
                split_command_to_argv(cmd_str)
            } else {
                Vec::new()
            };

            let step = ActionStep {
                id: idx + 1,
                description,
                tool,
                argv,
                requires_admin,
                risk_level: RiskLevel::ReadOnly,
            };
            steps.push(policy.evaluate_step(step));
        }
    }

    Some(ActionPlan { goal, steps })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action_plan_and_heredoc_preservation() {
        let heredoc = "cat << 'EOF' > ~/deploy.sh\n#!/usr/bin/env bash\necho ok\nEOF";
        let argv = split_command_to_argv(heredoc);
        assert_eq!(argv.len(), 1);

        let llm_output = r#"
Aqui está o plano solicitado:
```json
{
  "intent": "Verificar status do Nginx e atualizar pacotes",
  "steps": [
    {
      "tool": "shell.run",
      "argv": ["git", "status"],
      "description": "Checar status do repo",
      "requires_admin": false
    },
    {
      "tool": "shell.run",
      "argv": ["sudo", "apt", "update"],
      "description": "Atualizar índices apt",
      "requires_admin": true
    }
  ]
}
```
"#;
        let policy = PolicyEngine::new();
        let plan = parse_action_plan(llm_output, &policy).expect("must parse plan");
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].risk_level, RiskLevel::ReadOnly);
        assert_eq!(plan.steps[1].risk_level, RiskLevel::Admin);
        assert_eq!(plan.max_risk_level(), RiskLevel::Admin);
    }
}
