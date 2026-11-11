//! Relm4-style Unidirectional Application State Machine (`src/onyxsh/window.py`).

use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::sessions::SessionItem;
use onyxsh_core::window_state::{TabNodeState, WindowSessionState};
use serde::{Deserialize, Serialize};

/// Search bar state for an active terminal pane (`src/onyxsh/ui/search_bar.py` & `manager.py`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSearchState {
    pub visible: bool,
    pub query: String,
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub current_match_index: usize,
    pub total_matches: usize,
}

impl TerminalSearchState {
    /// Formats the match counter label (`0/0` or `1/5`).
    pub fn counter_label(&self) -> String {
        if self.total_matches == 0 {
            "0/0".to_string()
        } else {
            format!("{}/{}", self.current_match_index + 1, self.total_matches)
        }
    }

    pub fn next_match(&mut self) {
        if self.total_matches > 0 {
            self.current_match_index = (self.current_match_index + 1) % self.total_matches;
        }
    }

    pub fn prev_match(&mut self) {
        if self.total_matches > 0 {
            self.current_match_index = if self.current_match_index == 0 {
                self.total_matches - 1
            } else {
                self.current_match_index - 1
            };
        }
    }
}

/// Represents a single workspace tab in the main window (`Adw.TabView`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTab {
    pub id: u64,
    pub title: String,
    pub root_node: TabNodeState,
    pub search: TerminalSearchState,
}

/// Messages processed by the Relm4-style unidirectional update loop (`AppModel::update`).
#[derive(Debug, Clone, PartialEq)]
pub enum AppInput {
    NewTab {
        title: Option<String>,
        cwd: Option<String>,
        session_data: Option<SessionItem>,
    },
    CloseTab(u64),
    SelectTab(usize),
    SplitActiveTab {
        orientation: String,
        cwd: Option<String>,
    },
    ToggleSidebar,
    ToggleFileManager,
    ToggleAiPanel,
    ToggleSearchBar,
    UpdateSearchQuery {
        query: String,
        total_matches: usize,
    },
    SearchNext,
    SearchPrev,
}

/// Top-level application state model (`OnyxWindow` in `src/onyxsh/window.py`).
#[derive(Debug, Clone)]
pub struct AppModel {
    pub tabs: Vec<WorkspaceTab>,
    pub active_tab_index: usize,
    pub sidebar_visible: bool,
    pub file_manager_visible: bool,
    pub ai_panel_visible: bool,
    next_tab_id: u64,
}

impl Default for AppModel {
    fn default() -> Self {
        let mut model = Self {
            tabs: Vec::new(),
            active_tab_index: 0,
            sidebar_visible: true,
            file_manager_visible: false,
            ai_panel_visible: false,
            next_tab_id: 1,
        };
        model.update(AppInput::NewTab {
            title: Some(tr("Local Terminal")),
            cwd: None,
            session_data: None,
        });
        model
    }
}

impl AppModel {
    /// Relm4-compatible unidirectional state transition reducer.
    pub fn update(&mut self, msg: AppInput) {
        match msg {
            AppInput::NewTab {
                title,
                cwd,
                session_data,
            } => {
                let id = self.next_tab_id;
                self.next_tab_id += 1;
                let tab_title = title.unwrap_or_else(|| tr_fmt("Terminal {id}", &[("id", &id.to_string())]));
                let session_type = session_data
                    .as_ref()
                    .map(|s| s.session_type.clone())
                    .unwrap_or_else(|| "local".to_string());
                let root_node = TabNodeState::Terminal {
                    session_type,
                    title: tab_title.clone(),
                    working_directory: cwd,
                    session_data,
                };
                self.tabs.push(WorkspaceTab {
                    id,
                    title: tab_title,
                    root_node,
                    search: TerminalSearchState::default(),
                });
                self.active_tab_index = self.tabs.len() - 1;
            }
            AppInput::CloseTab(tab_id) => {
                if let Some(pos) = self.tabs.iter().position(|t| t.id == tab_id) {
                    self.tabs.remove(pos);
                    if self.tabs.is_empty() {
                        self.active_tab_index = 0;
                    } else if self.active_tab_index >= self.tabs.len() {
                        self.active_tab_index = self.tabs.len() - 1;
                    }
                }
            }
            AppInput::SelectTab(index) => {
                if index < self.tabs.len() {
                    self.active_tab_index = index;
                }
            }
            AppInput::SplitActiveTab { orientation, cwd } => {
                if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
                    let existing = active.root_node.clone();
                    let new_leaf = TabNodeState::Terminal {
                        session_type: "local".to_string(),
                        title: active.title.clone(),
                        working_directory: cwd,
                        session_data: None,
                    };
                    active.root_node = TabNodeState::Paned {
                        orientation,
                        position: 450,
                        position_ratio: Some(0.5),
                        child1: Box::new(existing),
                        child2: Box::new(new_leaf),
                    };
                }
            }
            AppInput::ToggleSidebar => {
                self.sidebar_visible = !self.sidebar_visible;
            }
            AppInput::ToggleFileManager => {
                self.file_manager_visible = !self.file_manager_visible;
            }
            AppInput::ToggleAiPanel => {
                self.ai_panel_visible = !self.ai_panel_visible;
            }
            AppInput::ToggleSearchBar => {
                if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
                    active.search.visible = !active.search.visible;
                }
            }
            AppInput::UpdateSearchQuery {
                query,
                total_matches,
            } => {
                if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
                    active.search.query = query;
                    active.search.total_matches = total_matches;
                    active.search.current_match_index = 0;
                }
            }
            AppInput::SearchNext => {
                if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
                    active.search.next_match();
                }
            }
            AppInput::SearchPrev => {
                if let Some(active) = self.tabs.get_mut(self.active_tab_index) {
                    active.search.prev_match();
                }
            }
        }
    }

    /// Exports the current window layout to a [`WindowSessionState`] snapshot for `~/.config/onyxsh/state.json`.
    pub fn to_window_session_state(&self) -> WindowSessionState {
        WindowSessionState {
            tabs: self.tabs.iter().map(|t| t.root_node.clone()).collect(),
            active_tab_index: self.active_tab_index,
            sidebar_visible: self.sidebar_visible,
            ai_panel_visible: self.ai_panel_visible,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_model_tabs_splits_and_search() {
        let mut app = AppModel::default();
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs[0].root_node.terminal_count(), 1);

        // Split active tab horizontally
        app.update(AppInput::SplitActiveTab {
            orientation: "horizontal".to_string(),
            cwd: Some("/var/log".to_string()),
        });
        assert_eq!(app.tabs[0].root_node.terminal_count(), 2);

        // Search bar match cycling
        app.update(AppInput::ToggleSearchBar);
        app.update(AppInput::UpdateSearchQuery {
            query: "error".to_string(),
            total_matches: 3,
        });
        assert_eq!(app.tabs[0].search.counter_label(), "1/3");
        app.update(AppInput::SearchNext);
        assert_eq!(app.tabs[0].search.counter_label(), "2/3");
        app.update(AppInput::SearchPrev);
        assert_eq!(app.tabs[0].search.counter_label(), "1/3");
        app.update(AppInput::SearchPrev);
        assert_eq!(app.tabs[0].search.counter_label(), "3/3");
    }
}
