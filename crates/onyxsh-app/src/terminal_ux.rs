//! Phase 16: Advanced Terminal UX — Autocomplete Popup (`CompletionEngine`),
//! Selection Context Popover, Production Safety Banner & Confirmation Dialog (`ProductionGuard`),
//! and Unified Command Palette (`Ctrl+Shift+P`).
//!
//! Ports:
//! - `src/onyxsh/terminal/completion/popup.py`
//! - `src/onyxsh/ui/widgets/selection_popover.py`
//! - `src/onyxsh/ui/widgets/production_banner.py` & `ui/dialogs/production_confirm_dialog.py`
//! - `src/onyxsh/ui/dialogs/command_palette.py`

use crate::command_manager_ui::CommandButton;
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::command_history::CommandHistoryStore;
use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::{platform, APP_ID};
use onyxsh_core::sessions::{SessionItem, SessionsStorageData};
use onyxsh_terminal::completion::{CompletionEngine, CompletionItem, CompletionType};
use onyxsh_terminal::production_guard::{GuardViolation, ProductionGuard};
use onyxsh_terminal::pty_proxy::OscStreamEvent;
use onyxsh_terminal::semantic_tracker::{SemanticCommand, SemanticTerminalState};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};
use vte4::prelude::*;

#[derive(Debug, Default)]
pub struct TerminalUxState {
    pub semantic: SemanticTerminalState,
    pub alt_screen: bool,
    pub is_running_command: bool,
    pub command_started_at: Option<Instant>,
    pub last_cwd: String,
    pub is_production: bool,
    pub last_exit_code: Option<i32>,
    pub recorded_commands: Vec<SemanticCommand>,
    pub last_command_output: Option<(String, String)>,
    pub captured_outputs: HashMap<String, String>,
}

thread_local! {
    static TERMINAL_UX_STATES: RefCell<HashMap<String, TerminalUxState>> = RefCell::new(HashMap::new());
    static GLOBAL_PROD_SHIELD_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

fn term_key(term: &vte4::Terminal) -> String {
    term.widget_name().to_string()
}

pub fn init_terminal_ux_state(term: &vte4::Terminal, is_production: bool, initial_cwd: &str) {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| {
        let mut guard = map.borrow_mut();
        let entry = guard.entry(key).or_default();
        entry.is_production = is_production;
        if !initial_cwd.is_empty() {
            entry.last_cwd = initial_cwd.to_string();
        }
    });
}

pub fn remove_terminal_ux_state(term: &vte4::Terminal) {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| {
        map.borrow_mut().remove(&key);
    });
}

pub fn set_terminal_production_mode(term: &vte4::Terminal, is_prod: bool) {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| {
        map.borrow_mut().entry(key).or_default().is_production = is_prod;
    });
}

pub fn is_terminal_production_effective(term: &vte4::Terminal) -> bool {
    if GLOBAL_PROD_SHIELD_ACTIVE.with(|c| c.get()) {
        return true;
    }
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| map.borrow().get(&key).is_some_and(|s| s.is_production))
}

pub fn is_autocomplete_allowed_for_terminal(term: &vte4::Terminal) -> bool {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| {
        map.borrow()
            .get(&key)
            .is_none_or(|s| !s.alt_screen && !s.is_running_command)
    })
}

pub fn is_terminal_alt_screen(term: &vte4::Terminal) -> bool {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| map.borrow().get(&key).is_some_and(|s| s.alt_screen))
}

/// Resolves the non-empty confirmation hostname/target for `ProductionGuard` dialogs.
/// Priority: `session.host` -> `session.name` -> local OS hostname -> `"production"`.
pub fn resolve_production_target_hostname(term_opt: Option<&vte4::Terminal>) -> String {
    if let Some(term) = term_opt {
        if let Some(sess) = crate::session_sidebar::get_pane_session(term) {
            if !sess.host.trim().is_empty() {
                return sess.host.trim().to_string();
            }
            if !sess.name.trim().is_empty() {
                return sess.name.trim().to_string();
            }
        }
    }
    if let Ok(hostname) = std::fs::read_to_string("/etc/hostname") {
        let h = hostname.trim();
        if !h.is_empty() {
            return h.to_string();
        }
    }
    "production".to_string()
}

pub fn should_notify_long_command(
    condition: &str,
    exit_code: i32,
    window_active: bool,
    tab_selected: bool,
    term_has_focus: bool,
) -> bool {
    match condition {
        "always" => true,
        "errors" | "errors_only" => exit_code != 0,
        _ => !window_active || !tab_selected || !term_has_focus,
    }
}

fn has_installed_desktop_entry() -> bool {
    let filename = format!("{APP_ID}.desktop");
    if platform::data_dir().join("../applications").join(&filename).is_file() {
        return true;
    }
    let home_apps = platform::home_dir()
        .join(".local/share/applications")
        .join(&filename);
    if home_apps.is_file() {
        return true;
    }
    if let Ok(xdg_dirs) = std::env::var("XDG_DATA_DIRS") {
        for base in xdg_dirs.split(':').filter(|s| !s.is_empty()) {
            if std::path::Path::new(base)
                .join("applications")
                .join(&filename)
                .is_file()
            {
                return true;
            }
        }
    }
    std::path::Path::new("/usr/share/applications")
        .join(&filename)
        .is_file()
}

fn dispatch_desktop_notification(window: &adw::ApplicationWindow, title: &str, body: &str) {
    if has_installed_desktop_entry() {
        if let Some(app) = window.application() {
            let notif = gio::Notification::new(title);
            notif.set_body(Some(body));
            notif.set_icon(&gio::ThemedIcon::new("utilities-terminal-symbolic"));
            app.send_notification(Some("onyxsh-cmd-complete"), &notif);
            return;
        }
    }

    let raw_cmd = vec![
        "notify-send".to_string(),
        "-a".to_string(),
        "OnyxSH".to_string(),
        "-i".to_string(),
        "utilities-terminal".to_string(),
        "-u".to_string(),
        "normal".to_string(),
        title.to_string(),
        body.to_string(),
    ];
    let wrapped = platform::CommandBuilder::new().wrap_host_command(&raw_cmd, None, None);
    if let Some((bin, args)) = wrapped.split_first() {
        if std::process::Command::new(bin)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return;
        }
    }
    if let Some(app) = window.application() {
        let notif = gio::Notification::new(title);
        notif.set_body(Some(body));
        app.send_notification(Some("onyxsh-cmd-complete"), &notif);
    }
}

pub fn last_exit_code_for(term: &vte4::Terminal) -> Option<i32> {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| map.borrow().get(&key).and_then(|s| s.last_exit_code))
}

pub fn previous_prompt_row_for(term: &vte4::Terminal, current_row: i64) -> Option<i64> {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| {
        map.borrow()
            .get(&key)
            .and_then(|s| s.semantic.previous_prompt_row(current_row))
    })
}

pub fn next_prompt_row_for(term: &vte4::Terminal, current_row: i64) -> Option<i64> {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| {
        map.borrow()
            .get(&key)
            .and_then(|s| s.semantic.next_prompt_row(current_row))
    })
}

pub fn recorded_semantic_commands_for(term: &vte4::Terminal) -> Vec<SemanticCommand> {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| {
        map.borrow()
            .get(&key)
            .map(|s| s.recorded_commands.clone())
            .unwrap_or_default()
    })
}

pub fn captured_output_for_command_id(term: &vte4::Terminal, command_id: &str) -> Option<String> {
    let key = term_key(term);
    TERMINAL_UX_STATES.with(|map| {
        map.borrow()
            .get(&key)
            .and_then(|s| s.captured_outputs.get(command_id).cloned())
    })
}

pub fn last_command_output_for(term: &vte4::Terminal) -> Option<(String, String)> {
    let key = term_key(term);
    if let Some(captured) = TERMINAL_UX_STATES.with(|map| {
        map.borrow()
            .get(&key)
            .and_then(|s| s.last_command_output.clone())
    }) {
        return Some(captured);
    }

    let last_cmd = TERMINAL_UX_STATES.with(|map| {
        map.borrow()
            .get(&key)
            .and_then(|s| s.recorded_commands.last().cloned())
    })?;
    let start_row = last_cmd.output_start_row.max(last_cmd.prompt_row);
    let end_row = last_cmd.output_end_row.unwrap_or_else(|| term.cursor_position().1);
    let cols = term.column_count().max(1);
    let (txt_opt, _) = term.text_range_format(
        vte4::Format::Text,
        start_row.min(end_row),
        0,
        end_row.max(start_row),
        cols,
    );
    let out = txt_opt.map(|g| g.to_string()).unwrap_or_default();
    Some((last_cmd.command_text, out))
}

fn record_command_history_entry(term: &vte4::Terminal, cmd_text: &str, cwd: &str) {
    if cmd_text.trim().is_empty() {
        return;
    }
    if let Ok(store) = CommandHistoryStore::open_default() {
        let sess_opt = crate::session_sidebar::get_pane_session(term);
        let host = sess_opt
            .as_ref()
            .map(|s| s.connection_string())
            .unwrap_or_else(|| "localhost".to_string());
        let sess_name = sess_opt
            .as_ref()
            .map(|s| s.name.as_str())
            .unwrap_or("");
        let _ = store.record_command(cmd_text, cwd, &host, sess_name, None, 0);
    }
}

