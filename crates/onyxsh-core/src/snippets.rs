//! Parameterized command snippet resolver (`{{var}}`, `{{var=default}}`, `{var}`).
//! Port of `src/onyxsh/data/snippet_resolver.py`.

use regex::{Captures, Regex};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

static TEMPLATE_VAR_RE: OnceLock<Regex> = OnceLock::new();

fn template_var_regex() -> &'static Regex {
    TEMPLATE_VAR_RE.get_or_init(|| {
        Regex::new(r"\{\{([^}]+)\}\}|\{([a-zA-Z0-9_-]+(?:=[^}]*)?)\}")
            .expect("valid snippet variable regex")
    })
}

pub const SYSTEM_VARIABLE_NAMES: &[&str] = &[
    "cwd",
    "pwd",
    "host",
    "hostname",
    "user",
    "username",
    "session",
    "date",
    "time",
    "datetime",
    "git_branch",
    "branch_git",
    "clipboard",
    "selection",
    "random_uuid",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnippetVariable {
    pub name: String,
    pub default_value: String,
    pub is_system: bool,
    pub display_label: String,
    pub raw_token: String,
}

impl SnippetVariable {
    pub fn key(&self) -> String {
        self.name.trim().to_lowercase()
    }
}

/// Extracts all unique variables from a snippet template.
pub fn extract_variables(template: &str) -> Vec<SnippetVariable> {
    if template.is_empty() {
        return Vec::new();
    }

    let mut variables = Vec::new();
    let mut seen = HashSet::new();

    for cap in template_var_regex().captures_iter(template) {
        let raw_token = cap.get(0).map(|m| m.as_str()).unwrap_or_default().to_string();
        let inner = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map(|m| m.as_str())
            .unwrap_or_default();
        if inner.is_empty() {
            continue;
        }

        let (var_name, default_val) = match inner.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => (inner.trim(), ""),
        };

        let norm_key = var_name.to_lowercase();
        if seen.contains(&norm_key) {
            continue;
        }
        seen.insert(norm_key.clone());

        let is_system = SYSTEM_VARIABLE_NAMES.contains(&norm_key.as_str());
        let display_label = var_name
            .split('_')
            .filter(|s| !s.is_empty())
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        variables.push(SnippetVariable {
            name: var_name.to_string(),
            default_value: default_val.to_string(),
            is_system,
            display_label,
            raw_token,
        });
    }

    variables
}

/// Returns only user-customizable variables (excluding dynamic system variables).
pub fn get_custom_variables(template: &str) -> Vec<SnippetVariable> {
    extract_variables(template)
        .into_iter()
        .filter(|v| !v.is_system)
        .collect()
}

/// Resolves all placeholders in `template` using `user_values` and `system_context`.
pub fn resolve_template(
    template: &str,
    user_values: &HashMap<String, String>,
    system_context: &HashMap<String, String>,
) -> String {
    if template.is_empty() {
        return String::new();
    }

    let replaced = template_var_regex().replace_all(template, |caps: &Captures| {
        let inner = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or_default();
        if inner.is_empty() {
            return caps.get(0).map(|m| m.as_str()).unwrap_or_default().to_string();
        }

        let (var_name, default_val) = match inner.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => (inner.trim(), ""),
        };
        let norm_key = var_name.to_lowercase();

        if let Some(val) = user_values.get(var_name).or_else(|| user_values.get(&norm_key)) {
            return val.clone();
        }
        if let Some(sys_val) = system_context.get(&norm_key) {
            if !sys_val.is_empty() {
                return sys_val.clone();
            }
        }
        default_val.to_string()
    });

    replaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_and_resolve_snippet_variables() {
        let tpl = "docker logs --tail {{lines=100}} {{container}} > {{cwd}}/out.log";
        let all_vars = extract_variables(tpl);
        assert_eq!(all_vars.len(), 3);

        let custom_vars = get_custom_variables(tpl);
        assert_eq!(custom_vars.len(), 2);
        assert_eq!(custom_vars[0].name, "lines");
        assert_eq!(custom_vars[0].default_value, "100");
        assert_eq!(custom_vars[1].name, "container");

        let mut user_vals = HashMap::new();
        user_vals.insert("container".to_string(), "nginx-proxy".to_string());
        let mut sys_ctx = HashMap::new();
        sys_ctx.insert("cwd".to_string(), "/home/vagnarok".to_string());

        let resolved = resolve_template(tpl, &user_vals, &sys_ctx);
        assert_eq!(
            resolved,
            "docker logs --tail 100 nginx-proxy > /home/vagnarok/out.log"
        );
    }
}
