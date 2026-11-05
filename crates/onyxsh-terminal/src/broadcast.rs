//! Cluster Broadcast / Multi-Execution Engine (`src/onyxsh/terminal/manager.py` & `window.py`).
//!
//! Coordinates simultaneous keyboard/command fan-out across split panes or tab groups
//! while enforcing [`ProductionGuard`] checks before broadcasting destructive commands.

use crate::production_guard::{GuardViolation, ProductionGuard};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BroadcastScope {
    #[default]
    Disabled,
    CurrentTabSplits,
    AllTerminals,
    CustomSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastOutcome {
    /// Broadcast is disabled; send only to the focused terminal.
    SingleTarget(u64),
    /// Fan out the payload to the resolved list of target terminal IDs.
    FanOut {
        targets: Vec<u64>,
        payload: String,
    },
    /// Intercepted by [`ProductionGuard`] before fan-out to protect cluster nodes.
    BlockedByGuard {
        targets: Vec<u64>,
        violation: GuardViolation,
    },
}

#[derive(Debug, Clone, Default)]
pub struct BroadcastRouter {
    pub scope: BroadcastScope,
    pub custom_targets: BTreeSet<u64>,
    pub guard_enabled: bool,
}

impl BroadcastRouter {
    pub fn new(scope: BroadcastScope, guard_enabled: bool) -> Self {
        Self {
            scope,
            custom_targets: BTreeSet::new(),
            guard_enabled,
        }
    }

    pub fn toggle_custom_target(&mut self, terminal_id: u64) {
        if !self.custom_targets.remove(&terminal_id) {
            self.custom_targets.insert(terminal_id);
        }
    }

    /// Resolves the target terminal IDs for a given active terminal, current tab terminals, and all window terminals.
    pub fn resolve_targets(
        &self,
        focused_terminal: u64,
        current_tab_terminals: &[u64],
        all_terminals: &[u64],
    ) -> Vec<u64> {
        match self.scope {
            BroadcastScope::Disabled => vec![focused_terminal],
            BroadcastScope::CurrentTabSplits => current_tab_terminals.to_vec(),
            BroadcastScope::AllTerminals => all_terminals.to_vec(),
            BroadcastScope::CustomSelection => self.custom_targets.iter().copied().collect(),
        }
    }

    /// Routes a command line across the active broadcast scope, checking [`ProductionGuard`] when enabled.
    pub fn dispatch_command(
        &self,
        focused_terminal: u64,
        current_tab_terminals: &[u64],
        all_terminals: &[u64],
        command_line: &str,
    ) -> BroadcastOutcome {
        if self.scope == BroadcastScope::Disabled {
            return BroadcastOutcome::SingleTarget(focused_terminal);
        }

        let targets = self.resolve_targets(focused_terminal, current_tab_terminals, all_terminals);
        if self.guard_enabled {
            let guard = ProductionGuard::new();
            if let Some(violation) = guard.evaluate_command(command_line) {
                return BroadcastOutcome::BlockedByGuard { targets, violation };
            }
        }

        BroadcastOutcome::FanOut {
            targets,
            payload: command_line.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_broadcast_router_fanout_and_production_guard_interception() {
        let router = BroadcastRouter::new(BroadcastScope::CurrentTabSplits, true);
        let tab_terms = vec![101, 102, 103];
        let all_terms = vec![101, 102, 103, 201];

        let safe = router.dispatch_command(101, &tab_terms, &all_terms, "uptime");
        assert_eq!(
            safe,
            BroadcastOutcome::FanOut {
                targets: vec![101, 102, 103],
                payload: "uptime".to_string(),
            }
        );

        let dangerous = router.dispatch_command(101, &tab_terms, &all_terms, "sudo rm -rf /var/lib");
        assert!(matches!(
            dangerous,
            BroadcastOutcome::BlockedByGuard { ref targets, .. } if targets == &vec![101, 102, 103]
        ));
    }
}
