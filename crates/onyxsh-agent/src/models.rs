//! Domain models for the Zero-Trust AI Agent (`src/onyxsh/agent/models.py`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum RiskLevel {
    ReadOnly = 0,
    UserWrite = 1,
    SystemModify = 2,
    Admin = 3,
    Blocked = 4,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionStep {
    pub id: usize,
    pub description: String,
    pub tool: String,
    pub argv: Vec<String>,
    pub requires_admin: bool,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionPlan {
    pub goal: String,
    pub steps: Vec<ActionStep>,
}

impl ActionPlan {
    pub fn max_risk_level(&self) -> RiskLevel {
        self.steps
            .iter()
            .map(|s| s.risk_level)
            .max()
            .unwrap_or(RiskLevel::ReadOnly)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: String,
    pub timestamp: String,
    pub tool: String,
    pub command_or_path: String,
    pub risk_level: RiskLevel,
    pub exit_code: i32,
    pub backup_path: Option<String>,
    pub target_path: Option<String>,
}