/// Processes a single live [`OscStreamEvent`] from the PTY proxy for a `vte4::Terminal`.
pub fn handle_osc_stream_event(
    term: &vte4::Terminal,
    event: &OscStreamEvent,
    settings: &AppSettings,
    window: &adw::ApplicationWindow,
    tab_page: Option<&adw::TabPage>,
    status_lbl: &gtk4::Label,
    on_cwd_sync: Option<&Rc<dyn Fn(&str)>>,
) {
    let key = term_key(term);
    let (_, cursor_row) = term.cursor_position();

    match event {
        OscStreamEvent::PromptStart => {
            TERMINAL_UX_STATES.with(|map| {
                let mut guard = map.borrow_mut();
                let st = guard.entry(key.clone()).or_default();
                st.semantic.start_prompt(cursor_row);
                st.is_running_command = false;
                st.command_started_at = None;
            });
            if let Some(page) = tab_page {
                page.set_loading(false);
            }
        }
        OscStreamEvent::CommandStart => {
            TERMINAL_UX_STATES.with(|map| {
                let mut guard = map.borrow_mut();
                let st = guard.entry(key.clone()).or_default();
                st.semantic.start_command(cursor_row);
            });
        }
        OscStreamEvent::CommandExecuted { command } => {
            let cmd_text = command.clone().unwrap_or_default();
            let cwd = TERMINAL_UX_STATES.with(|map| {
                let mut guard = map.borrow_mut();
                let st = guard.entry(key.clone()).or_default();
                st.semantic.execute_command(cursor_row, &cmd_text, &st.last_cwd);
                st.is_running_command = true;
                st.command_started_at = Some(Instant::now());
                st.last_cwd.clone()
            });
            record_command_history_entry(term, &cmd_text, &cwd);
            if let Some(page) = tab_page {
                page.set_loading(true);
                page.set_icon(None::<&gio::Icon>);
                page.set_indicator_icon(None::<&gio::Icon>);
                page.set_indicator_activatable(false);
                page.set_indicator_tooltip("");
                page.set_tooltip("");
            }
        }
        OscStreamEvent::CommandFinished {
            exit_code,
            command,
            output,
        } => {
            let (finished_cmd, elapsed_secs, deferred_cmd_cwd) = TERMINAL_UX_STATES.with(|map| {
                let mut guard = map.borrow_mut();
                let st = guard.entry(key.clone()).or_default();
                let elapsed = st
                    .command_started_at
                    .map(|t| t.elapsed().as_secs_f64())
                    .unwrap_or(0.0);
                let mut finished = st.semantic.finish_command(cursor_row, *exit_code, elapsed);
                finished.output_end_row = Some(cursor_row);

                let had_cmd_before = !finished.command_text.trim().is_empty();
                if !had_cmd_before {
                    if let Some(resolved_cmd) = command.as_ref().filter(|s| !s.trim().is_empty()) {
                        finished.command_text = resolved_cmd.trim().to_string();
                    }
                }
                if finished.cwd.is_empty() {
                    finished.cwd = st.last_cwd.clone();
                }

                st.is_running_command = false;
                st.command_started_at = None;
                st.last_exit_code = Some(*exit_code);

                if let Some(out_str) = output {
                    // Only treat this as an executed command (not an empty Enter at prompt) when
                    // PTY stream captured an OSC 133;C -> 133;D lifecycle (`output.is_some()`).
                    let display_cmd = if finished.command_text.trim().is_empty() {
                        "(comando)".to_string()
                    } else {
                        finished.command_text.clone()
                    };
                    st.last_command_output = Some((display_cmd, out_str.clone()));
                    st.captured_outputs
                        .insert(finished.command_id.clone(), out_str.clone());
                    if st.recorded_commands.len() >= 200 {
                        let dropped = st.recorded_commands.remove(0);
                        st.captured_outputs.remove(&dropped.command_id);
                    }
                    if finished.command_text.trim().is_empty() {
                        finished.command_text = "(comando)".to_string();
                    }
                    st.recorded_commands.push(finished.clone());
                } else if !finished.command_text.trim().is_empty() {
                    if st.recorded_commands.len() >= 200 {
                        let dropped = st.recorded_commands.remove(0);
                        st.captured_outputs.remove(&dropped.command_id);
                    }
                    st.recorded_commands.push(finished.clone());
                }

                let deferred = (!had_cmd_before && output.is_some() && !finished.command_text.is_empty() && finished.command_text != "(comando)")
                    .then(|| (finished.command_text.clone(), finished.cwd.clone()));
                (finished, elapsed, deferred)
            });

            if let Some((cmd_txt, cwd_txt)) = deferred_cmd_cwd {
                record_command_history_entry(term, &cmd_txt, &cwd_txt);
            }

            if let Some(page) = tab_page {
                page.set_loading(false);
                if *exit_code != 0 {
                    let err_msg = tr_fmt(
                        "Último comando falhou (código {code}, {dur}s)",
                        &[
                            ("code", &exit_code.to_string()),
                            ("dur", &format!("{elapsed_secs:.1}")),
                        ],
                    );
                    let err_icon = gio::ThemedIcon::new("dialog-error-symbolic");
                    page.set_icon(Some(&err_icon));
                    page.set_indicator_icon(Some(&err_icon));
                    page.set_indicator_activatable(true);
                    page.set_indicator_tooltip(&err_msg);
                    page.set_tooltip(&err_msg);
                    status_lbl.set_text(&err_msg);
                } else {
                    page.set_icon(None::<&gio::Icon>);
                    page.set_indicator_icon(None::<&gio::Icon>);
                    page.set_indicator_activatable(false);
                    page.set_indicator_tooltip("");
                    page.set_tooltip("");
                }
            } else if *exit_code != 0 {
                status_lbl.set_text(&tr_fmt(
                    "Último comando falhou (código {code}, {dur}s)",
                    &[
                        ("code", &exit_code.to_string()),
                        ("dur", &format!("{elapsed_secs:.1}")),
                    ],
                ));
            }

            // Desktop Notification when long-running command finishes
            let threshold = settings.notify_long_commands_threshold as f64;
            if settings.notify_long_commands && elapsed_secs >= threshold {
                let is_tab_selected = tab_page.is_some_and(|p| p.is_selected());
                if should_notify_long_command(
                    settings.notify_long_commands_condition.as_str(),
                    *exit_code,
                    window.is_active(),
                    is_tab_selected,
                    term.has_focus(),
                ) {
                    let cmd_name = if finished_cmd.command_text.is_empty() {
                        "Terminal".to_string()
                    } else {
                        finished_cmd.command_text.clone()
                    };
                    let notif_title = if *exit_code == 0 {
                        tr_fmt(
                            "✅ Comando concluído ({dur}s)",
                            &[("dur", &format!("{elapsed_secs:.1}"))],
                        )
                    } else {
                        tr_fmt(
                            "❌ Comando falhou (código {code}, {dur}s)",
                            &[
                                ("code", &exit_code.to_string()),
                                ("dur", &format!("{elapsed_secs:.1}")),
                            ],
                        )
                    };
                    dispatch_desktop_notification(window, &notif_title, &cmd_name);
                }
            }
        }
        OscStreamEvent::CwdChanged { display_path, .. } => {
            TERMINAL_UX_STATES.with(|map| {
                map.borrow_mut().entry(key.clone()).or_default().last_cwd = display_path.clone();
            });
            if let Some(cb) = on_cwd_sync {
                cb(display_path);
            }
        }
        OscStreamEvent::ClipboardSet { text, .. } => {
            if settings.enable_osc52_clipboard {
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(text);
                    status_lbl.set_text(&tr("Conteúdo recebido via OSC 52 copiado para área de transferência."));
                }
            }
        }
        OscStreamEvent::AltScreenChanged(active) => {
            TERMINAL_UX_STATES.with(|map| {
                map.borrow_mut().entry(key.clone()).or_default().alt_screen = *active;
            });
        }
        OscStreamEvent::ChildExited(_) => {
            if let Some(page) = tab_page {
                page.set_loading(false);
            }
        }
    }
}

/// Entry in the unified Command Palette (`Ctrl+Shift+P`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem {
    pub category: String,
    pub title: String,
    pub subtitle: String,
    pub action_key: String,
}

/// Extracts the active command line typed after the shell prompt marker (`$ `, `# `, `% `, `❯ `, `> `).
pub fn extract_prompt_command_line(full_line: &str) -> String {
    let trimmed = full_line.trim_end_matches(['\r', '\n']);
    let markers = ["$ ", "# ", "% ", "❯ ", "➜ ", "> "];
    let mut best_pos: Option<usize> = None;
    for marker in markers {
        if let Some(idx) = trimmed.rfind(marker) {
            let end = idx + marker.len();
            best_pos = Some(best_pos.map_or(end, |prev| prev.max(end)));
        }
    }
    if let Some(pos) = best_pos {
        trimmed[pos..].to_string()
    } else if !trimmed.contains(char::is_whitespace)
        && trimmed.ends_with(['$', '#', '%', '❯', '➜', '>'])
    {
        String::new()
    } else {
        trimmed.to_string()
    }
}

