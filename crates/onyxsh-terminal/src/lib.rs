//! Terminal subsystem: ProductionGuard, OSC 133 SemanticTracker, OSC 7 CWD Tracker,
//! OSC 52 Zero-Trust Clipboard Bridge, DFA Syntax Highlighter, Autocomplete Engine,
//! Multi-Format Exporter, Runbook Generator, SSH Health Monitor, SSH Tunnel Manager,
//! and Terminal Spawner.

pub mod broadcast;
pub mod completion;
pub mod exporter;
pub mod git_assistant;
pub mod highlighter;
pub mod osc52;
pub mod osc7;
pub mod production_guard;
pub mod pty_proxy;
pub mod runbook;
pub mod semantic_tracker;
pub mod spawner;
pub mod ssh_health;
pub mod tunnel_manager;

#[cfg(test)]
mod property_tests {
    use crate::osc52::parse_osc52_sequences;
    use crate::osc7::create_display_path;
    use crate::production_guard::ProductionGuard;
    use crate::semantic_tracker::SemanticTerminalState;

    /// Deterministic xorshift64 PRNG for fast zero-dependency property fuzzing.
    fn next_u64(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }

    #[test]
    fn prop_osc_parsers_and_production_guard_never_panic_and_enforce_invariants() {
        let guard = ProductionGuard::new();
        let mut tracker = SemanticTerminalState::new();
        let mut seed = 0x5EED_CAFE_BABE_1234u64;

        let wrappers = [
            "bash -c",
            "sh -c",
            "eval",
            "sudo",
            "nohup",
            "xargs",
            "env",
        ];

        for i in 0..250 {
            let r = next_u64(&mut seed);
            let noise: String = (0..24)
                .map(|j| {
                    let b = ((r.wrapping_add(j * 31)) & 0x7F) as u8;
                    (b.max(32)) as char
                })
                .collect();

            // 1. Property: OSC 52 parser never panics and rejects `?` read queries
            let osc52_probe = format!("\x1b]52;c;?\x07\x1b]52;c;{noise}\x07trailing_{i}");
            let parsed = parse_osc52_sequences(osc52_probe.as_bytes(), 1024);
            assert!(parsed.iter().all(|(_, val)| val != "?"));

            // 2. Property: OSC 7 display path formatter never panics
            let display = create_display_path(&format!("/home/user/{noise}/sub"), "/home/user");
            assert!(!display.is_empty());

            // 3. Property: SemanticTerminalState handles arbitrary lifecycle sequences cleanly
            let row = i as i64;
            tracker.start_prompt(row);
            tracker.start_command(row);
            tracker.execute_command(row + 1, &noise, "/tmp");
            let finished = tracker.finish_command(row + 2, (r % 255) as i32, 0.25);
            assert!(finished.is_finished());

            // 4. Property: Any destructive core command wrapped in arbitrary shell wrappers is still intercepted
            let w = wrappers[(r as usize) % wrappers.len()];
            let mutated_destructive = format!("{w} \"rm -rf / --no-preserve-root #{noise}\"");
            assert!(
                guard.evaluate_command(&mutated_destructive).is_some(),
                "ProductionGuard must intercept wrapped destructive command: {mutated_destructive}"
            );
        }
    }
}
