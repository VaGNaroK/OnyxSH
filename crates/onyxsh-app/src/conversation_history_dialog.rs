//! Conversation History Dialog for AI Chat (`historicoIA.png` and `src/onyxsh/ui/widgets/conversation_history.py`).
//!
//! Features:
//! - Multi-conversation storage persisted to `~/.config/onyxsh/ai_history.json`.
//! - Backward-compatible with legacy flat message lists and `{ "history": [...] }` formats.
//! - HeaderBar with `+` (`list-add-symbolic`) for starting a new conversation, title "Histórico de Conversa",
//!   and trash button (`user-trash-symbolic`) for deleting all conversations.
//! - Live `GtkSearchEntry` filtering conversations by title and message content.
//! - Relative date grouping headers: "Hoje", "Ontem", "Últimos 7 dias", "Este mês", "Mais velho".
//! - Action rows displaying title ("Nova Conversa" or first user prompt truncated), relative time
//!   ("Agora mesmo", "10 min", "2 hr", "Ontem", "ago 15"), and individual trash icon button.
//! - Destructive `adw::MessageDialog` confirmations for deleting a single conversation or all conversations.
//! - `adw::StatusPage` empty state when no conversations exist.

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::platform;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

/// Path to `~/.config/onyxsh/ai_history.json`.
pub fn ai_history_file_path() -> PathBuf {
    platform::config_dir().join("ai_history.json")
}

/// Represents a single message in an AI conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiConversationMessage {
    #[serde(default = "current_iso_timestamp")]
    pub timestamp: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub route_summary: String,
    #[serde(default)]
    pub commands: Option<Vec<String>>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
}

/// Represents a conversation session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiConversation {
    pub id: String,
    #[serde(default = "current_iso_timestamp")]
    pub created_at: String,
    #[serde(default)]
    pub messages: Vec<AiConversationMessage>,
}

/// Full persistence structure for AI chat history.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct AiHistoryStore {
    #[serde(default)]
    pub conversations: Vec<AiConversation>,
    #[serde(default)]
    pub current_conversation_id: Option<String>,
}

impl AiHistoryStore {
    /// Limit number of saved conversations to 100 (matching Python specification).
    pub const MAX_CONVERSATIONS: usize = 100;

    /// Creates a new conversation and sets it as the active one.
    pub fn new_conversation(&mut self) -> AiConversation {
        let conv_id = glib::uuid_string_random().to_string();
        let conv = AiConversation {
            id: conv_id.clone(),
            created_at: current_iso_timestamp(),
            messages: Vec::new(),
        };
        self.conversations.push(conv.clone());
        self.current_conversation_id = Some(conv_id);
        self.trim_excess();
        conv
    }

    /// Returns a reference to the active conversation, or creates one if none exists.
    pub fn ensure_current_conversation(&mut self) -> &mut AiConversation {
        if self.current_conversation_id.is_none() || self.conversations.is_empty() {
            self.new_conversation();
        }
        let cur_id = self.current_conversation_id.clone().unwrap_or_default();
        if let Some(pos) = self.conversations.iter().position(|c| c.id == cur_id) {
            &mut self.conversations[pos]
        } else {
            self.new_conversation();
            self.conversations.last_mut().expect("just added")
        }
    }

    /// Returns an optional reference to the current active conversation.
    pub fn get_current_conversation(&self) -> Option<&AiConversation> {
        let cur_id = self.current_conversation_id.as_deref()?;
        self.conversations.iter().find(|c| c.id == cur_id)
    }

    /// Sets the current conversation by ID.
    pub fn load_conversation(&mut self, conv_id: &str) -> bool {
        if self.conversations.iter().any(|c| c.id == conv_id) {
            self.current_conversation_id = Some(conv_id.to_string());
            true
        } else {
            false
        }
    }