/// Builds live autocomplete suggestions for the floating terminal popover.
///
/// When `manual_trigger` (`Ctrl+Space`) is active on an empty prompt or partial substring query,
/// falls back to recent SQLite history entries (and common built-in commands if history is empty).
pub fn build_live_completions(
    cmd_line: &str,
    manual_trigger: bool,
    settings: &AppSettings,
    history_cmds: &[String],
    limit: usize,
) -> Vec<CompletionItem> {
    use onyxsh_terminal::completion::CompletionSource;

    let max_items = limit.max(1);
    let trimmed = cmd_line.trim();
    if trimmed.is_empty() {
        if !manual_trigger {
            return Vec::new();
        }
        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if settings.autocomplete_history_enabled {
            for cmd in history_cmds {
                let c = cmd.trim();
                if !c.is_empty() && seen.insert(c.to_string()) {
                    items.push(CompletionItem {
                        text: c.to_string(),
                        description: tr("Histórico SQLite recente"),
                        completion_type: CompletionType::History,
                        source: CompletionSource::History,
                        score: 3.0,
                        prefix_to_replace: String::new(),
                        suffix_to_insert: c.to_string(),
                    });
                    if items.len() >= max_items {
                        return items;
                    }
                }
            }
        }
        if items.is_empty() && settings.autocomplete_specs_enabled {
            for (cmd, desc) in [
                ("ls -la", "Listar arquivos detalhados"),
                ("cd ", "Alternar diretório"),
                ("git status", "Estado do repositório Git"),
                ("systemctl status ", "Inspecionar serviço systemd"),
                ("docker ps", "Listar contêineres Docker"),
                ("apt update", "Atualizar índice de pacotes"),
            ] {
                items.push(CompletionItem {
                    text: cmd.to_string(),
                    description: desc.to_string(),
                    completion_type: CompletionType::Command,
                    source: CompletionSource::Spec,
                    score: 2.0,
                    prefix_to_replace: String::new(),
                    suffix_to_insert: cmd.to_string(),
                });
                if items.len() >= max_items {
                    break;
                }
            }
        }
        return items;
    }

    let engine = CompletionEngine::new();
    let mut suggestions = engine.get_completions(cmd_line, Some(cmd_line.len()), history_cmds, 8);
    if !settings.autocomplete_specs_enabled {
        suggestions.retain(|it| it.source != CompletionSource::Spec);
    }
    if !settings.autocomplete_history_enabled {
        suggestions.retain(|it| it.source != CompletionSource::History);
    }
    if !settings.autocomplete_snippets_enabled {
        suggestions.retain(|it| it.source != CompletionSource::Snippet);
    }

    // Include SQLite substring matches (`LIKE %query%`) that did not match strict prefix rules
    if settings.autocomplete_history_enabled && suggestions.len() < max_items {
        let q_lower = trimmed.to_lowercase();
        let existing: std::collections::HashSet<String> =
            suggestions.iter().map(|s| s.text.clone()).collect();
        for cmd in history_cmds {
            let c = cmd.trim();
            if c.is_empty() || existing.contains(c) {
                continue;
            }
            if c.to_lowercase().contains(&q_lower) && c != trimmed {
                let starts_prefix = c.to_lowercase().starts_with(&q_lower);
                suggestions.push(CompletionItem {
                    text: c.to_string(),
                    description: tr("Histórico SQLite"),
                    completion_type: CompletionType::History,
                    source: CompletionSource::History,
                    score: if starts_prefix { 2.2 } else { 1.8 },
                    prefix_to_replace: cmd_line.trim_start().to_string(),
                    suffix_to_insert: if starts_prefix && c.is_char_boundary(trimmed.len()) {
                        c[trimmed.len()..].to_string()
                    } else {
                        String::new()
                    },
                });
                if suggestions.len() >= max_items {
                    break;
                }
            }
        }
    }

    suggestions.truncate(max_items);
    suggestions
}

/// Filters [`PaletteItem`]s by a case-insensitive query across category, title, and subtitle.
pub fn filter_palette_items(items: &[PaletteItem], query: &str) -> Vec<PaletteItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|it| {
            it.title.to_lowercase().contains(&q)
                || it.subtitle.to_lowercase().contains(&q)
                || it.category.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
}

/// Queries [`CompletionEngine`] for a command line and returns ranked [`CompletionItem`]s.
pub fn query_terminal_completions(input_line: &str, limit: usize) -> Vec<CompletionItem> {
    let engine = CompletionEngine::new();
    let probe = if input_line.trim().is_empty() {
        "git "
    } else {
        input_line
    };
    engine.get_completions(probe, Some(probe.len()), &[], limit)
}

/// Builds the Production Environment Warning Banner (`ui/widgets/production_banner.py`).
pub fn build_production_banner(
    parent_window: &adw::ApplicationWindow,
    active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>>,
    status_lbl: &gtk4::Label,
) -> (gtk4::Box, gtk4::ToggleButton) {
    let banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    banner.set_margin_start(10);
    banner.set_margin_end(10);
    banner.set_margin_top(4);
    banner.set_margin_bottom(4);
    banner.set_visible(false);

    let icon = gtk4::Image::from_icon_name("dialog-warning-symbolic");
    banner.append(&icon);

    let lbl = gtk4::Label::new(Some(&tr(
        "🛡️ MODO PRODUÇÃO ATIVO — ProductionGuard interceptando comandos destrutivos (rm -rf, mkfs, dd, DROP TABLE, subshells)",
    )));
    lbl.add_css_class("heading");
    lbl.set_halign(gtk4::Align::Start);
    lbl.set_hexpand(true);
    lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    banner.append(&lbl);

    let btn_test_guard = gtk4::Button::with_label(&tr("Testar Interceptação"));
    btn_test_guard.add_css_class("destructive-action");
    banner.append(&btn_test_guard);

    let toggle_prod_btn = gtk4::ToggleButton::builder()
        .icon_name("security-high-symbolic")
        .tooltip_text(&tr(
            "Ativar / Desativar Escudo de Ambiente de Produção (ProductionBanner)",
        ))
        .build();

    {
        let b = banner.clone();
        toggle_prod_btn.connect_toggled(move |btn| {
            let active = btn.is_active();
            GLOBAL_PROD_SHIELD_ACTIVE.with(|c| c.set(active));
            b.set_visible(active);
        });
    }

    {
        let win_ref = parent_window.clone();
        let act_term = Rc::clone(&active_term_cb);
        let st = status_lbl.clone();
        btn_test_guard.connect_clicked(move |_| {
            let guard = ProductionGuard::new();
            let sample_cmd = "sudo rm -rf /var/lib/postgresql/data";
            if let Some(violation) = guard.evaluate_command(sample_cmd) {
                let target = resolve_production_target_hostname(act_term().as_ref());
                show_production_confirm_dialog(&win_ref, &violation, Some(&target), {
                    let at = Rc::clone(&act_term);
                    let st = st.clone();
                    Rc::new(move || {
                        if let Some(term) = at() {
                            term.feed_child(b"# [Confirmado pelo Operador] rm -rf cancelado em modo demo\n");
                        }
                        st.set_text(&tr("Comando de produção avaliado pelo operador."));
                    })
                });
            }
        });
    }

    (banner, toggle_prod_btn)
}

/// Evaluates `critical` requirement & hostname match cleanly for unit testing and dialog validation.
pub fn validate_production_confirmation_input(
    expected_hostname: Option<&str>,
    severity: &str,
    typed_input: &str,
) -> bool {
    if severity.eq_ignore_ascii_case("critical") {
        if let Some(host) = expected_hostname.map(str::trim).filter(|h| !h.is_empty()) {
            return typed_input.trim() == host;
        }
    }
    true
}

/// Opens the modal `ProductionConfirmDialog` (`ui/dialogs/production_confirm_dialog.py`)
/// when [`ProductionGuard`] intercepts a high-risk/destructive command.
pub fn show_production_confirm_dialog(
    parent: &adw::ApplicationWindow,
    violation: &GuardViolation,
    expected_hostname: Option<&str>,
    on_confirm_execute: Rc<dyn Fn()>,
) {
    show_production_confirm_dialog_with_cancel(
        parent,
        violation,
        expected_hostname,
        on_confirm_execute,
        None,
    );
}

/// Variant of [`show_production_confirm_dialog`] accepting an optional cancellation callback
/// (used e.g. to send `Ctrl+C` (`0x03`) to clear the aborted command from the shell prompt).
pub fn show_production_confirm_dialog_with_cancel(
    parent: &adw::ApplicationWindow,
    violation: &GuardViolation,
    expected_hostname: Option<&str>,
    on_confirm_execute: Rc<dyn Fn()>,
    on_cancel_abort: Option<Rc<dyn Fn()>>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("🛑 Alerta Crítico — ProductionGuard"))
        .default_width(520)
        .default_height(340)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.set_margin_top(8);
    content.set_margin_bottom(16);

    let head = gtk4::Label::new(Some(&tr_fmt(
        "⚠️ Comando Destrutivo Interceptado ({cat} / {sev})",
        &[
            ("cat", &violation.category),
            ("sev", &violation.severity),
        ],
    )));
    head.add_css_class("title-4");
    head.set_halign(gtk4::Align::Start);
    content.append(&head);

    let summary_lbl = gtk4::Label::new(Some(&violation.risk_summary));
    summary_lbl.set_wrap(true);
    summary_lbl.set_halign(gtk4::Align::Start);
    content.append(&summary_lbl);

    let cmd_lbl = gtk4::Label::new(Some(&format!("$ {}", violation.command)));
    cmd_lbl.add_css_class("monospace");
    cmd_lbl.set_halign(gtk4::Align::Start);
    cmd_lbl.set_selectable(true);
    content.append(&cmd_lbl);

    let needs_hostname = violation.severity.eq_ignore_ascii_case("critical")
        && expected_hostname.is_some_and(|h| !h.trim().is_empty());

    let btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_row.set_halign(gtk4::Align::End);

    let btn_cancel = gtk4::Button::with_label(&tr("Bloquear e Cancelar (Recomendado)"));
    btn_cancel.add_css_class("suggested-action");

    let btn_proceed = gtk4::Button::with_label(&tr("Assumir Risco e Executar"));
    btn_proceed.add_css_class("destructive-action");

    let decided = Rc::new(Cell::new(false));
    let mut focus_entry_opt: Option<gtk4::Entry> = None;

    if needs_hostname {
        let host = expected_hostname.unwrap().trim().to_string();
        btn_proceed.set_sensitive(false);
        let confirm_hint = gtk4::Label::new(Some(&tr_fmt(
            "Para confirmar esta operação crítica em produção, digite o hostname '{host}':",
            &[("host", &host)],
        )));
        confirm_hint.set_halign(gtk4::Align::Start);
        confirm_hint.add_css_class("caption");
        let host_entry = gtk4::Entry::builder()
            .placeholder_text(&host)
            .build();
        host_entry.add_css_class("monospace");
        let bp = btn_proceed.clone();
        let h_expected = host.clone();
        let sev = violation.severity.clone();
        host_entry.connect_changed(move |e| {
            bp.set_sensitive(validate_production_confirmation_input(
                Some(&h_expected),
                &sev,
                &e.text(),
            ));
        });
        let bp_act = btn_proceed.clone();
        host_entry.connect_activate(move |_| {
            if bp_act.is_sensitive() {
                bp_act.emit_clicked();
            }
        });
        content.append(&confirm_hint);
        content.append(&host_entry);
        focus_entry_opt = Some(host_entry);
    }

    btn_row.append(&btn_cancel);
    btn_row.append(&btn_proceed);
    content.append(&btn_row);

    {
        let d = dlg.clone();
        let dec = Rc::clone(&decided);
        let cancel_cb = on_cancel_abort.clone();
        btn_cancel.connect_clicked(move |_| {
            if !dec.replace(true) {
                if let Some(ref cb) = cancel_cb {
                    cb();
                }
            }
            d.close();
        });
    }
    {
        let d = dlg.clone();
        let cb = on_confirm_execute;
        let dec = Rc::clone(&decided);
        btn_proceed.connect_clicked(move |_| {
            if !dec.replace(true) {
                cb();
            }
            d.close();
        });
    }
    {
        let dec = Rc::clone(&decided);
        let cancel_cb = on_cancel_abort;
        dlg.connect_close_request(move |_| {
            if !dec.replace(true) {
                if let Some(ref cb) = cancel_cb {
                    cb();
                }
            }
            glib::Propagation::Proceed
        });
    }
    {
        let bc = btn_cancel.clone();
        let kc = gtk4::EventControllerKey::new();
        kc.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                bc.emit_clicked();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        dlg.add_controller(kc);
    }

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
    if let Some(entry) = focus_entry_opt {
        entry.grab_focus();
    } else {
        btn_cancel.grab_focus();
    }
}

