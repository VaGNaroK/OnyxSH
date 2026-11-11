//! Smart Model Routing Engine and Task Complexity Classifier.
//! Port of `src/onyxsh/agent/router.py`.

use onyxsh_core::i18n::tr;
use regex::RegexSet;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskComplexity {
    Simple,
    Complex,
    Security,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingProfile {
    Auto,
    Fast,
    Advanced,
    Security,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteDecision {
    pub provider: String,
    pub model: String,
    pub profile_used: RoutingProfile,
    pub complexity_inferred: TaskComplexity,
    pub offline: bool,
    pub reason: String,
}

struct ClassifierSets {
    security: RegexSet,
    complex: RegexSet,
    simple: RegexSet,
}

static CLASSIFIER: OnceLock<ClassifierSets> = OnceLock::new();

fn classifier_sets() -> &'static ClassifierSets {
    CLASSIFIER.get_or_init(|| {
        let security = RegexSet::new([
            r"(?i)\b(segurança|vulnerabilidade|cve|auditoria|auditar|hardening|firewall|iptables|ufw)\b",
            r"(?i)\b(permissoes?\s+indevidas|rootkit|malware|analise\s+de\s+logs?\s+de\s+erro)\b",
            r"(?i)\b(self-healing|auto-fix|corrigir?\s+falha|reparar?\s+serviço|diagnosticar?\s+erro)\b",
            r"(?i)\b(security|vulnerability|cve|audit|hardening|firewall|exploit|penetration)\b",
        ])
        .expect("valid security regex set");

        let complex = RegexSet::new([
            r"(?i)\b(crie|criar|escreva|escrever|gere|gerar|desenvolva)\s+.*\b(script|código|automação|pipeline|playbook|dockerfile)\b",
            r"(?i)\b(pipeline|playbook|dockerfile|workflow|automação|automacao)\b",
            r"(?i)\b(passo\s+a\s+passo|etapas|plano|planejar|roteiro|tutorial)\b",
            r"(?i)\b(instalar?\s+e\s+configurar?|subir\s+serviço|deploy|implantar?|migrar?|orquestrar?)\b",
            r"(?i)\b(create|write|generate|build)\s+.*\b(script|code|automation|pipeline|playbook|dockerfile)\b",
            r"(?i)\b(step\s+by\s+step|multi-step|plan|planning|workflow|architecture)\b",
        ])
        .expect("valid complex regex set");

        let simple = RegexSet::new([
            r"(?i)^(como\s+(ver|listar?|checar?|mostrar?|exibir?|consultar?)|qual\s+(o\s+)?comando|qual\s+a\s+flag|o\s+que\s+significa)\b",
            r"(?i)^(how\s+to\s+(check|list|show|view|find)|what\s+is\s+the\s+command|what\s+does\s+flag)\b",
        ])
        .expect("valid simple regex set");

        ClassifierSets {
            security,
            complex,
            simple,
        }
    })
}

/// Classifies a user prompt into `TaskComplexity`.
pub fn classify_prompt_complexity(prompt: &str) -> TaskComplexity {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return TaskComplexity::Simple;
    }

    let sets = classifier_sets();
    if sets.security.is_match(trimmed) {
        return TaskComplexity::Security;
    }
    if sets.complex.is_match(trimmed) {
        return TaskComplexity::Complex;
    }
    if sets.simple.is_match(trimmed) {
        return TaskComplexity::Simple;
    }

    let non_empty_lines = trimmed.lines().filter(|l| !l.trim().is_empty()).count();
    if non_empty_lines >= 3 || trimmed.len() > 300 {
        return TaskComplexity::Complex;
    }

    TaskComplexity::Simple
}

#[derive(Debug, Clone)]
pub struct SmartRouter {
    pub smart_routing_enabled: bool,
    pub offline_mode: bool,
    pub default_provider: String,
    pub default_model: String,
    pub local_model: String,
}

impl Default for SmartRouter {
    fn default() -> Self {
        Self {
            smart_routing_enabled: true,
            offline_mode: false,
            default_provider: "gemini".to_string(),
            default_model: "gemini-2.5-flash".to_string(),
            local_model: "llama3.2".to_string(),
        }
    }
}

impl SmartRouter {
    pub fn resolve_route(
        &self,
        prompt: &str,
        forced_profile: Option<RoutingProfile>,
    ) -> RouteDecision {
        // 1. Strict Offline Mode always takes highest precedence
        if self.offline_mode {
            return RouteDecision {
                provider: "local".to_string(),
                model: self.local_model.clone(),
                profile_used: RoutingProfile::Fast,
                complexity_inferred: TaskComplexity::Simple,
                offline: true,
                reason: tr("Strict Offline Mode active (local Ollama/LM Studio only)."),
            };
        }

        let complexity = classify_prompt_complexity(prompt);
        let profile = forced_profile.unwrap_or(RoutingProfile::Auto);

        if !self.smart_routing_enabled && profile == RoutingProfile::Auto {
            return RouteDecision {
                provider: self.default_provider.clone(),
                model: self.default_model.clone(),
                profile_used: RoutingProfile::Auto,
                complexity_inferred: complexity,
                offline: false,
                reason: tr("Smart routing disabled: using default configured provider."),
            };
        }

        let effective_profile = match profile {
            RoutingProfile::Auto => match complexity {
                TaskComplexity::Simple => RoutingProfile::Fast,
                TaskComplexity::Complex => RoutingProfile::Advanced,
                TaskComplexity::Security => RoutingProfile::Security,
            },
            explicit => explicit,
        };

        match effective_profile {
            RoutingProfile::Fast => RouteDecision {
                provider: "groq".to_string(),
                model: "llama-3.1-8b-instant".to_string(),
                profile_used: RoutingProfile::Fast,
                complexity_inferred: complexity,
                offline: false,
                reason: tr("Routed to Fast profile (low-latency simple query)."),
            },
            RoutingProfile::Advanced | RoutingProfile::Security | RoutingProfile::Auto => {
                RouteDecision {
                    provider: "gemini".to_string(),
                    model: "gemini-2.5-flash".to_string(),
                    profile_used: effective_profile,
                    complexity_inferred: complexity,
                    offline: false,
                    reason: tr("Routed to Advanced/Security reasoning profile."),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_router_offline_and_complexity_routing() {
        let mut router = SmartRouter::default();

        let simple = router.resolve_route("como listar arquivos?", None);
        assert_eq!(simple.provider, "groq");
        assert_eq!(simple.complexity_inferred, TaskComplexity::Simple);

        let complex = router.resolve_route("crie um script completo de deploy com dockerfile", None);
        assert_eq!(complex.provider, "gemini");
        assert_eq!(complex.complexity_inferred, TaskComplexity::Complex);

        router.offline_mode = true;
        let offline_route = router.resolve_route("crie um script complexo de auditoria", None);
        assert_eq!(offline_route.provider, "local");
        assert!(offline_route.offline);
    }
}