    /// Deletes a specific conversation by ID.
    pub fn delete_conversation(&mut self, conv_id: &str) -> bool {
        if let Some(pos) = self.conversations.iter().position(|c| c.id == conv_id) {
            self.conversations.remove(pos);
            if self.current_conversation_id.as_deref() == Some(conv_id) {
                self.current_conversation_id = self.conversations.last().map(|c| c.id.clone());
            }
            true
        } else {
            false
        }
    }

    /// Clears all conversations and resets current ID.
    pub fn clear_all_history(&mut self) {
        self.conversations.clear();
        self.current_conversation_id = None;
    }

    /// Appends a user message to the current conversation.
    pub fn add_user_message(&mut self, content: &str) {
        if content.trim().is_empty() {
            return;
        }
        let conv = self.ensure_current_conversation();
        conv.messages.push(AiConversationMessage {
            timestamp: current_iso_timestamp(),
            role: "user".to_string(),
            content: content.trim().to_string(),
            route_summary: String::new(),
            commands: None,
            model: None,
            provider: None,
        });
    }

    /// Appends an assistant message to the current conversation.
    pub fn add_assistant_message(&mut self, content: &str, route_summary: &str) {
        if content.trim().is_empty() {
            return;
        }
        let conv = self.ensure_current_conversation();
        conv.messages.push(AiConversationMessage {
            timestamp: current_iso_timestamp(),
            role: "assistant".to_string(),
            content: content.trim().to_string(),
            route_summary: route_summary.to_string(),
            commands: None,
            model: None,
            provider: None,
        });
    }

    /// Trims conversations if count exceeds MAX_CONVERSATIONS.
    fn trim_excess(&mut self) {
        if self.conversations.len() > Self::MAX_CONVERSATIONS {
            let excess = self.conversations.len() - Self::MAX_CONVERSATIONS;
            self.conversations.drain(0..excess);
        }
    }
}

/// Generates the current local ISO 8601 timestamp string.
pub fn current_iso_timestamp() -> String {
    if let Ok(now) = glib::DateTime::now_local() {
        if let Ok(s) = now.format("%Y-%m-%dT%H:%M:%S") {
            return s.to_string();
        }
    }
    "2026-01-01T00:00:00".to_string()
}

/// Parses an ISO 8601 timestamp string into a `glib::DateTime`.
pub fn parse_iso_datetime(iso_str: &str) -> Option<glib::DateTime> {
    let trimmed = iso_str.trim();
    if trimmed.is_empty() {
        return None;
    }
    let local_tz = glib::TimeZone::local();
    if let Ok(dt) = glib::DateTime::from_iso8601(trimmed, Some(&local_tz)) {
        return Some(dt);
    }
    if let Ok(dt) = glib::DateTime::from_iso8601(trimmed, None) {
        return Some(dt);
    }
    let with_z = format!("{trimmed}Z");
    if let Ok(dt) = glib::DateTime::from_iso8601(&with_z, None) {
        return Some(dt);
    }
    None
}

/// Formats a created_at ISO timestamp into human-readable relative time matching Python OnyxSH:
/// - "Agora mesmo" (< 2 mins)
/// - "{mins} min"
/// - "{hours} hr"
/// - "Ontem" (yesterday)
/// - "{days} dias" (< 7 days)
/// - "{mes} {dia}" (e.g. "ago 15" as seen in `historicoIA.png`)
pub fn format_relative_time(created_at: &str) -> String {
    let Some(dt) = parse_iso_datetime(created_at) else {
        return String::new();
    };
    let Ok(now) = glib::DateTime::now_local() else {
        return String::new();
    };

    let diff_micros = now.difference(&dt);
    if diff_micros.0 < 0 {
        return "Agora mesmo".to_string();
    }
    let diff_secs = diff_micros.0 / 1_000_000;
    let diff_days = diff_secs / 86400;

    if diff_days == 0 {
        let hours = (diff_secs % 86400) / 3600;
        if hours == 0 {
            let mins = (diff_secs % 3600) / 60;
            if mins < 2 {
                "Agora mesmo".to_string()
            } else {
                format!("{mins} min")
            }
        } else {
            format!("{hours} hr")
        }
    } else if diff_days == 1 {
        "Ontem".to_string()
    } else if diff_days < 7 {
        format!("{diff_days} dias")
    } else {
        let month_str = match dt.month() {
            1 => "jan",
            2 => "fev",
            3 => "mar",
            4 => "abr",
            5 => "mai",
            6 => "jun",
            7 => "jul",
            8 => "ago",
            9 => "set",
            10 => "out",
            11 => "nov",
            12 => "dez",
            _ => "",
        };
        format!("{} {:02}", month_str, dt.day_of_month())
    }
}