/// Opens the interactive Autocomplete Assistant (`terminal/completion/popup.py` & `completion.rs`).
pub fn show_autocomplete_assistant_dialog(
    parent: &adw::ApplicationWindow,
    active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>>,
    status_lbl: &gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Autocompletar Inteligente — OnyxSH CompletionEngine"))
        .default_width(540)
        .default_height(420)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    content.set_margin_start(14);
    content.set_margin_end(14);
    content.set_margin_top(8);
    content.set_margin_bottom(14);
    content.set_vexpand(true);

    let hint = gtk4::Label::new(Some(&tr(
        "Digite um comando (ex: 'sudo rpm -', 'sudo dnf install ', 'apt ', 'pacman -', 'flatpak ', 'zypper ', 'snap '):",
    )));
    hint.add_css_class("caption");
    hint.set_halign(gtk4::Align::Start);
    content.append(&hint);

    let entry = gtk4::Entry::builder()
        .text("sudo dnf ")
        .hexpand(true)
        .build();
    entry.add_css_class("monospace");
    content.append(&entry);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    scroll.set_child(Some(&list_box));
    content.append(&scroll);

    let populate = {
        let lb = list_box.clone();
        let e = entry.clone();
        let act_term = Rc::clone(&active_term_cb);
        let st = status_lbl.clone();
        let d_close = dlg.clone();

        Rc::new(move || {
            lb.remove_all();
            let line = e.text().to_string();
            let suggestions = query_terminal_completions(&line, 25);

            for item in suggestions {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                row.set_margin_start(10);
                row.set_margin_end(10);
                row.set_margin_top(6);
                row.set_margin_bottom(6);

                let cmd_lbl = gtk4::Label::new(Some(&item.text));
                cmd_lbl.add_css_class("monospace");
                cmd_lbl.add_css_class("heading");
                cmd_lbl.set_halign(gtk4::Align::Start);
                cmd_lbl.set_width_request(130);

                let desc_lbl = gtk4::Label::new(Some(&item.description));
                desc_lbl.add_css_class("caption");
                desc_lbl.add_css_class("dim-label");
                desc_lbl.set_halign(gtk4::Align::Start);
                desc_lbl.set_hexpand(true);
                desc_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                let full_to_insert = if line.ends_with(char::is_whitespace) {
                    format!("{line}{}", item.text)
                } else if !item.suffix_to_insert.is_empty() {
                    format!("{line}{}", item.suffix_to_insert)
                } else {
                    format!("{line} {}", item.text)
                };

                let btn_ins = gtk4::Button::with_label(&tr("Insert"));
                btn_ins.add_css_class("suggested-action");
                let at = Rc::clone(&act_term);
                let status = st.clone();
                let dc = d_close.clone();
                btn_ins.connect_clicked(move |_| {
                    if let Some(term) = at() {
                        term.feed_child(full_to_insert.as_bytes());
                        status.set_text(&tr_fmt(
                            "Autocompletado: {cmd}",
                            &[("cmd", &full_to_insert)],
                        ));
                    }
                    dc.close();
                });

                row.append(&cmd_lbl);
                row.append(&desc_lbl);
                row.append(&btn_ins);
                lb.append(&row);
            }
        })
    };

    populate();

    {
        let pop = Rc::clone(&populate);
        entry.connect_changed(move |_| pop());
    }

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Detects if there is a hyperlink at coordinates `(x, y)` in `term`.
pub fn detect_url_at_coords(term: &vte4::Terminal, x: f64, y: f64) -> Option<String> {
    if let Some(uri) = term.check_hyperlink_at(x, y) {
        let s = uri.to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    let (matched, _tag) = term.check_match_at(x, y);
    if let Some(m) = matched {
        let s = m.to_string();
        if s.starts_with("http://")
            || s.starts_with("https://")
            || s.starts_with("file://")
            || s.starts_with("ssh://")
            || s.starts_with("ftp://")
        {
            return Some(s);
        }
    }
    None
}

/// Checks if recent terminal lines contain an error indicator.
pub fn check_terminal_last_error(term: &vte4::Terminal) -> Option<i32> {
    let text = crate::global_search_and_shortcuts::extract_vte_text(term);
    let last_lines: Vec<&str> = text.lines().rev().take(6).collect();
    let lower = last_lines.join("\n").to_lowercase();
    let err_keywords = [
        "error",
        "erro",
        "failed",
        "falha",
        "command not found",
        "não encontrada",
        "não reconhecida",
        "no such file",
        "permission denied",
        "permissão negada",
    ];
    if err_keywords.iter().any(|kw| lower.contains(kw)) {
        Some(1)
    } else {
        None
    }
}

/// Builds the native GTK4 `gio::Menu` model for the terminal right-click context menu.
/// Matches `src/onyxsh/ui/menus.py` (`create_terminal_menu`) and `CORRIGIR/menucontext3.jpg`.
pub fn create_terminal_context_menu_model(
    has_selection: bool,
    url_at_click: Option<&str>,
    error_code: Option<i32>,
) -> gio::Menu {
    let menu = gio::Menu::new();

    // 0. Contextual URL section (when right-clicking over a hyperlink)
    if let Some(_url) = url_at_click {
        let url_section = gio::Menu::new();
        url_section.append(Some(&tr("Open Link")), Some("win.open-url"));
        url_section.append(Some(&tr("Copy Link")), Some("win.copy-url"));
        menu.append_section(None, &url_section);
    }

    // 1. Standard editing, export & session clear
    let standard_section = gio::Menu::new();
    standard_section.append(Some(&tr("Copiar")), Some("win.copy"));
    standard_section.append(Some(&tr("Copiar saída do último comando")), Some("win.copy-last-output"));
    standard_section.append(Some(&tr("Paste")), Some("win.paste"));
    standard_section.append(Some(&tr("Selecionar Tudo")), Some("win.select-all"));
    standard_section.append(
        Some(&tr("Export Terminal Output...")),
        Some("win.export-terminal-buffer"),
    );
    standard_section.append(
        Some(&tr("Export Session as Runbook...")),
        Some("win.export-terminal-runbook"),
    );
    standard_section.append(Some(&tr("Clear Session")), Some("win.clear-session"));
    menu.append_section(None, &standard_section);

    // 2. Prompt navigation
    let nav_section = gio::Menu::new();
    nav_section.append(
        Some(&tr("Pular para o Prompt Anterior")),
        Some("win.jump-previous-prompt"),
    );
    nav_section.append(
        Some(&tr("Pular para o Próximo Prompt")),
        Some("win.jump-next-prompt"),
    );
    menu.append_section(None, &nav_section);

    // 3. AI / OpenCode Assistant
    let ai_section = gio::Menu::new();
    if has_selection {
        ai_section.append(Some(&tr("Perguntar à IA")), Some("win.ask-ai-selection"));
        ai_section.append(
            Some(&tr("Refatorar seleção com OpenCode")),
            Some("win.opencode-selection"),
        );
    } else {
        let label = if let Some(code) = error_code {
            if code != 1 && code != 0 {
                tr_fmt(
                    "Resolver erro (código {code}) com OpenCode",
                    &[("code", &code.to_string())],
                )
            } else {
                tr("Resolver erro com OpenCode")
            }
        } else {
            tr("Resolver erro com OpenCode")
        };
        ai_section.append(Some(&label), Some("win.resolve-error-opencode"));
    }
    menu.append_section(None, &ai_section);

    // 4. Splitting & Panes
    let split_section = gio::Menu::new();
    split_section.append(Some(&tr("Split Left/Right")), Some("win.split-horizontal"));
    split_section.append(Some(&tr("Split Top/Bottom")), Some("win.split-vertical"));
    split_section.append(Some(&tr("Mover painel para nova aba")), Some("win.move-pane-to-tab"));
    split_section.append(Some(&tr("Close Pane")), Some("win.close-pane"));
    menu.append_section(None, &split_section);

    menu
}

/// Attaches native right-click `PopoverMenu` (`CORRIGIR/menucontext3.jpg`), floating live autocomplete
/// popup (`CompletionEngine`), floating `SelectionActionPopover`, and `ProductionGuard` to a `vte4::Terminal`.
pub fn attach_vte_ux_controllers(
    term: &vte4::Terminal,
    clicked_url_rc: Rc<RefCell<Option<String>>>,
    parent_window: &adw::ApplicationWindow,
    ai_widget: &gtk4::Box,
    btn_ai_toggle: &gtk4::ToggleButton,
    submit_ai_prompt_cb: Rc<dyn Fn(&str)>,
    status_lbl: &gtk4::Label,
    settings_rc: &Rc<RefCell<AppSettings>>,
) {
    // 0. Floating Selection Action Toolbar (`src/onyxsh/ui/widgets/selection_popover.py`)
    let sel_popover = gtk4::Popover::new();
    sel_popover.set_parent(term);
    sel_popover.set_autohide(true);
    sel_popover.set_has_arrow(true);
    sel_popover.set_position(gtk4::PositionType::Top);

    let sel_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    sel_hbox.set_margin_start(4);
    sel_hbox.set_margin_end(4);
    sel_hbox.set_margin_top(4);
    sel_hbox.set_margin_bottom(4);

    let btn_sel_copy = gtk4::Button::builder()
        .icon_name("edit-copy-symbolic")
        .label(&tr("Copiar"))
        .css_classes(["flat"])
        .build();
    let btn_sel_search = gtk4::Button::builder()
        .icon_name("system-search-symbolic")
        .label(&tr("Search"))
        .css_classes(["flat"])
        .build();
    let btn_sel_ai = gtk4::Button::builder()
        .icon_name("avatar-default-symbolic")
        .label(&tr("Perguntar à IA"))
        .css_classes(["flat"])
        .build();
    let btn_sel_explain = gtk4::Button::builder()
        .icon_name("dialog-question-symbolic")
        .label(&tr("Explicar"))
        .css_classes(["flat"])
        .build();

    sel_hbox.append(&btn_sel_copy);
    sel_hbox.append(&btn_sel_search);
    sel_hbox.append(&btn_sel_ai);
    sel_hbox.append(&btn_sel_explain);
    sel_popover.set_child(Some(&sel_hbox));

    {
        let t = term.clone();
        let pop = sel_popover.clone();
        let st = status_lbl.clone();
        btn_sel_copy.connect_clicked(move |_| {
            t.copy_clipboard_format(vte4::Format::Text);
            pop.popdown();
            st.set_text(&tr("Seleção copiada."));
        });
    }
    {
        let win_s = parent_window.clone();
        let pop = sel_popover.clone();
        btn_sel_search.connect_clicked(move |_| {
            pop.popdown();
            let _ = gtk4::prelude::WidgetExt::activate_action(&win_s, "win.toggle-search", None);
        });
    }
    {
        let win_s = parent_window.clone();
        let pop = sel_popover.clone();
        btn_sel_ai.connect_clicked(move |_| {
            pop.popdown();
            let _ = gtk4::prelude::WidgetExt::activate_action(&win_s, "win.ask-ai-selection", None);
        });
    }
    {
        let t = term.clone();
        let pop = sel_popover.clone();
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai_toggle.clone();
        let sub_ai = Rc::clone(&submit_ai_prompt_cb);
        let st = status_lbl.clone();
        btn_sel_explain.connect_clicked(move |_| {
            pop.popdown();
            if let Some(selected) = t.text_selected(vte4::Format::Text) {
                let prompt = tr_fmt(
                    "Explique de forma objetiva o seguinte comando ou erro do terminal:\n\n<untrusted>\n{sel}\n</untrusted>",
                    &[("sel", selected.as_str().trim())],
                );
                ai_w.set_visible(true);
                ai_b.set_active(true);
                sub_ai(&prompt);
                st.set_text(&tr("Seleção enviada para análise no Assistente de IA."));
            }
        });
    }

    let sel_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    {
        let st_rc = Rc::clone(settings_rc);
        let pop = sel_popover.clone();
        let timer_rc = Rc::clone(&sel_timer);
        term.connect_selection_changed(move |t| {
            if let Some(id) = timer_rc.borrow_mut().take() {
                id.remove();
            }
            if !st_rc.borrow().show_selection_toolbar || !t.has_selection() {
                pop.popdown();
                return;
            }
            let t_ref = t.clone();
            let p_ref = pop.clone();
            let trc = Rc::clone(&timer_rc);
            let new_id = glib::timeout_add_local_once(Duration::from_millis(160), move || {
                trc.borrow_mut().take();
                if t_ref.has_selection() && t_ref.is_realized() && t_ref.is_mapped() {
                    let w = t_ref.width().max(200);
                    let h = t_ref.height().max(100);
                    let (col, row) = t_ref.cursor_position();
                    let char_w = (t_ref.char_width() as i32).max(8);
                    let char_h = (t_ref.char_height() as i32).max(16);
                    let vscroll = t_ref.vadjustment().map(|a| a.value()).unwrap_or(0.0);
                    let vis_row = ((row as f64) - vscroll).max(0.0) as i32;
                    let rx = ((col as i32) * char_w).clamp(16, w - 16);
                    let ry = (vis_row * char_h).clamp(16, h - 16);
                    p_ref.set_pointing_to(Some(&gdk::Rectangle::new(rx, ry, char_w, char_h)));
                    p_ref.popup();
                }
            });
            *timer_rc.borrow_mut() = Some(new_id);
        });
    }
    // 1. Live Floating Autocomplete Popover (`src/onyxsh/terminal/completion/popup.py` & `manager.py`)
    let comp_popover = gtk4::Popover::new();
    comp_popover.set_parent(term);
    comp_popover.set_autohide(false);
    comp_popover.set_focusable(false);
    comp_popover.set_can_focus(false);
    comp_popover.set_has_arrow(false);
    comp_popover.set_position(gtk4::PositionType::Bottom);

    let comp_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    comp_vbox.set_margin_start(4);
    comp_vbox.set_margin_end(4);
    comp_vbox.set_margin_top(4);
    comp_vbox.set_margin_bottom(4);
    comp_vbox.set_size_request(340, -1);
    comp_vbox.set_can_focus(false);

    let comp_list = gtk4::ListBox::new();
    comp_list.add_css_class("boxed-list");
    comp_list.set_selection_mode(gtk4::SelectionMode::Single);
    comp_list.set_can_focus(false);
    comp_vbox.append(&comp_list);

    let comp_hint = gtk4::Label::new(Some(&tr("Tab ou → aplicar • ↑/↓ navegar • Esc fechar")));
    comp_hint.add_css_class("dim-label");
    comp_hint.add_css_class("caption");
    comp_hint.set_halign(gtk4::Align::Start);
    comp_hint.set_margin_start(6);
    comp_vbox.append(&comp_hint);

    comp_popover.set_child(Some(&comp_vbox));

    // 2. Right-click Native VTE Context Menu (`CORRIGIR/menucontext3.jpg` & `src/onyxsh/terminal/manager.py`)
    // In GTK4, VTE natively manages and renders the context menu popover using its context_menu_model.
    // This provides automatic grab dismissal when clicking outside or switching windows,
    // correct accelerator display, and eliminates any duplicate or orphaned popovers.
    let initial_menu = create_terminal_context_menu_model(false, None, None);
    term.set_context_menu_model(Some(&initial_menu));

    let click_ctrl = gtk4::GestureClick::new();
    click_ctrl.set_button(3); // Right mouse button
    click_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let t = term.clone();
        let c_url = Rc::clone(&clicked_url_rc);
        let comp_p = comp_popover.clone();
        click_ctrl.connect_pressed(move |_gesture, _, x, y| {
            t.grab_focus();
            if comp_p.is_visible() {
                comp_p.popdown();
            }

            let url = detect_url_at_coords(&t, x, y);
            *c_url.borrow_mut() = url.clone();

            let has_sel = t.has_selection();
            let err_code = check_terminal_last_error(&t);

            let menu = create_terminal_context_menu_model(has_sel, url.as_deref(), err_code);
            t.set_context_menu_model(Some(&menu));
        });
    }
    term.add_controller(click_ctrl);

    let comp_items: Rc<RefCell<Vec<CompletionItem>>> = Rc::new(RefCell::new(Vec::new()));
    let comp_selected_idx: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let comp_rows: Rc<RefCell<Vec<gtk4::ListBoxRow>>> = Rc::new(RefCell::new(Vec::new()));

    let accept_completion: Rc<dyn Fn(usize)> = {
        let t = term.clone();
        let pop = comp_popover.clone();
        let items_rc = Rc::clone(&comp_items);
        let st = status_lbl.clone();

        Rc::new(move |idx: usize| {
            let maybe_item = items_rc.borrow().get(idx).cloned();
            pop.popdown();
            if let Some(item) = maybe_item {
                let mut feed = if !item.suffix_to_insert.is_empty() {
                    item.suffix_to_insert.clone()
                } else if !item.prefix_to_replace.is_empty() {
                    let backspaces = "\x08".repeat(item.prefix_to_replace.chars().count());
                    format!("{backspaces}{}", item.text)
                } else {
                    item.text.clone()
                };
                if !matches!(item.completion_type, CompletionType::History) && !feed.ends_with(' ') {
                    feed.push(' ');
                }
                t.feed_child(feed.as_bytes());
                st.set_text(&tr_fmt(
                    "Autocompletar aplicado: {cmd}",
                    &[("cmd", &item.text)],
                ));
            }
        })
    };

    {
        let acc = Rc::clone(&accept_completion);
        comp_list.connect_row_activated(move |_, row| {
            let idx = row.index();
            if idx >= 0 {
                acc(idx as usize);
            }
        });
    }

    let comp_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let key_ctrl = gtk4::EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let t = term.clone();
        let pop = comp_popover.clone();
        let lb = comp_list;
        let items_rc = Rc::clone(&comp_items);
        let sel_rc = Rc::clone(&comp_selected_idx);
        let rows_rc = Rc::clone(&comp_rows);
        let acc = Rc::clone(&accept_completion);
        let st_rc = Rc::clone(settings_rc);
        let win_ref = parent_window.clone();
        let sel_pop_hide = sel_popover.clone();
        let timer_rc = Rc::clone(&comp_timer);
        let st_lbl_key = status_lbl.clone();

        key_ctrl.connect_key_pressed(move |_, keyval, _, state| {
            let has_mod = state.intersects(
                gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK,
            );
            if sel_pop_hide.is_visible() {
                sel_pop_hide.popdown();
            }

            // Handle navigation and acceptance when floating CompletionPopup is visible
            if pop.is_visible() && !items_rc.borrow().is_empty() && !has_mod {
                let len = items_rc.borrow().len();
                if keyval == gdk::Key::Down || keyval == gdk::Key::KP_Down {
                    let next = (*sel_rc.borrow() + 1) % len;
                    *sel_rc.borrow_mut() = next;
                    if let Some(r) = rows_rc.borrow().get(next) {
                        lb.select_row(Some(r));
                    }
                    return glib::Propagation::Stop;
                }
                if keyval == gdk::Key::Up || keyval == gdk::Key::KP_Up {
                    let prev = if *sel_rc.borrow() == 0 {
                        len - 1
                    } else {
                        *sel_rc.borrow() - 1
                    };
                    *sel_rc.borrow_mut() = prev;
                    if let Some(r) = rows_rc.borrow().get(prev) {
                        lb.select_row(Some(r));
                    }
                    return glib::Propagation::Stop;
                }
                if keyval == gdk::Key::Tab
                    || keyval == gdk::Key::Right
                    || keyval == gdk::Key::KP_Right
                {
                    let idx = *sel_rc.borrow();
                    acc(idx);
                    return glib::Propagation::Stop;
                }
                if keyval == gdk::Key::Escape {
                    if let Some(id) = timer_rc.borrow_mut().take() {
                        id.remove();
                    }
                    pop.popdown();
                    return glib::Propagation::Stop;
                }
                if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
                    pop.popdown();
                }
            }

            // ProductionGuard interception on Enter (`Return` / `KP_Enter`)
            if !has_mod && (keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter) {
                if let Some(id) = timer_rc.borrow_mut().take() {
                    id.remove();
                }
                if is_terminal_production_effective(&t) && !is_terminal_alt_screen(&t) {
                    let (_col, cursor_row) = t.cursor_position();
                    let col_count = t.column_count().max(1);
                    let (text_opt, _) = t.text_range_format(
                        vte4::Format::Text,
                        cursor_row,
                        0,
                        cursor_row,
                        col_count,
                    );
                    let raw_line = text_opt.map(|g| g.to_string()).unwrap_or_default();
                    let cmd_line = extract_prompt_command_line(&raw_line);
                    if !cmd_line.trim().is_empty() {
                        if let Some(violation) = ProductionGuard::new().evaluate_command(&cmd_line) {
                            let expected_host = resolve_production_target_hostname(Some(&t));
                            let t_exec = t.clone();
                            let t_cancel = t.clone();
                            show_production_confirm_dialog_with_cancel(
                                &win_ref,
                                &violation,
                                Some(expected_host.as_str()),
                                Rc::new(move || {
                                    t_exec.feed_child(b"\r");
                                    t_exec.grab_focus();
                                }),
                                Some(Rc::new(move || {
                                    // Clear typed destructive command from shell prompt
                                    t_cancel.feed_child(b"\x03");
                                    t_cancel.grab_focus();
                                })),
                            );
                            return glib::Propagation::Stop;
                        }
                    }
                }
                return glib::Propagation::Proceed;
            }

            // Trigger debounced autocomplete lookup on printable keys, Backspace, or Ctrl+Space
            let is_ctrl_space = state.contains(gdk::ModifierType::CONTROL_MASK)
                && !state.intersects(gdk::ModifierType::ALT_MASK | gdk::ModifierType::SUPER_MASK)
                && (keyval == gdk::Key::space
                    || keyval == gdk::Key::KP_Space
                    || keyval.to_unicode() == Some(' '));
            let is_printable = keyval.to_unicode().is_some_and(|c| !c.is_control());
            if (is_ctrl_space || (!has_mod && (is_printable || keyval == gdk::Key::BackSpace)))
                && st_rc.borrow().autocomplete_enabled
            {
                if let Some(old_id) = timer_rc.borrow_mut().take() {
                    old_id.remove();
                }
                if !is_autocomplete_allowed_for_terminal(&t) {
                    pop.popdown();
                    return if is_ctrl_space {
                        glib::Propagation::Stop
                    } else {
                        glib::Propagation::Proceed
                    };
                }

                let t_ref = t.clone();
                let pop_ref = pop.clone();
                let lb_ref = lb.clone();
                let items_ref = Rc::clone(&items_rc);
                let sel_ref = Rc::clone(&sel_rc);
                let rows_ref = Rc::clone(&rows_rc);
                let st_lookup = Rc::clone(&st_rc);
                let timer_done = Rc::clone(&timer_rc);
                let status_done = st_lbl_key.clone();
                let manual_trigger = is_ctrl_space;
                let delay_ms = if is_ctrl_space { 10 } else { 120 };

                let src_id = glib::timeout_add_local_once(Duration::from_millis(delay_ms), move || {
                    *timer_done.borrow_mut() = None;
                    if !t_ref.is_realized()
                        || !t_ref.is_mapped()
                        || !is_autocomplete_allowed_for_terminal(&t_ref)
                        || !st_lookup.borrow().autocomplete_enabled
                    {
                        pop_ref.popdown();
                        return;
                    }
                    let (cursor_col, cursor_row) = t_ref.cursor_position();
                    let col_count = t_ref.column_count();
                    let (text_opt, _) = t_ref.text_range_format(
                        vte4::Format::Text,
                        cursor_row,
                        0,
                        cursor_row,
                        col_count,
                    );
                    let raw_line = text_opt.map(|g| g.to_string()).unwrap_or_default();
                    let cmd_line = extract_prompt_command_line(&raw_line);

                    if !manual_trigger && cmd_line.trim().is_empty() {
                        pop_ref.popdown();
                        return;
                    }

                    let st_snap = st_lookup.borrow().clone();
                    let mut history_cmds = Vec::new();
                    if st_snap.autocomplete_history_enabled {
                        if let Ok(store) = CommandHistoryStore::open_default() {
                            if let Ok(items) =
                                store.search_history(cmd_line.trim(), None, None, false, 8, 0)
                            {
                                for h in items {
                                    history_cmds.push(h.command);
                                }
                            }
                        }
                    }

                    let suggestions = build_live_completions(
                        &cmd_line,
                        manual_trigger,
                        &st_snap,
                        &history_cmds,
                        6,
                    );

                    lb_ref.select_row(None::<&gtk4::ListBoxRow>);
                    lb_ref.remove_all();
                    rows_ref.borrow_mut().clear();

                    if suggestions.is_empty() {
                        items_ref.borrow_mut().clear();
                        pop_ref.popdown();
                        if manual_trigger {
                            status_done.set_text(&tr("Nenhuma sugestão disponível para o texto atual."));
                        }
                        return;
                    }

                    for item in &suggestions {
                        let row = gtk4::ListBoxRow::new();
                        row.set_can_focus(false);
                        row.set_focusable(false);

                        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                        hbox.set_margin_start(8);
                        hbox.set_margin_end(8);
                        hbox.set_margin_top(5);
                        hbox.set_margin_bottom(5);

                        let icon = gtk4::Image::from_icon_name(item.completion_type.icon_name());
                        icon.add_css_class("dim-label");
                        hbox.append(&icon);

                        let vbox_txt = gtk4::Box::new(gtk4::Orientation::Vertical, 1);
                        vbox_txt.set_hexpand(true);

                        let title = gtk4::Label::new(Some(&item.text));
                        title.add_css_class("monospace");
                        title.add_css_class("heading");
                        title.set_halign(gtk4::Align::Start);
                        vbox_txt.append(&title);

                        if !item.description.is_empty() {
                            let desc = gtk4::Label::new(Some(&item.description));
                            desc.add_css_class("caption");
                            desc.add_css_class("dim-label");
                            desc.set_halign(gtk4::Align::Start);
                            desc.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                            vbox_txt.append(&desc);
                        }
                        hbox.append(&vbox_txt);

                        let badge = gtk4::Label::new(Some(item.completion_type.badge_label()));
                        badge.add_css_class("caption");
                        badge.add_css_class("dim-label");
                        hbox.append(&badge);

                        row.set_child(Some(&hbox));
                        lb_ref.append(&row);
                        rows_ref.borrow_mut().push(row);
                    }

                    *items_ref.borrow_mut() = suggestions;
                    *sel_ref.borrow_mut() = 0;
                    if let Some(first_row) = rows_ref.borrow().first() {
                        lb_ref.select_row(Some(first_row));
                    }

                    let term_w = t_ref.width().max(1);
                    let term_h = t_ref.height().max(1);
                    let char_w = (t_ref.char_width() as i32).max(8);
                    let char_h = (t_ref.char_height() as i32).max(16);

                    let scroll_offset = t_ref
                        .vadjustment()
                        .map(|adj| adj.value())
                        .unwrap_or(0.0);
                    let visible_row = ((cursor_row as f64) - scroll_offset).max(0.0) as i32;

                    let cur_x = ((cursor_col as i32) * char_w).clamp(0, (term_w - char_w).max(0));
                    let cur_y = (visible_row * char_h).clamp(0, (term_h - char_h).max(0));

                    pop_ref.set_pointing_to(Some(&gdk::Rectangle::new(
                        cur_x, cur_y, char_w, char_h,
                    )));
                    pop_ref.popup();
                });
                *timer_rc.borrow_mut() = Some(src_id);
                if is_ctrl_space {
                    return glib::Propagation::Stop;
                }
            }

            glib::Propagation::Proceed
        });
    }
    term.add_controller(key_ctrl);

    let sel_pop_cleanup = sel_popover;
    let comp_pop_cleanup = comp_popover;
    let timer_cleanup = comp_timer;
    term.connect_destroy(move |_| {
        if let Some(id) = timer_cleanup.borrow_mut().take() {
            id.remove();
        }
        sel_pop_cleanup.popdown();
        sel_pop_cleanup.unparent();
        comp_pop_cleanup.popdown();
        comp_pop_cleanup.unparent();
    });
}

