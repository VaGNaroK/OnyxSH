//! Window, tab, and recursive split layout state persistence.
//! Port of `src/onyxsh/state/window_state.py`.

use crate::sessions::SessionItem;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Recursive representation of a tab's widget tree (`terminal` leaf or `paned` split).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TabNodeState {
    Terminal {
        session_type: String,
        title: String,
        #[serde(default)]
        working_directory: Option<String>,
        #[serde(default)]
        session_data: Option<SessionItem>,
    },
    Paned {
        orientation: String,
        position: i32,
        #[serde(default)]
        position_ratio: Option<f64>,
        child1: Box<TabNodeState>,
        child2: Box<TabNodeState>,
    },
}

impl TabNodeState {
    /// Counts the number of terminal leaves inside this recursive tab tree.
    pub fn terminal_count(&self) -> usize {
        match self {
            TabNodeState::Terminal { .. } => 1,
            TabNodeState::Paned { child1, child2, .. } => {
                child1.terminal_count() + child2.terminal_count()
            }
        }
    }
}

/// Top-level window session state compatible with `~/.config/onyxsh/state.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSessionState {
    pub tabs: Vec<TabNodeState>,
    pub active_tab_index: usize,
    pub sidebar_visible: bool,
    pub ai_panel_visible: bool,
}

impl WindowSessionState {
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    pub fn load_from_path(path: &Path) -> std::io::Result<Self> {
        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save_to_path(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(self)?;
        fs::write(path, serialized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_state_recursive_split_serialization() {
        let state = WindowSessionState {
            tabs: vec![TabNodeState::Paned {
                orientation: "horizontal".to_string(),
                position: 400,
                position_ratio: Some(0.5),
                child1: Box::new(TabNodeState::Terminal {
                    session_type: "local".to_string(),
                    title: "Local 1".to_string(),
                    working_directory: Some("/home/vagnarok".to_string()),
                    session_data: None,
                }),
                child2: Box::new(TabNodeState::Terminal {
                    session_type: "ssh".to_string(),
                    title: "Prod SSH".to_string(),
                    working_directory: None,
                    session_data: None,
                }),
            }],
            active_tab_index: 0,
            sidebar_visible: true,
            ai_panel_visible: false,
        };

        let json = serde_json::to_string(&state).unwrap();
        let roundtrip: WindowSessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip, state);
        assert!(roundtrip.has_tabs());
    }
}