/// Returns the time grouping label matching `historicoIA.png` and Python `_get_time_group`:
/// - "Hoje"
/// - "Ontem"
/// - "Últimos 7 dias"
/// - "Este mês"
/// - "Mais velho"
pub fn get_time_group(created_at: &str) -> &'static str {
    let Some(dt) = parse_iso_datetime(created_at) else {
        return "Mais velho";
    };
    let Ok(now) = glib::DateTime::now_local() else {
        return "Mais velho";
    };

    if dt.year() == now.year()
        && dt.month() == now.month()
        && dt.day_of_month() == now.day_of_month()
    {
        return "Hoje";
    }

    let diff_micros = now.difference(&dt);
    if diff_micros.0 < 0 {
        return "Hoje";
    }
    let diff_secs = diff_micros.0 / 1_000_000;
    let diff_days = diff_secs / 86400;

    if diff_days == 1 {
        "Ontem"
    } else if diff_days < 7 {
        "Últimos 7 dias"
    } else if dt.year() == now.year() && dt.month() == now.month() {
        "Este mês"
    } else {
        "Mais velho"
    }
}

/// Extracts a conversation title from its first user message, or defaults to "Nova Conversa".
pub fn conversation_title(conv: &AiConversation) -> String {
    for msg in &conv.messages {
        if msg.role == "user" {
            let clean = msg.content.replace('\n', " ").trim().to_string();
            if !clean.is_empty() {
                if clean.chars().count() > 45 {
                    let truncated: String = clean.chars().take(45).collect();
                    return format!("{truncated}…");
                }
                return clean;
            }
        }
    }
    "Nova Conversa".to_string()
}

/// Extracts a preview subtitle from the last message in the conversation.
pub fn conversation_subtitle(conv: &AiConversation) -> String {
    if let Some(last_msg) = conv.messages.last() {
        let clean = last_msg.content.replace('\n', " ").trim().to_string();
        if !clean.is_empty() {
            let role_prefix = if last_msg.role == "user" {
                "Você: "
            } else {
                "IA: "
            };
            let preview = if clean.chars().count() > 40 {
                let truncated: String = clean.chars().take(40).collect();
                format!("{truncated}…")
            } else {
                clean
            };
            return format!("{role_prefix}{preview}");
        }
    }
    String::new()
}

/// Loads the `AiHistoryStore` from `~/.config/onyxsh/ai_history.json`.
/// Handles migration from legacy flat list `[AiChatEntry]` and `{ "history": [...] }`.
pub fn load_history_store() -> AiHistoryStore {
    let path = ai_history_file_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(store) = serde_json::from_str::<AiHistoryStore>(&raw) {
            if !store.conversations.is_empty() || store.current_conversation_id.is_some() {
                return store;
            }
        }
        #[derive(Deserialize)]
        struct LegacyHistoryObj {
            history: Vec<AiConversationMessage>,
        }
        if let Ok(obj) = serde_json::from_str::<LegacyHistoryObj>(&raw) {
            if !obj.history.is_empty() {
                let id = glib::uuid_string_random().to_string();
                let created = obj
                    .history
                    .first()
                    .map(|m| m.timestamp.clone())
                    .unwrap_or_else(current_iso_timestamp);
                let conv = AiConversation {
                    id: id.clone(),
                    created_at: created,
                    messages: obj.history,
                };
                return AiHistoryStore {
                    conversations: vec![conv],
                    current_conversation_id: Some(id),
                };
            }
        }
        if let Ok(list) = serde_json::from_str::<Vec<AiConversationMessage>>(&raw) {
            if !list.is_empty() {
                let id = glib::uuid_string_random().to_string();
                let created = list
                    .first()
                    .map(|m| m.timestamp.clone())
                    .unwrap_or_else(current_iso_timestamp);
                let conv = AiConversation {
                    id: id.clone(),
                    created_at: created,
                    messages: list,
                };
                return AiHistoryStore {
                    conversations: vec![conv],
                    current_conversation_id: Some(id),
                };
            }
        }
    }
    AiHistoryStore::default()
}

