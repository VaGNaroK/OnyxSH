//! Zero-Trust AI Agent subsystem: Models, Secret Redactor, PathGuard, PolicyEngine,
//! SmartRouter, PlanParser, ContextManager, TerminalErrorMatcher, PostExecutionVerifier,
//! and AuditLogger.

pub mod audit;
pub mod client;
pub mod context_manager;
pub mod error_matcher;
pub mod models;
pub mod path_guard;
pub mod planner;
pub mod policy_engine;
pub mod redactor;
pub mod router;
pub mod verifier;

#[cfg(test)]
mod property_tests {
    use crate::path_guard::PathGuard;
    use crate::planner::parse_action_plan;
    use crate::policy_engine::PolicyEngine;
    use crate::redactor::redact_secrets;

    fn next_u64(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    #[test]
    fn prop_agent_parsers_and_zero_trust_guards_never_panic_and_enforce_invariants() {
        let policy = PolicyEngine::default();
        let path_guard = PathGuard::default();
        let mut seed = 0xDEAD_BEEF_1337_4242u64;

        for _ in 0..250 {
            let r = next_u64(&mut seed);
            let token_suffix: String = (0..36)
                .map(|j| {
                    let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
                    let idx = ((r.wrapping_add(j * 17)) as usize) % chars.len();
                    chars[idx] as char
                })
                .collect();

            // 1. Property: GitHub PATs (`ghp_...`) are always redacted regardless of surrounding text
            let secret_input = format!("export TOKEN=ghp_{token_suffix} && echo ok");
            let (redacted, count) = redact_secrets(&secret_input);
            assert!(count >= 1);
            assert!(!redacted.contains(&token_suffix));

            // 2. Property: PathGuard always denies reading `~/.ssh/*` and `.env*` traversal variants
            let traversal_ssh = format!("~/.ssh/../.ssh/id_rsa_{token_suffix}");
            assert!(!path_guard.can_read(&traversal_ssh));
            assert!(!path_guard.can_write(&traversal_ssh));

            // 3. Property: PlanParser never panics on malformed/random JSON fences
            let malformed_plan = format!(
                "Prefix {token_suffix} ```json\n{{\"intent\": \"{token_suffix}\", \"steps\": [{{\"command\": \"ls -la\"}}]}}\n```"
            );
            let parsed = parse_action_plan(&malformed_plan, &policy);
            assert!(parsed.is_some());
        }
    }
}