/// Opens the Unified Command Palette (`ui/dialogs/command_palette.py` — `Ctrl+Shift+P`).
pub fn show_command_palette_dialog(
    parent: &adw::ApplicationWindow,
    cmd_buttons: &[CommandButton],
    on_select_action: Rc<dyn Fn(&str)>,
    on_spawn_session: Rc<dyn Fn(&SessionItem)>,
    on_run_command: Rc<dyn Fn(&str)>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Command Palette (Ctrl+Shift+P) — Busca Unificada"))
        .default_width(580)
        .default_height(420)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    content.set_margin_start(14);
    content.set_margin_end(14);
    content.set_margin_top(6);
    content.set_margin_bottom(14);
    content.set_vexpand(true);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text(&tr(
            "Digite uma ação, sessão SSH ou comando parametrizado...",
        ))
        .hexpand(true)
        .build();
    content.append(&search_entry);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    scroll.set_child(Some(&list_box));
    content.append(&scroll);

    // Build unified catalog of PaletteItems (Port of `src/onyxsh/ui/dialogs/command_palette_dialog.py`)
    let mut catalog = vec![
        PaletteItem {
            category: tr("Busca"),
            title: tr("Busca Global Multi-Abas / Sessões / Histórico"),
            subtitle: tr(
                "Pesquisa em todas as abas abertas, sessões SSH, histórico e snippets (Ctrl+Shift+G)",
            ),
            action_key: "act:global-search".to_string(),
        },
        PaletteItem {
            category: tr("Busca"),
            title: tr("Buscar no Terminal Atual"),
            subtitle: tr("Abre a barra de busca Regex/Literal na aba ativa (Ctrl+Shift+F)"),
            action_key: "act:toggle-search".to_string(),
        },
        PaletteItem {
            category: tr("Terminal"),
            title: tr("Histórico Enriquecido de Comandos"),
            subtitle: tr("Busca fuzzy por diretório, host e favoritos (Ctrl+H)"),
            action_key: "act:show-command-history".to_string(),
        },
        PaletteItem {
            category: tr("Abas e Janelas"),
            title: tr("Nova Aba de Terminal"),
            subtitle: tr("Abre um novo terminal local (Ctrl+Shift+T)"),
            action_key: "act:new-tab".to_string(),
        },
        PaletteItem {
            category: tr("Abas e Janelas"),
            title: tr("Fechar Aba Atual"),
            subtitle: tr("Encerra a aba ativa (Ctrl+Shift+W)"),
            action_key: "act:close-tab".to_string(),
        },
        PaletteItem {
            category: tr("Abas e Janelas"),
            title: tr("Próxima Aba"),
            subtitle: tr("Avança para a próxima aba (Alt+Page Down)"),
            action_key: "act:next-tab".to_string(),
        },
        PaletteItem {
            category: tr("Abas e Janelas"),
            title: tr("Aba Anterior"),
            subtitle: tr("Volta para a aba anterior (Alt+Page Up)"),
            action_key: "act:previous-tab".to_string(),
        },
        PaletteItem {
            category: tr("Divisão de Telas"),
            title: tr("Dividir Painel Horizontalmente"),
            subtitle: tr("Divide a aba atual lado a lado (Ctrl+Shift+()"),
            action_key: "act:split".to_string(),
        },
        PaletteItem {
            category: tr("Divisão de Telas"),
            title: tr("Dividir Painel Verticalmente"),
            subtitle: tr("Divide a aba atual acima/abaixo (Ctrl+Shift+))"),
            action_key: "act:split-vertical".to_string(),
        },
        PaletteItem {
            category: tr("Terminal"),
            title: tr("Exportar Saída do Terminal (.txt, .log, .md, .html, .cast)"),
            subtitle: tr("Exporta todo o buffer ou a seleção ativa do terminal"),
            action_key: "act:export-terminal-buffer".to_string(),
        },
        PaletteItem {
            category: tr("Terminal"),
            title: tr("Exportar Sessão como Caderno de Bordo (Runbook)"),
            subtitle: tr("Relatório operacional estruturado com etapas, anotações, objetivo e conclusão (.md, .html, .log)"),
            action_key: "act:export-terminal-runbook".to_string(),
        },
        PaletteItem {
            category: tr("Terminal"),
            title: tr("Limpar Sessão do Terminal"),
            subtitle: tr("Limpa a tela e o scrollback da sessão ativa (Ctrl+Shift+L)"),
            action_key: "act:clear-session".to_string(),
        },
        PaletteItem {
            category: tr("Terminal"),
            title: tr("Modo Transmissão (Broadcast p/ Todas as Abas)"),
            subtitle: tr("Envia comandos simultaneamente para múltiplos terminais (Ctrl+Shift+B)"),
            action_key: "act:toggle-broadcast".to_string(),
        },
        PaletteItem {
            category: tr("Terminal"),
            title: tr("Aumentar Zoom / Tamanho da Fonte"),
            subtitle: tr("Incrementa o tamanho da fonte do terminal (Ctrl++)"),
            action_key: "act:zoom-in".to_string(),
        },
        PaletteItem {
            category: tr("Terminal"),
            title: tr("Diminuir Zoom / Tamanho da Fonte"),
            subtitle: tr("Reduz o tamanho da fonte do terminal (Ctrl+-)"),
            action_key: "act:zoom-out".to_string(),
        },
        PaletteItem {
            category: tr("Terminal"),
            title: tr("Resetar Zoom"),
            subtitle: tr("Restaura a fonte padrão Monospace 11 (Ctrl+0)"),
            action_key: "act:zoom-reset".to_string(),
        },
        PaletteItem {
            category: tr("Sessões & SSH"),
            title: tr("Alternar Painel Lateral de Sessões SSH"),
            subtitle: tr("Mostra ou oculta a árvore de sessões SSH (Ctrl+Shift+H)"),
            action_key: "act:toggle-sidebar".to_string(),
        },
        PaletteItem {
            category: tr("Sessões & SSH"),
            title: tr("Alternar Gerenciador de Arquivos Dual-Pane"),
            subtitle: tr("Lista, Grade de Miniaturas, Árvore & Quick Look (Ctrl+Shift+E)"),
            action_key: "act:toggle-fm".to_string(),
        },
        PaletteItem {
            category: tr("Ferramentas"),
            title: tr("Gerenciador de Comandos e Snippets"),
            subtitle: tr("Cria e edita botões de comando parametrizados (Ctrl+Shift+M)"),
            action_key: "act:show-command-manager".to_string(),
        },
        PaletteItem {
            category: tr("Assistente de IA"),
            title: tr("Alternar Assistente de IA Zero-Trust"),
            subtitle: tr("SmartRouter, Planos de Ação e Rollback Atômico (Ctrl+Shift+I)"),
            action_key: "act:toggle-ai".to_string(),
        },
        PaletteItem {
            category: tr("Ferramentas"),
            title: tr("Abrir Monitor de Recursos (Sparklines Cairo)"),
            subtitle: tr("Gráficos em tempo real de CPU, RAM, Disco I/O e Rede/GPU"),
            action_key: "act:resource-monitor".to_string(),
        },
        PaletteItem {
            category: tr("Ferramentas"),
            title: tr("Autocompletar Inteligente (CompletionEngine)"),
            subtitle: tr("Sugestões para rpm, dnf, apt, pacman, flatpak, zypper, snap, docker, git"),
            action_key: "act:autocomplete".to_string(),
        },
        PaletteItem {
            category: tr("Assistente de IA"),
            title: tr("Git Assistant — Sugerir Conventional Commit"),
            subtitle: tr("Inspeciona repositório Git atual e sugere mensagem de commit"),
            action_key: "act:git-assistant".to_string(),
        },
        PaletteItem {
            category: tr("Assistente de IA"),
            title: tr("Registro de Auditoria do Agente"),
            subtitle: tr("Trilha contínua de auditoria das decisões, comandos e rollback atômico"),
            action_key: "act:show-audit-log".to_string(),
        },
        PaletteItem {
            category: tr("Assistente de IA"),
            title: tr("Configuração de Escopo e Segurança do Agente"),
            subtitle: tr("Diretórios permitidos, isolamento PathGuard e políticas default-deny"),
            action_key: "act:show-scope-dialog".to_string(),
        },
        PaletteItem {
            category: tr("Ferramentas"),
            title: tr("Diagnóstico Seguro do Sistema (Telemetria & Suporte)"),
            subtitle: tr("Relatório sanitizado de SO, RAM, GPU/VRAM e ambiente Flatpak"),
            action_key: "act:system-diagnostics".to_string(),
        },
        PaletteItem {
            category: tr("Configurações"),
            title: tr("Atalhos de Teclado (34 Ações Configuráveis)"),
            subtitle: tr("Visualiza, edita e restaura todos os atalhos de teclado do OnyxSH"),
            action_key: "act:shortcuts".to_string(),
        },
        PaletteItem {
            category: tr("Configurações"),
            title: tr("Preferências do OnyxSH"),
            subtitle: tr("Fonte, paleta de cores, transparência e IA (F2)"),
            action_key: "act:preferences".to_string(),
        },
        PaletteItem {
            category: tr("Configurações"),
            title: tr("Cores e Destaques do Terminal"),
            subtitle: tr("30 esquemas de cores, destaque de saída e regras por comando"),
            action_key: "act:highlight-settings".to_string(),
        },
        PaletteItem {
            category: tr("Configurações"),
            title: tr("Criar Backup de Configurações (Bundle SHA-256)"),
            subtitle: tr("Exporta settings.json, sessions.json e command_buttons.json com hash SHA-256"),
            action_key: "act:backup-config".to_string(),
        },
    ];

    let sessions_path = platform::config_dir().join("sessions.json");
    let loaded_sessions = SessionsStorageData::load_from_path(&sessions_path)
        .map(|s| s.sessions)
        .unwrap_or_default();

    for (idx, sess) in loaded_sessions.iter().enumerate() {
        catalog.push(PaletteItem {
            category: tr("Session"),
            title: tr_fmt("Conectar: {name}", &[("name", &sess.name)]),
            subtitle: format!("{} ({})", sess.connection_string(), sess.session_type),
            action_key: format!("sess:{idx}"),
        });
    }

    for cmd in cmd_buttons {
        catalog.push(PaletteItem {
            category: tr_fmt("Comando ({cat})", &[("cat", &tr(&cmd.category))]),
            title: tr(&cmd.name),
            subtitle: format!("$ {}", cmd.command_template),
            action_key: format!("cmd:{}", cmd.command_template),
        });
    }

    let catalog_rc = Rc::new(catalog);
    let sessions_rc = Rc::new(loaded_sessions);

    let populate = {
        let lb = list_box.clone();
        let se = search_entry.clone();
        let cat = Rc::clone(&catalog_rc);
        let sess_list = Rc::clone(&sessions_rc);
        let act_cb = Rc::clone(&on_select_action);
        let sess_cb = Rc::clone(&on_spawn_session);
        let cmd_cb = Rc::clone(&on_run_command);
        let d_close = dlg.clone();

        Rc::new(move || {
            lb.remove_all();
            let filtered = filter_palette_items(&cat, &se.text());

            for item in filtered {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                row.set_margin_start(10);
                row.set_margin_end(10);
                row.set_margin_top(6);
                row.set_margin_bottom(6);

                let text_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                text_vbox.set_hexpand(true);

                let t_lbl = gtk4::Label::new(Some(&format!("[{}] {}", item.category, item.title)));
                t_lbl.add_css_class("heading");
                t_lbl.set_halign(gtk4::Align::Start);

                let s_lbl = gtk4::Label::new(Some(&item.subtitle));
                s_lbl.add_css_class("caption");
                s_lbl.add_css_class("dim-label");
                s_lbl.set_halign(gtk4::Align::Start);

                text_vbox.append(&t_lbl);
                text_vbox.append(&s_lbl);
                row.append(&text_vbox);

                let run_btn = gtk4::Button::with_label(&tr("Execute"));
                run_btn.add_css_class("flat");
                let key = item.action_key.clone();
                let a_fn = Rc::clone(&act_cb);
                let s_fn = Rc::clone(&sess_cb);
                let c_fn = Rc::clone(&cmd_cb);
                let sl = Rc::clone(&sess_list);
                let dc = d_close.clone();

                run_btn.connect_clicked(move |_| {
                    if let Some(act) = key.strip_prefix("act:") {
                        a_fn(act);
                    } else if let Some(idx_str) = key.strip_prefix("sess:") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if let Some(s) = sl.get(idx) {
                                s_fn(s);
                            }
                        }
                    } else if let Some(cmd_tpl) = key.strip_prefix("cmd:") {
                        c_fn(cmd_tpl);
                    }
                    dc.close();
                });

                row.append(&run_btn);
                lb.append(&row);
            }
        })
    };

    populate();

    {
        let pop = Rc::clone(&populate);
        search_entry.connect_search_changed(move |_| pop());
    }
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        let d_close = dlg.clone();
        let lb = list_box.clone();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                d_close.close();
                return glib::Propagation::Stop;
            }
            if keyval == gdk::Key::Down || keyval == gdk::Key::Up {
                let curr = lb.selected_row().map(|r| r.index()).unwrap_or(0);
                let next = if keyval == gdk::Key::Down {
                    curr + 1
                } else {
                    (curr - 1).max(0)
                };
                if let Some(r) = lb.row_at_index(next) {
                    lb.select_row(Some(&r));
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        dlg.add_controller(key_ctrl);
    }

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
    search_entry.grab_focus();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_filtering_and_terminal_completions() {
        let items = vec![
            PaletteItem {
                category: "Ação".to_string(),
                title: "Abrir Monitor de Recursos".to_string(),
                subtitle: "Sparklines Cairo".to_string(),
                action_key: "act:resource-monitor".to_string(),
            },
            PaletteItem {
                category: "Comando".to_string(),
                title: "Docker Logs".to_string(),
                subtitle: "docker logs --tail 100".to_string(),
                action_key: "cmd:docker logs".to_string(),
            },
        ];

        let filtered = filter_palette_items(&items, "sparkline");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].action_key, "act:resource-monitor");

        let completions = query_terminal_completions("sudo systemctl ", 10);
        assert!(!completions.is_empty());
        assert!(completions.iter().any(|c| c.text == "restart" || c.text == "status"));

        assert_eq!(
            extract_prompt_command_line("vagnarok@host:~/OnyxSH$ sudo dnf in\n"),
            "sudo dnf in"
        );
        assert_eq!(
            extract_prompt_command_line("root@prod:/etc# rpm -ivh --no"),
            "rpm -ivh --no"
        );
        let rpm_comps = query_terminal_completions("rpm -ivh --no", 6);
        assert!(rpm_comps.iter().any(|c| c.text == "--nodeps"));
    }

    #[test]
    fn test_create_terminal_context_menu_model() {
        // 1. Default context menu (no selection, no URL, no error)
        let menu_default = create_terminal_context_menu_model(false, None, None);
        assert_eq!(menu_default.n_items(), 4);

        // 2. With URL clicked
        let menu_url = create_terminal_context_menu_model(false, Some("https://github.com"), None);
        assert_eq!(menu_url.n_items(), 5);

        // 3. With Selection
        let menu_sel = create_terminal_context_menu_model(true, None, None);
        assert_eq!(menu_sel.n_items(), 4);

        // 4. With error code
        let menu_err = create_terminal_context_menu_model(false, None, Some(127));
        assert_eq!(menu_err.n_items(), 4);
    }

    #[test]
    fn test_production_hostname_confirmation_logic() {
        assert!(!validate_production_confirmation_input(
            Some("prod-db-01.internal"),
            "critical",
            "prod-db"
        ));
        assert!(validate_production_confirmation_input(
            Some("prod-db-01.internal"),
            "critical",
            "prod-db-01.internal"
        ));
        // Non-critical warnings do not require typing hostname even if provided
        assert!(validate_production_confirmation_input(
            Some("prod-db-01.internal"),
            "high",
            ""
        ));
        assert!(validate_production_confirmation_input(
            None,
            "critical",
            ""
        ));
    }

    #[test]
    fn test_should_notify_long_command_conditions() {
        // "unfocused": fires when window inactive, tab unselected, or terminal lacks focus
        assert!(!should_notify_long_command("unfocused", 0, true, true, true));
        assert!(should_notify_long_command("unfocused", 0, false, true, true));
        assert!(should_notify_long_command("unfocused", 0, true, false, true));
        assert!(should_notify_long_command("unfocused", 0, true, true, false));

        // "always": fires regardless of focus/exit code
        assert!(should_notify_long_command("always", 0, true, true, true));

        // "errors_only": fires only on non-zero exit
        assert!(!should_notify_long_command("errors_only", 0, false, false, false));
        assert!(should_notify_long_command("errors_only", 2, true, true, true));
    }

    #[test]
    fn test_resolve_production_target_hostname_fallback() {
        let host = resolve_production_target_hostname(None);
        assert!(!host.trim().is_empty());
    }

    #[test]
    fn test_extract_prompt_and_manual_ctrl_space_completions() {
        assert_eq!(extract_prompt_command_line("vagnarok@host:~$"), "");
        assert_eq!(extract_prompt_command_line("vagnarok@host:~$ "), "");
        assert_eq!(extract_prompt_command_line("vagnarok@host:~$ una"), "una");
        assert_eq!(extract_prompt_command_line("echo $"), "echo $");

        let settings = AppSettings::default();
        let hist = vec!["uname -a".to_string(), "apt update".to_string()];

        // Automatic trigger on empty prompt remains silent
        assert!(build_live_completions("", false, &settings, &hist, 6).is_empty());

        // Manual Ctrl+Space trigger on empty prompt surfaces SQLite history items
        let empty_manual = build_live_completions("", true, &settings, &hist, 6);
        assert_eq!(empty_manual.len(), 2);
        assert_eq!(empty_manual[0].text, "uname -a");

        // Substring SQLite match works even when not a prefix
        let substr = build_live_completions("update", true, &settings, &hist, 6);
        assert!(substr.iter().any(|i| i.text == "apt update"));
    }
}