/// Atomically saves the `AiHistoryStore` to `~/.config/onyxsh/ai_history.json`.
pub fn save_history_store(store: &AiHistoryStore) -> std::io::Result<()> {
    let path = ai_history_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(store).map_err(std::io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

/// Displays the native Libadwaita Conversation History Window matching `historicoIA.png` and
/// `src/onyxsh/ui/widgets/conversation_history.py`.
pub fn show_conversation_history_dialog<F>(
    parent: &adw::ApplicationWindow,
    history_store_rc: Rc<RefCell<AiHistoryStore>>,
    on_conversation_selected: F,
) where
    F: Fn(&str) + 'static,
{
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Histórico de Conversa")
        .default_width(420)
        .default_height(520)
        .build();

    let root_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root_vbox.add_css_class("background");

    // ── HeaderBar ─────────────────────────────────────────────────────────────
    let header_bar = adw::HeaderBar::new();
    header_bar.set_show_end_title_buttons(true);
    header_bar.set_show_start_title_buttons(false);

    let title_widget = adw::WindowTitle::new("Histórico de Conversa", "");
    header_bar.set_title_widget(Some(&title_widget));

    let btn_new_conv = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Nova Conversa")
        .css_classes(["flat"])
        .build();
    header_bar.pack_start(&btn_new_conv);

    let btn_clear_all = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Excluir Todas as Conversas")
        .css_classes(["flat"])
        .build();
    header_bar.pack_end(&btn_clear_all);

    root_vbox.append(&header_bar);

    // ── Search Entry ──────────────────────────────────────────────────────────
    let search_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    search_box.set_margin_start(12);
    search_box.set_margin_end(12);
    search_box.set_margin_top(6);
    search_box.set_margin_bottom(6);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Pesquisar...")
        .hexpand(true)
        .build();
    search_box.append(&search_entry);
    root_vbox.append(&search_box);

    // ── Stack (List vs Empty State) ───────────────────────────────────────────
    let stack = gtk4::Stack::builder()
        .transition_type(gtk4::StackTransitionType::Crossfade)
        .vexpand(true)
        .build();

    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .build();

    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    list_box.add_css_class("navigation-sidebar");

    scrolled.set_child(Some(&list_box));
    stack.add_named(&scrolled, Some("list"));

    let empty_page = adw::StatusPage::builder()
        .icon_name("chat-symbolic")
        .title("Nenhuma Conversa")
        .description("Comece a conversar para criar sua primeira conversa")
        .build();
    stack.add_named(&empty_page, Some("empty"));

    root_vbox.append(&stack);
    dlg.set_content(Some(&root_vbox));

    // ── Group Headers via header_func ─────────────────────────────────────────
    let row_group_map: Rc<RefCell<HashMap<String, &'static str>>> =
        Rc::new(RefCell::new(HashMap::new()));
    {
        let g_map = Rc::clone(&row_group_map);
        list_box.set_header_func(move |row, before| {
            let cur_id = row.widget_name().to_string();
            let cur_group = g_map
                .borrow()
                .get(&cur_id)
                .copied()
                .unwrap_or("Mais velho");

            let need_header = match before {
                None => true,
                Some(prev) => {
                    let prev_id = prev.widget_name().to_string();
                    let prev_group = g_map
                        .borrow()
                        .get(&prev_id)
                        .copied()
                        .unwrap_or("Mais velho");
                    prev_group != cur_group
                }
            };

            if need_header {
                let lbl = gtk4::Label::new(Some(cur_group));
                lbl.set_halign(gtk4::Align::Start);
                lbl.add_css_class("caption");
                lbl.add_css_class("dim-label");
                lbl.set_margin_start(16);
                lbl.set_margin_top(14);
                lbl.set_margin_bottom(6);
                row.set_header(Some(&lbl));
            } else {
                row.set_header(None::<&gtk4::Widget>);
            }
        });
    }

    let on_select_rc = Rc::new(on_conversation_selected);

    // ── Population / Refresh Logic ────────────────────────────────────────────
    let populate_list = {
        let lb = list_box.clone();
        let stk = stack.clone();
        let store_rc = Rc::clone(&history_store_rc);
        let g_map = Rc::clone(&row_group_map);
        let s_entry = search_entry.clone();
        let dlg_weak = dlg.downgrade();
        let on_sel = Rc::clone(&on_select_rc);

        Rc::new(move || {
            while let Some(child) = lb.first_child() {
                lb.remove(&child);
            }
            g_map.borrow_mut().clear();

            let query = s_entry.text().to_string();
            let query_lower = query.trim().to_lowercase();

            let store = store_rc.borrow();
            let mut convs = store.conversations.clone();
            drop(store);

            // Filter by search query if present
            if !query_lower.is_empty() {
                convs.retain(|c| {
                    let title = conversation_title(c).to_lowercase();
                    title.contains(&query_lower)
                        || c.messages
                            .iter()
                            .any(|m| m.content.to_lowercase().contains(&query_lower))
                });
            }

            if convs.is_empty() {
                stk.set_visible_child_name("empty");
                return;
            }

            stk.set_visible_child_name("list");

            // Sort newest first
            convs.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            for conv in convs {
                let conv_id = conv.id.clone();
                let title = conversation_title(&conv);
                let subtitle = conversation_subtitle(&conv);
                let time_group = get_time_group(&conv.created_at);
                let time_str = format_relative_time(&conv.created_at);

                g_map.borrow_mut().insert(conv_id.clone(), time_group);

                let row = adw::ActionRow::builder()
                    .title(&title)
                    .activatable(true)
                    .build();
                if !subtitle.is_empty() {
                    row.set_subtitle(&subtitle);
                }
                row.set_widget_name(&conv_id);

                if !time_str.is_empty() {
                    let time_lbl = gtk4::Label::new(Some(&time_str));
                    time_lbl.add_css_class("caption");
                    time_lbl.add_css_class("dim-label");
                    time_lbl.set_valign(gtk4::Align::Center);
                    row.add_suffix(&time_lbl);
                }

                // Delete trash button matching `historicoIA.png`
                let del_btn = gtk4::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .valign(gtk4::Align::Center)
                    .tooltip_text("Excluir")
                    .css_classes(["flat", "circular"])
                    .build();

                {
                    let store_rc_del = Rc::clone(&store_rc);
                    let c_id = conv_id.clone();
                    let dlg_w = dlg_weak.clone();
                    let pop_fn = Rc::clone(&on_sel);

                    del_btn.connect_clicked(move |_| {
                        let Some(window) = dlg_w.upgrade() else {
                            return;
                        };
                        let confirm = adw::MessageDialog::builder()
                            .heading("Excluir Conversa?")
                            .body("Esta conversa será excluída permanentemente.")
                            .transient_for(&window)
                            .close_response("cancel")
                            .build();
                        confirm.add_response("cancel", "Cancelar");
                        confirm.add_response("delete", "Excluir");
                        confirm.set_response_appearance(
                            "delete",
                            adw::ResponseAppearance::Destructive,
                        );
                        confirm.set_default_response(Some("cancel"));

                        let s_rc = Rc::clone(&store_rc_del);
                        let del_id = c_id.clone();
                        let on_s = Rc::clone(&pop_fn);
                        let win_ref = window.clone();

                        confirm.connect_response(None, move |_, resp| {
                            if resp == "delete" {
                                let mut s = s_rc.borrow_mut();
                                let was_active = s.current_conversation_id.as_deref() == Some(&del_id);
                                s.delete_conversation(&del_id);
                                if was_active {
                                    if let Some(active_id) = s.current_conversation_id.clone() {
                                        on_s(&active_id);
                                    } else {
                                        let new_conv = s.new_conversation();
                                        on_s(&new_conv.id);
                                    }
                                }
                                let _ = save_history_store(&s);
                                drop(s);
                                win_ref.close();
                            }
                        });
                        confirm.present();
                    });
                }

                row.add_suffix(&del_btn);
                lb.append(&row);
            }
        })
    };

    populate_list();

    // ── Row Activation (Open Conversation) ────────────────────────────────────
    {
        let store_rc = Rc::clone(&history_store_rc);
        let dlg_weak = dlg.downgrade();
        let on_sel = Rc::clone(&on_select_rc);

        list_box.connect_row_activated(move |_, row| {
            let conv_id = row.widget_name().to_string();
            if !conv_id.is_empty() {
                let mut store = store_rc.borrow_mut();
                store.load_conversation(&conv_id);
                let _ = save_history_store(&store);
                drop(store);

                on_sel(&conv_id);
                if let Some(w) = dlg_weak.upgrade() {
                    w.close();
                }
            }
        });
    }

    // ── Search Changed ────────────────────────────────────────────────────────
    {
        let pop = Rc::clone(&populate_list);
        search_entry.connect_search_changed(move |_| {
            pop();
        });
    }

    // ── New Conversation Button (+) ───────────────────────────────────────────
    {
        let store_rc = Rc::clone(&history_store_rc);
        let dlg_weak = dlg.downgrade();
        let on_sel = Rc::clone(&on_select_rc);

        btn_new_conv.connect_clicked(move |_| {
            let mut store = store_rc.borrow_mut();
            let new_conv = store.new_conversation();
            let new_id = new_conv.id.clone();
            let _ = save_history_store(&store);
            drop(store);

            on_sel(&new_id);
            if let Some(w) = dlg_weak.upgrade() {
                w.close();
            }
        });
    }

    // ── Clear All Conversations Button (Trash in Header) ───────────────────────
    {
        let store_rc = Rc::clone(&history_store_rc);
        let dlg_weak = dlg.downgrade();
        let on_sel = Rc::clone(&on_select_rc);

        btn_clear_all.connect_clicked(move |_| {
            let Some(window) = dlg_weak.upgrade() else {
                return;
            };
            let count = store_rc.borrow().conversations.len();
            if count == 0 {
                return;
            }

            let confirm = adw::MessageDialog::builder()
                .heading("Excluir Todas as Conversas?")
                .body(&format!(
                    "Isso excluirá permanentemente todas as {} conversas. Esta ação não pode ser desfeita.",
                    count
                ))
                .transient_for(&window)
                .close_response("cancel")
                .build();
            confirm.add_response("cancel", "Cancelar");
            confirm.add_response("delete", "Excluir Tudo");
            confirm.set_response_appearance(
                "delete",
                adw::ResponseAppearance::Destructive,
            );
            confirm.set_default_response(Some("cancel"));

            let s_rc = Rc::clone(&store_rc);
            let on_s = Rc::clone(&on_sel);
            let win_ref = window.clone();

            confirm.connect_response(None, move |_, resp| {
                if resp == "delete" {
                    let mut s = s_rc.borrow_mut();
                    s.clear_all_history();
                    let new_conv = s.new_conversation();
                    let new_id = new_conv.id.clone();
                    let _ = save_history_store(&s);
                    drop(s);

                    on_s(&new_id);
                    win_ref.close();
                }
            });
            confirm.present();
        });
    }

    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_title_and_subtitle() {
        let mut conv = AiConversation {
            id: "test-conv-1".to_string(),
            created_at: "2026-11-09T00:00:00".to_string(),
            messages: Vec::new(),
        };

        // Empty conversation defaults to "Nova Conversa"
        assert_eq!(conversation_title(&conv), "Nova Conversa");
        assert_eq!(conversation_subtitle(&conv), "");

        // User message becomes title
        conv.messages.push(AiConversationMessage {
            timestamp: "2026-11-09T00:01:00".to_string(),
            role: "user".to_string(),
            content: "Como listar arquivos ordenados por tamanho no Linux?".to_string(),
            route_summary: "".to_string(),
            commands: None,
            model: None,
            provider: None,
        });

        assert_eq!(
            conversation_title(&conv),
            "Como listar arquivos ordenados por tamanho no…"
        );
        assert_eq!(
            conversation_subtitle(&conv),
            "Você: Como listar arquivos ordenados por taman…"
        );

        // Assistant reply becomes subtitle
        conv.messages.push(AiConversationMessage {
            timestamp: "2026-11-09T00:01:05".to_string(),
            role: "assistant".to_string(),
            content: "Use o comando `ls -lSh` no terminal.".to_string(),
            route_summary: "fast".to_string(),
            commands: None,
            model: None,
            provider: None,
        });

        assert_eq!(
            conversation_subtitle(&conv),
            "IA: Use o comando `ls -lSh` no terminal."
        );
    }

    #[test]
    fn test_store_conversation_crud_and_trim() {
        let mut store = AiHistoryStore::default();
        assert!(store.conversations.is_empty());
        assert!(store.current_conversation_id.is_none());

        // Add user & assistant messages
        store.add_user_message("primeiro prompt");
        assert_eq!(store.conversations.len(), 1);
        assert!(store.current_conversation_id.is_some());

        store.add_assistant_message("resposta 1", "local:llama");
        let conv = store.get_current_conversation().unwrap();
        assert_eq!(conv.messages.len(), 2);

        // Create second conversation
        let c2 = store.new_conversation();
        assert_eq!(store.conversations.len(), 2);
        assert_eq!(store.current_conversation_id.as_ref().unwrap(), &c2.id);

        // Load first conversation
        let c1_id = store.conversations[0].id.clone();
        assert!(store.load_conversation(&c1_id));
        assert_eq!(store.current_conversation_id.as_ref().unwrap(), &c1_id);

        // Delete conversation
        assert!(store.delete_conversation(&c2.id));
        assert_eq!(store.conversations.len(), 1);

        // Clear all
        store.clear_all_history();
        assert!(store.conversations.is_empty());
        assert!(store.current_conversation_id.is_none());
    }

    #[test]
    fn test_time_grouping_and_relative_time() {
        let now_str = current_iso_timestamp();
        assert_eq!(get_time_group(&now_str), "Hoje");
        assert_eq!(format_relative_time(&now_str), "Agora mesmo");

        // Invalid or empty returns default
        assert_eq!(get_time_group(""), "Mais velho");
        assert_eq!(format_relative_time(""), "");

        // Far past date (August 15)
        let past = "2024-08-15T12:00:00";
        assert_eq!(get_time_group(past), "Mais velho");
        assert_eq!(format_relative_time(past), "ago 15");
    }

    #[test]
    fn test_legacy_format_backward_compatibility() {
        let legacy_json = r#"[
            {
                "timestamp": "2026-11-09T01:00:00",
                "role": "user",
                "content": "olá mundo",
                "route_summary": "fast"
            },
            {
                "timestamp": "2026-11-09T01:00:02",
                "role": "assistant",
                "content": "olá! como posso ajudar?",
                "route_summary": "fast"
            }
        ]"#;

        let parsed: Vec<AiConversationMessage> = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].content, "olá mundo");
        assert_eq!(parsed[1].content, "olá! como posso ajudar?");
    }
}
