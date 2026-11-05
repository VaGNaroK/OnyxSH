//! Global Multi-Tab/Multi-Source Search (`Ctrl+Shift+G`),
//! Enriched Command History Dialog (`Ctrl+H` — `src/onyxsh/ui/dialogs/command_history_dialog.py`),
//! Interactive Keyboard Shortcuts Editor (`src/onyxsh/ui/dialogs/shortcuts_dialog.py`),
//! System Diagnostics Dialog (`src/onyxsh/ui/dialogs/diagnostics_dialog.py`),
//! and Complete 34-Action Window Shortcut Dispatcher (`src/onyxsh/ui/actions.py` & `app.py`).

use crate::command_manager_ui::CommandButton;
use crate::terminal_search::apply_vte_search;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::{accelerator_to_label, shortcut_groups, AppSettings};
use onyxsh_core::platform;
use onyxsh_core::sessions::{SessionItem, SessionsStorageData};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use vte4::prelude::*;

/// Category filter for Global Search (`Ctrl+Shift+G`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalSearchFilter {
    All,
    Terminals,
    Sessions,
    History,
    Snippets,
}

/// A single hit returned by Global Search across open terminal buffers, SSH sessions, history, and snippets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalSearchResult {
    pub source_type: GlobalSearchFilter,
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub tab_index: Option<u32>,
    pub matched_query: String,
    pub command_to_feed: Option<String>,
    pub session_index: Option<usize>,
}

/// Enriched command history entry (`src/onyxsh/data/command_history_manager.py`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandHistoryEntry {
    pub command: String,
    pub cwd: String,
    pub host: String,
    pub execution_count: u32,
    pub exit_code: i32,
    pub pinned: bool,
}

pub fn command_history_file_path() -> PathBuf {
    platform::config_dir().join("command_history.json")
}

/// Loads enriched command history from `~/.config/onyxsh/command_history.json`,
/// seeding from `~/.bash_history` or `~/.zsh_history` if empty.
pub fn load_command_history() -> Vec<CommandHistoryEntry> {
    let path = command_history_file_path();
    if path.exists() {
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(entries) = serde_json::from_str::<Vec<CommandHistoryEntry>>(&raw) {
                if !entries.is_empty() {
                    return entries;
                }
            }
        }
    }

    let home = platform::home_dir();
    let home_str = home.to_string_lossy().to_string();
    let mut seen = std::collections::HashSet::new();
    let mut entries = Vec::new();

    for hist_name in [".bash_history", ".zsh_history"] {
        let hist_path = home.join(hist_name);
        if let Ok(raw) = fs::read_to_string(&hist_path) {
            for line in raw.lines().rev() {
                let clean = line
                    .strip_prefix(": ")
                    .and_then(|s| s.split_once(';').map(|(_, cmd)| cmd))
                    .unwrap_or(line)
                    .trim();
                if clean.len() >= 2 && seen.insert(clean.to_string()) {
                    entries.push(CommandHistoryEntry {
                        command: clean.to_string(),
                        cwd: home_str.clone(),
                        host: "localhost".to_string(),
                        execution_count: 1,
                        exit_code: 0,
                        pinned: false,
                    });
                    if entries.len() >= 120 {
                        break;
                    }
                }
            }
        }
    }

    if entries.is_empty() {
        for sample in [
            "sudo dnf upgrade --refresh -y",
            "sudo apt update && sudo apt upgrade -y",
            "rpm -qa --last | head -n 20",
            "flatpak update -y",
            "systemctl status sshd",
            "docker ps -a",
            "git status -sb",
            "cargo test --workspace",
        ] {
            entries.push(CommandHistoryEntry {
                command: sample.to_string(),
                cwd: home_str.clone(),
                host: "localhost".to_string(),
                execution_count: 1,
                exit_code: 0,
                pinned: false,
            });
        }
    }

    entries
}

pub fn save_command_history(entries: &[CommandHistoryEntry]) {
    let path = command_history_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(entries) {
        let _ = fs::write(&path, json);
    }
}

/// Filters enriched command history entries by query and active pill (`all`, `cwd`, `host`, `pinned`).
pub fn filter_command_history(
    entries: &[CommandHistoryEntry],
    query: &str,
    pill: &str,
    current_cwd: &str,
    current_host: &str,
) -> Vec<CommandHistoryEntry> {
    let q = query.trim().to_lowercase();
    let mut out: Vec<CommandHistoryEntry> = entries
        .iter()
        .filter(|e| {
            let pill_ok = match pill {
                "cwd" => e.cwd == current_cwd,
                "host" => e.host.eq_ignore_ascii_case(current_host),
                "pinned" => e.pinned,
                _ => true,
            };
            if !pill_ok {
                return false;
            }
            if q.is_empty() {
                return true;
            }
            e.command.to_lowercase().contains(&q)
                || e.cwd.to_lowercase().contains(&q)
                || e.host.to_lowercase().contains(&q)
        })
        .cloned()
        .collect();

    // Sort pinned first, then by execution_count descending
    out.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.execution_count.cmp(&a.execution_count))
    });
    out
}

/// Collects all `vte4::Terminal` instances inside an `adw::TabPage` (supporting both single and split panes).
pub fn collect_page_terminals(page: &adw::TabPage) -> Vec<vte4::Terminal> {
    let mut out = Vec::new();
    collect_widget_terminals(&page.child(), &mut out);
    out
}

fn collect_widget_terminals(widget: &gtk4::Widget, out: &mut Vec<vte4::Terminal>) {
    if let Ok(term) = widget.clone().downcast::<vte4::Terminal>() {
        out.push(term);
        return;
    }
    if let Ok(paned) = widget.clone().downcast::<gtk4::Paned>() {
        if let Some(start) = paned.start_child() {
            collect_widget_terminals(&start, out);
        }
        if let Some(end) = paned.end_child() {
            collect_widget_terminals(&end, out);
        }
    }
}

/// Extracts visible/scrollback text from a `vte4::Terminal`.
pub fn extract_vte_text(term: &vte4::Terminal) -> String {
    let cols = term.column_count().max(80);
    let (_, cursor_row) = term.cursor_position();
    let start_row = (cursor_row - 500).max(0);
    let end_row = (cursor_row + 50).max(40);
    let (txt_opt, _) = term.text_range_format(vte4::Format::Text, start_row, 0, end_row, cols);
    txt_opt.map(|g| g.to_string()).unwrap_or_default()
}

/// Pure function that searches terminal buffer lines, sessions, history, and snippets for `query`.
pub fn compute_global_search_results(
    query: &str,
    filter: GlobalSearchFilter,
    terminal_buffers: &[(u32, String, String)], // (tab_index, tab_title, buffer_text)
    sessions: &[SessionItem],
    history: &[CommandHistoryEntry],
    snippets: &[CommandButton],
) -> Vec<GlobalSearchResult> {
    let q = query.trim().to_lowercase();
    let mut results = Vec::new();

    // 1. Open Terminal Buffers
    if filter == GlobalSearchFilter::All || filter == GlobalSearchFilter::Terminals {
        for (tab_idx, tab_title, buf) in terminal_buffers {
            for (line_no, line) in buf.lines().enumerate() {
                let clean = line.trim();
                if clean.is_empty() {
                    continue;
                }
                if q.is_empty() || clean.to_lowercase().contains(&q) {
                    results.push(GlobalSearchResult {
                        source_type: GlobalSearchFilter::Terminals,
                        badge: format!("ABA #{} (L{})", tab_idx + 1, line_no + 1),
                        title: clean.chars().take(90).collect(),
                        subtitle: format!("Terminal: {tab_title}"),
                        tab_index: Some(*tab_idx),
                        matched_query: query.trim().to_string(),
                        command_to_feed: None,
                        session_index: None,
                    });
                    if results.len() >= 40 {
                        break;
                    }
                }
            }
        }
    }

    // 2. Saved SSH Sessions
    if filter == GlobalSearchFilter::All || filter == GlobalSearchFilter::Sessions {
        for (idx, sess) in sessions.iter().enumerate() {
            let conn = sess.connection_string();
            if q.is_empty()
                || sess.name.to_lowercase().contains(&q)
                || conn.to_lowercase().contains(&q)
                || sess.host.to_lowercase().contains(&q)
            {
                results.push(GlobalSearchResult {
                    source_type: GlobalSearchFilter::Sessions,
                    badge: "SESSÃO SSH".to_string(),
                    title: sess.name.clone(),
                    subtitle: format!("{} ({}:{})", conn, sess.session_type, sess.port),
                    tab_index: None,
                    matched_query: String::new(),
                    command_to_feed: None,
                    session_index: Some(idx),
                });
            }
        }
    }

    // 3. Command History
    if filter == GlobalSearchFilter::All || filter == GlobalSearchFilter::History {
        for entry in history.iter().take(80) {
            if q.is_empty()
                || entry.command.to_lowercase().contains(&q)
                || entry.cwd.to_lowercase().contains(&q)
            {
                results.push(GlobalSearchResult {
                    source_type: GlobalSearchFilter::History,
                    badge: if entry.pinned {
                        "⭐ HISTÓRICO".to_string()
                    } else {
                        "HISTÓRICO".to_string()
                    },
                    title: entry.command.clone(),
                    subtitle: format!("📁 {} • {}x execuções", entry.cwd, entry.execution_count),
                    tab_index: None,
                    matched_query: String::new(),
                    command_to_feed: Some(entry.command.clone()),
                    session_index: None,
                });
            }
        }
    }

    // 4. Snippets & Command Manager Buttons
    if filter == GlobalSearchFilter::All || filter == GlobalSearchFilter::Snippets {
        for snip in snippets {
            if q.is_empty()
                || snip.name.to_lowercase().contains(&q)
                || snip.command_template.to_lowercase().contains(&q)
                || snip.category.to_lowercase().contains(&q)
            {
                results.push(GlobalSearchResult {
                    source_type: GlobalSearchFilter::Snippets,
                    badge: format!("SNIPPET ({})", snip.category),
                    title: snip.name.clone(),
                    subtitle: format!("$ {}", snip.command_template),
                    tab_index: None,
                    matched_query: String::new(),
                    command_to_feed: Some(snip.command_template.clone()),
                    session_index: None,
                });
            }
        }
    }

    results.truncate(80);
    results
}

/// Opens the **Global Multi-Tab & Multi-Source Search Dialog** (`Ctrl+Shift+G`).
pub fn show_global_search_dialog(
    parent: &adw::ApplicationWindow,
    tab_view: &adw::TabView,
    cmd_buttons: &[CommandButton],
    active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>>,
    on_spawn_session: Rc<dyn Fn(&SessionItem)>,
    status_lbl: &gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Busca Global no OnyxSH (Ctrl+Shift+G) — Abas, Sessões, Histórico & Snippets")
        .default_width(740)
        .default_height(520)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(8);
    content.set_margin_bottom(14);
    content.set_vexpand(true);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text(
            "Pesquisar texto em todas as abas de terminal abertas, sessões SSH, histórico e snippets...",
        )
        .hexpand(true)
        .build();
    content.append(&search_entry);

    // Filter Pills
    let filter_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let btn_all = gtk4::ToggleButton::with_label("🌐 Tudo");
    btn_all.set_active(true);
    let btn_terms = gtk4::ToggleButton::with_label("🖥️ Abas de Terminal");
    btn_terms.set_group(Some(&btn_all));
    let btn_sess = gtk4::ToggleButton::with_label("🔌 Sessões SSH");
    btn_sess.set_group(Some(&btn_all));
    let btn_hist = gtk4::ToggleButton::with_label("📜 Histórico");
    btn_hist.set_group(Some(&btn_all));
    let btn_snip = gtk4::ToggleButton::with_label("⚡ Snippets");
    btn_snip.set_group(Some(&btn_all));

    let count_lbl = gtk4::Label::new(Some("0 resultados"));
    count_lbl.add_css_class("caption");
    count_lbl.add_css_class("dim-label");
    count_lbl.set_hexpand(true);
    count_lbl.set_halign(gtk4::Align::End);

    filter_bar.append(&btn_all);
    filter_bar.append(&btn_terms);
    filter_bar.append(&btn_sess);
    filter_bar.append(&btn_hist);
    filter_bar.append(&btn_snip);
    filter_bar.append(&count_lbl);
    content.append(&filter_bar);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    scroll.set_child(Some(&list_box));
    content.append(&scroll);

    let hint_lbl = gtk4::Label::new(Some(
        "↵ Ir para Aba / Executar • ↑/↓ Navegar • Esc Fechar",
    ));
    hint_lbl.add_css_class("caption");
    hint_lbl.add_css_class("dim-label");
    hint_lbl.set_halign(gtk4::Align::Start);
    content.append(&hint_lbl);

    // Snapshot open terminal buffers
    let mut term_buffers = Vec::new();
    let n_pages = tab_view.n_pages();
    for i in 0..n_pages {
        let page = tab_view.nth_page(i);
        let title = page.title().to_string();
        let terms = collect_page_terminals(&page);
        let mut combined = String::new();
        for t in terms {
            let txt = extract_vte_text(&t);
            if !txt.trim().is_empty() {
                combined.push_str(&txt);
                combined.push('\n');
            }
        }
        term_buffers.push((i as u32, title, combined));
    }

    let sessions_path = platform::config_dir().join("sessions.json");
    let sessions = SessionsStorageData::load_from_path(&sessions_path)
        .map(|s| s.sessions)
        .unwrap_or_default();
    let history = load_command_history();
    let snippets = cmd_buttons.to_vec();

    let term_buffers_rc = Rc::new(term_buffers);
    let sessions_rc = Rc::new(sessions);
    let history_rc = Rc::new(history);
    let snippets_rc = Rc::new(snippets);
    let current_results_rc: Rc<RefCell<Vec<GlobalSearchResult>>> =
        Rc::new(RefCell::new(Vec::new()));

    let activate_result: Rc<dyn Fn(usize)> = {
        let res_rc = Rc::clone(&current_results_rc);
        let tv = tab_view.clone();
        let sess_rc = Rc::clone(&sessions_rc);
        let on_sess = Rc::clone(&on_spawn_session);
        let act_term = Rc::clone(&active_term_cb);
        let st = status_lbl.clone();
        let d_close = dlg.clone();

        Rc::new(move |idx: usize| {
            let maybe_hit = res_rc.borrow().get(idx).cloned();
            if let Some(hit) = maybe_hit {
                d_close.close();
                if let Some(tab_idx) = hit.tab_index {
                    if (tab_idx as i32) < tv.n_pages() {
                        let page = tv.nth_page(tab_idx as i32);
                        tv.set_selected_page(&page);
                        if let Some(term) = collect_page_terminals(&page).first() {
                            term.grab_focus();
                            if !hit.matched_query.is_empty() {
                                apply_vte_search(term, &hit.matched_query, false, false, false);
                            }
                        }
                        st.set_text(&format!("Busca Global: focado na Aba #{}", tab_idx + 1));
                    }
                } else if let Some(s_idx) = hit.session_index {
                    if let Some(s) = sess_rc.get(s_idx) {
                        on_sess(s);
                    }
                } else if let Some(cmd) = hit.command_to_feed {
                    if let Some(term) = act_term() {
                        term.feed_child(cmd.as_bytes());
                        term.grab_focus();
                        st.set_text(&format!("Busca Global inseriu no prompt: {cmd}"));
                    }
                }
            }
        })
    };

    let populate: Rc<dyn Fn()> = {
        let lb = list_box.clone();
        let se = search_entry.clone();
        let b_terms = btn_terms.clone();
        let b_sess = btn_sess.clone();
        let b_hist = btn_hist.clone();
        let b_snip = btn_snip.clone();
        let cnt = count_lbl.clone();
        let tb = Rc::clone(&term_buffers_rc);
        let ss = Rc::clone(&sessions_rc);
        let hs = Rc::clone(&history_rc);
        let sn = Rc::clone(&snippets_rc);
        let res_rc = Rc::clone(&current_results_rc);
        let act_fn = Rc::clone(&activate_result);

        Rc::new(move || {
            while let Some(c) = lb.first_child() {
                lb.remove(&c);
            }
            let filter = if b_terms.is_active() {
                GlobalSearchFilter::Terminals
            } else if b_sess.is_active() {
                GlobalSearchFilter::Sessions
            } else if b_hist.is_active() {
                GlobalSearchFilter::History
            } else if b_snip.is_active() {
                GlobalSearchFilter::Snippets
            } else {
                GlobalSearchFilter::All
            };

            let hits =
                compute_global_search_results(&se.text(), filter, &tb, &ss, &hs, &sn);
            cnt.set_text(&format!("{} resultados", hits.len()));

            for (idx, hit) in hits.iter().enumerate() {
                let row = gtk4::ListBoxRow::new();
                let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                hbox.set_margin_start(10);
                hbox.set_margin_end(10);
                hbox.set_margin_top(6);
                hbox.set_margin_bottom(6);

                let badge_lbl = gtk4::Label::new(Some(&hit.badge));
                badge_lbl.add_css_class("caption");
                badge_lbl.add_css_class("monospace");
                badge_lbl.set_width_request(135);
                badge_lbl.set_halign(gtk4::Align::Start);

                let text_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                text_vbox.set_hexpand(true);

                let title_lbl = gtk4::Label::new(Some(&hit.title));
                title_lbl.add_css_class("monospace");
                title_lbl.add_css_class("heading");
                title_lbl.set_halign(gtk4::Align::Start);
                title_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                let sub_lbl = gtk4::Label::new(Some(&hit.subtitle));
                sub_lbl.add_css_class("caption");
                sub_lbl.add_css_class("dim-label");
                sub_lbl.set_halign(gtk4::Align::Start);
                sub_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                text_vbox.append(&title_lbl);
                text_vbox.append(&sub_lbl);

                let btn_go = gtk4::Button::with_label(if hit.tab_index.is_some() {
                    "Ir p/ Aba"
                } else {
                    "Aplicar"
                });
                btn_go.add_css_class("flat");
                let af = Rc::clone(&act_fn);
                btn_go.connect_clicked(move |_| af(idx));

                hbox.append(&badge_lbl);
                hbox.append(&text_vbox);
                hbox.append(&btn_go);
                row.set_child(Some(&hbox));
                lb.append(&row);
            }

            if let Some(first) = lb.row_at_index(0) {
                lb.select_row(Some(&first));
            }
            *res_rc.borrow_mut() = hits;
        })
    };

    populate();

    {
        let pop = Rc::clone(&populate);
        search_entry.connect_search_changed(move |_| pop());
    }
    for btn in [&btn_all, &btn_terms, &btn_sess, &btn_hist, &btn_snip] {
        let pop = Rc::clone(&populate);
        btn.connect_toggled(move |b| {
            if b.is_active() {
                pop();
            }
        });
    }
    {
        let af = Rc::clone(&activate_result);
        list_box.connect_row_activated(move |_, row| {
            if row.index() >= 0 {
                af(row.index() as usize);
            }
        });
    }
    {
        let af = Rc::clone(&activate_result);
        let lb = list_box.clone();
        search_entry.connect_activate(move |_| {
            let idx = lb.selected_row().map(|r| r.index()).unwrap_or(0);
            if idx >= 0 {
                af(idx as usize);
            }
        });
    }

    // Keyboard navigation (Up/Down/Escape)
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        let lb = list_box.clone();
        let d_close = dlg.clone();
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

/// Opens the **Enriched Command History Dialog (`Ctrl+H`)** (`src/onyxsh/ui/dialogs/command_history_dialog.py`).
pub fn show_command_history_dialog(
    parent: &adw::ApplicationWindow,
    active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>>,
    status_lbl: &gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Histórico Enriquecido de Comandos (Ctrl+H)")
        .default_width(760)
        .default_height(540)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    let hb = adw::HeaderBar::new();

    let btn_clear = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Limpar Histórico Não-Favoritado")
        .build();
    btn_clear.add_css_class("flat");
    hb.pack_start(&btn_clear);
    vbox.append(&hb);

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(8);
    content.set_margin_bottom(12);
    content.set_vexpand(true);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Buscar comando por texto, diretório ou parâmetros...")
        .hexpand(true)
        .build();
    content.append(&search_entry);

    let current_cwd = std::env::current_dir()
        .unwrap_or_else(|_| platform::home_dir())
        .to_string_lossy()
        .to_string();

    let filter_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    let btn_all = gtk4::ToggleButton::with_label("Todos");
    btn_all.set_active(true);
    let btn_cwd = gtk4::ToggleButton::with_label("📁 Diretório Atual");
    btn_cwd.set_group(Some(&btn_all));
    let btn_pinned = gtk4::ToggleButton::with_label("⭐ Favoritos");
    btn_pinned.set_group(Some(&btn_all));

    let stats_lbl = gtk4::Label::new(Some(""));
    stats_lbl.add_css_class("caption");
    stats_lbl.add_css_class("dim-label");
    stats_lbl.set_hexpand(true);
    stats_lbl.set_halign(gtk4::Align::End);

    filter_box.append(&btn_all);
    filter_box.append(&btn_cwd);
    filter_box.append(&btn_pinned);
    filter_box.append(&stats_lbl);
    content.append(&filter_box);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    scroll.set_child(Some(&list_box));
    content.append(&scroll);

    let footer = gtk4::Label::new(Some(
        "↵ Executar   •   Tab Inserir no Prompt   •   ⭐ Favoritar   •   Esc Fechar",
    ));
    footer.add_css_class("caption");
    footer.add_css_class("dim-label");
    footer.set_halign(gtk4::Align::Start);
    content.append(&footer);

    let entries_rc = Rc::new(RefCell::new(load_command_history()));
    let filtered_rc: Rc<RefCell<Vec<CommandHistoryEntry>>> = Rc::new(RefCell::new(Vec::new()));

    let populate_cell: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let populate: Rc<dyn Fn()> = {
        let lb = list_box.clone();
        let se = search_entry.clone();
        let b_cwd = btn_cwd.clone();
        let b_pin = btn_pinned.clone();
        let st_lbl = stats_lbl.clone();
        let all_rc = Rc::clone(&entries_rc);
        let filt_rc = Rc::clone(&filtered_rc);
        let cwd_str = current_cwd.clone();
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();
        let d_close = dlg.clone();
        let pop_self = Rc::clone(&populate_cell);

        Rc::new(move || {
            while let Some(c) = lb.first_child() {
                lb.remove(&c);
            }
            let pill = if b_cwd.is_active() {
                "cwd"
            } else if b_pin.is_active() {
                "pinned"
            } else {
                "all"
            };
            let list =
                filter_command_history(&all_rc.borrow(), &se.text(), pill, &cwd_str, "localhost");
            st_lbl.set_text(&format!("{} comandos", list.len()));

            for entry in &list {
                let row = gtk4::ListBoxRow::new();
                let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                hbox.set_margin_start(10);
                hbox.set_margin_end(10);
                hbox.set_margin_top(6);
                hbox.set_margin_bottom(6);

                let pin_btn = gtk4::Button::with_label(if entry.pinned { "⭐" } else { "☆" });
                pin_btn.add_css_class("flat");
                pin_btn.set_tooltip_text(Some("Favoritar / Desfavoritar comando"));

                let txt_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                txt_vbox.set_hexpand(true);

                let cmd_lbl = gtk4::Label::new(Some(&entry.command));
                cmd_lbl.add_css_class("monospace");
                cmd_lbl.add_css_class("heading");
                cmd_lbl.set_halign(gtk4::Align::Start);
                cmd_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                let meta_lbl = gtk4::Label::new(Some(&format!(
                    "📁 {} • {}x • exit {}",
                    entry.cwd, entry.execution_count, entry.exit_code
                )));
                meta_lbl.add_css_class("caption");
                meta_lbl.add_css_class("dim-label");
                meta_lbl.set_halign(gtk4::Align::Start);

                txt_vbox.append(&cmd_lbl);
                txt_vbox.append(&meta_lbl);

                let btn_insert = gtk4::Button::with_label("Inserir (Tab)");
                btn_insert.add_css_class("flat");
                let btn_run = gtk4::Button::with_label("Executar (↵)");
                btn_run.add_css_class("suggested-action");

                {
                    let cmd_str = entry.command.clone();
                    let all_ref = Rc::clone(&all_rc);
                    let ps = Rc::clone(&pop_self);
                    pin_btn.connect_clicked(move |_| {
                        {
                            let mut vec = all_ref.borrow_mut();
                            if let Some(found) = vec.iter_mut().find(|e| e.command == cmd_str) {
                                found.pinned = !found.pinned;
                            }
                            save_command_history(&vec);
                        }
                        if let Some(p) = ps.borrow().as_ref() {
                            p();
                        }
                    });
                }
                {
                    let cmd_str = entry.command.clone();
                    let at = Rc::clone(&act_term);
                    let st = status.clone();
                    let dc = d_close.clone();
                    btn_insert.connect_clicked(move |_| {
                        if let Some(term) = at() {
                            term.feed_child(cmd_str.as_bytes());
                            term.grab_focus();
                            st.set_text(&format!("Comando inserido no prompt: {cmd_str}"));
                        }
                        dc.close();
                    });
                }
                {
                    let cmd_str = entry.command.clone();
                    let at = Rc::clone(&act_term);
                    let st = status.clone();
                    let dc = d_close.clone();
                    btn_run.connect_clicked(move |_| {
                        if let Some(term) = at() {
                            term.feed_child(format!("{cmd_str}\n").as_bytes());
                            term.grab_focus();
                            st.set_text(&format!("Comando executado do histórico: {cmd_str}"));
                        }
                        dc.close();
                    });
                }

                hbox.append(&pin_btn);
                hbox.append(&txt_vbox);
                hbox.append(&btn_insert);
                hbox.append(&btn_run);
                row.set_child(Some(&hbox));
                lb.append(&row);
            }

            if let Some(first) = lb.row_at_index(0) {
                lb.select_row(Some(&first));
            }
            *filt_rc.borrow_mut() = list;
        })
    };

    *populate_cell.borrow_mut() = Some(Rc::clone(&populate));
    populate();

    {
        let pop = Rc::clone(&populate);
        search_entry.connect_search_changed(move |_| pop());
    }
    for btn in [&btn_all, &btn_cwd, &btn_pinned] {
        let pop = Rc::clone(&populate);
        btn.connect_toggled(move |b| {
            if b.is_active() {
                pop();
            }
        });
    }
    {
        let all_ref = Rc::clone(&entries_rc);
        let pop = Rc::clone(&populate);
        btn_clear.connect_clicked(move |_| {
            all_ref.borrow_mut().retain(|e| e.pinned);
            save_command_history(&all_ref.borrow());
            pop();
        });
    }

    // Keyboard controller: Enter runs, Tab inserts, Up/Down navigates, Esc closes
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let lb = list_box.clone();
        let filt_rc = Rc::clone(&filtered_rc);
        let at = Rc::clone(&active_term_cb);
        let st = status_lbl.clone();
        let dc = dlg.clone();

        key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                dc.close();
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
            if keyval == gdk::Key::Tab || keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter
            {
                let idx = lb.selected_row().map(|r| r.index()).unwrap_or(0);
                if idx >= 0 {
                    if let Some(item) = filt_rc.borrow().get(idx as usize) {
                        if let Some(term) = at() {
                            if keyval == gdk::Key::Tab {
                                term.feed_child(item.command.as_bytes());
                            } else {
                                term.feed_child(format!("{}\n", item.command).as_bytes());
                            }
                            term.grab_focus();
                            st.set_text(&format!("Histórico: {}", item.command));
                        }
                        dc.close();
                        return glib::Propagation::Stop;
                    }
                }
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

/// Opens the **Interactive Keyboard Shortcuts Editor Dialog** (`src/onyxsh/ui/dialogs/shortcuts_dialog.py`).
pub fn show_editable_shortcuts_dialog(
    parent: &adw::ApplicationWindow,
    settings_rc: &Rc<RefCell<AppSettings>>,
    settings_path: &PathBuf,
    status_lbl: &gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Atalhos de Teclado — OnyxSH (34 Ações Configuráveis)")
        .default_width(720)
        .default_height(600)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let hb = adw::HeaderBar::new();

    let btn_reset_all = gtk4::Button::with_label("Resetar Todos");
    btn_reset_all.add_css_class("destructive-action");
    hb.pack_end(&btn_reset_all);
    vbox.append(&hb);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);

    let groups_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    groups_vbox.set_margin_start(18);
    groups_vbox.set_margin_end(18);
    groups_vbox.set_margin_top(12);
    groups_vbox.set_margin_bottom(18);
    scroll.set_child(Some(&groups_vbox));

    let rebuild_cell: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let rebuild: Rc<dyn Fn()> = {
        let gvbox = groups_vbox.clone();
        let st_rc = Rc::clone(settings_rc);
        let spath = settings_path.clone();
        let status = status_lbl.clone();
        let dlg_ref = dlg.clone();
        let reb_self = Rc::clone(&rebuild_cell);

        Rc::new(move || {
            while let Some(c) = gvbox.first_child() {
                gvbox.remove(&c);
            }

            for (group_title, actions) in shortcut_groups() {
                let grp_lbl = gtk4::Label::new(Some(group_title));
                grp_lbl.add_css_class("title-4");
                grp_lbl.set_halign(gtk4::Align::Start);
                gvbox.append(&grp_lbl);

                let list = gtk4::ListBox::new();
                list.add_css_class("boxed-list");
                list.set_selection_mode(gtk4::SelectionMode::None);

                for (act_key, act_label) in actions {
                    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
                    row.set_margin_start(12);
                    row.set_margin_end(12);
                    row.set_margin_top(8);
                    row.set_margin_bottom(8);

                    let title_lbl = gtk4::Label::new(Some(act_label));
                    title_lbl.set_hexpand(true);
                    title_lbl.set_halign(gtk4::Align::Start);

                    let current_accel = st_rc
                        .borrow()
                        .shortcuts
                        .get(act_key)
                        .cloned()
                        .unwrap_or_default();
                    let badge_lbl = gtk4::Label::new(Some(&accelerator_to_label(&current_accel)));
                    badge_lbl.add_css_class("monospace");
                    badge_lbl.add_css_class("dim-label");

                    let btn_edit = gtk4::Button::with_label("Editar");
                    btn_edit.add_css_class("flat");

                    {
                        let key_str = act_key.to_string();
                        let label_str = act_label.to_string();
                        let st_edit = Rc::clone(&st_rc);
                        let sp_edit = spath.clone();
                        let status_edit = status.clone();
                        let parent_dlg = dlg_ref.clone();
                        let rb = Rc::clone(&reb_self);

                        btn_edit.connect_clicked(move |_| {
                            let cap_dlg = adw::Window::builder()
                                .transient_for(&parent_dlg)
                                .modal(true)
                                .title(&format!("Definir Atalho: {label_str}"))
                                .default_width(420)
                                .default_height(220)
                                .build();

                            let cvbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
                            cvbox.append(&adw::HeaderBar::new());

                            let cbody = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
                            cbody.set_margin_start(16);
                            cbody.set_margin_end(16);
                            cbody.set_margin_top(8);
                            cbody.set_margin_bottom(14);

                            let info = gtk4::Label::new(Some(
                                "Pressione a nova combinação de teclas ou edite abaixo:",
                            ));
                            info.set_halign(gtk4::Align::Start);
                            cbody.append(&info);

                            let curr_val = st_edit
                                .borrow()
                                .shortcuts
                                .get(&key_str)
                                .cloned()
                                .unwrap_or_default();
                            let entry = gtk4::Entry::builder()
                                .text(&curr_val)
                                .hexpand(true)
                                .build();
                            entry.add_css_class("monospace");
                            cbody.append(&entry);

                            // Key capture controller on the dialog
                            let kctrl = gtk4::EventControllerKey::new();
                            {
                                let e_ref = entry.clone();
                                kctrl.connect_key_pressed(move |_, keyval, _, state| {
                                    let is_mod_only = matches!(
                                        keyval,
                                        gdk::Key::Control_L
                                            | gdk::Key::Control_R
                                            | gdk::Key::Shift_L
                                            | gdk::Key::Shift_R
                                            | gdk::Key::Alt_L
                                            | gdk::Key::Alt_R
                                            | gdk::Key::Super_L
                                            | gdk::Key::Super_R
                                    );
                                    if is_mod_only {
                                        return glib::Propagation::Proceed;
                                    }
                                    let mods = state
                                        & (gdk::ModifierType::CONTROL_MASK
                                            | gdk::ModifierType::SHIFT_MASK
                                            | gdk::ModifierType::ALT_MASK
                                            | gdk::ModifierType::SUPER_MASK);
                                    let accel_name = gtk4::accelerator_name(keyval, mods);
                                    if !accel_name.is_empty() {
                                        e_ref.set_text(&accel_name);
                                        return glib::Propagation::Stop;
                                    }
                                    glib::Propagation::Proceed
                                });
                            }
                            cap_dlg.add_controller(kctrl);

                            let brow = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                            brow.set_halign(gtk4::Align::End);
                            let btn_clear = gtk4::Button::with_label("Limpar");
                            btn_clear.add_css_class("destructive-action");
                            let btn_save = gtk4::Button::with_label("Salvar Atalho");
                            btn_save.add_css_class("suggested-action");
                            brow.append(&btn_clear);
                            brow.append(&btn_save);
                            cbody.append(&brow);

                            {
                                let k = key_str.clone();
                                let st_c = Rc::clone(&st_edit);
                                let sp_c = sp_edit.clone();
                                let rb_c = Rc::clone(&rb);
                                let cd = cap_dlg.clone();
                                btn_clear.connect_clicked(move |_| {
                                    st_c.borrow_mut().shortcuts.insert(k.clone(), String::new());
                                    let _ = st_c.borrow().save_to_path(&sp_c);
                                    if let Some(f) = rb_c.borrow().as_ref() {
                                        f();
                                    }
                                    cd.close();
                                });
                            }
                            {
                                let k = key_str.clone();
                                let e_s = entry.clone();
                                let st_s = Rc::clone(&st_edit);
                                let sp_s = sp_edit.clone();
                                let stat_s = status_edit.clone();
                                let rb_s = Rc::clone(&rb);
                                let cd = cap_dlg.clone();
                                btn_save.connect_clicked(move |_| {
                                    let new_acc = e_s.text().trim().to_string();
                                    st_s.borrow_mut()
                                        .shortcuts
                                        .insert(k.clone(), new_acc.clone());
                                    let _ = st_s.borrow().save_to_path(&sp_s);
                                    stat_s.set_text(&format!(
                                        "Atalho '{}' atualizado para {}",
                                        k,
                                        accelerator_to_label(&new_acc)
                                    ));
                                    if let Some(f) = rb_s.borrow().as_ref() {
                                        f();
                                    }
                                    cd.close();
                                });
                            }

                            cvbox.append(&cbody);
                            cap_dlg.set_content(Some(&cvbox));
                            cap_dlg.present();
                        });
                    }

                    row.append(&title_lbl);
                    row.append(&badge_lbl);
                    row.append(&btn_edit);
                    list.append(&row);
                }
                gvbox.append(&list);
            }
        })
    };

    *rebuild_cell.borrow_mut() = Some(Rc::clone(&rebuild));
    rebuild();

    {
        let st_r = Rc::clone(settings_rc);
        let sp_r = settings_path.clone();
        let status_r = status_lbl.clone();
        let rb = Rc::clone(&rebuild);
        btn_reset_all.connect_clicked(move |_| {
            st_r.borrow_mut().reset_shortcuts_to_default();
            let _ = st_r.borrow().save_to_path(&sp_r);
            status_r.set_text("Todos os 34 atalhos de teclado foram restaurados para o padrão.");
            rb();
        });
    }

    vbox.append(&scroll);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Opens the **System Diagnostics Dialog** (`src/onyxsh/ui/dialogs/diagnostics_dialog.py`).
pub fn show_system_diagnostics_dialog(parent: &adw::ApplicationWindow, status_lbl: &gtk4::Label) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Diagnóstico Seguro do Sistema — Telemetria & Suporte")
        .default_width(620)
        .default_height(440)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(8);
    content.set_margin_bottom(14);
    content.set_vexpand(true);

    let os_info = platform::detect_os_context();
    let mem = fs::read_to_string("/proc/meminfo")
        .map(|s| onyxsh_system::parse_proc_meminfo(&s))
        .unwrap_or_default();
    let gpu_summary = crate::resource_dashboard::probe_local_gpu_vram()
        .map(|g| format!("{} / {} MB VRAM ({:.1}%)", g.used_mb, g.total_mb, g.usage_percent))
        .unwrap_or_else(|| "Nenhuma GPU dedicada detectada via sysfs/nvidia-smi".to_string());

    let report = format!(
        "=== Relatório de Diagnóstico Sanitizado do OnyxSH ===\n\
         Versão: {} ({})\n\
         Motor: Rust Nativo (GTK4 + Libadwaita + VTE4)\n\
         Sistema Operacional: {}\n\
         Ambiente Flatpak: {}\n\
         Memória RAM: {} / {} MB ({:.1}%)\n\
         GPU / VRAM: {}\n\
         Diretório de Config: {}\n\
         Zero-Trust SecretRedactor: Ativo\n\
         ProductionGuard: Ativo\n",
        onyxsh_core::APP_VERSION,
        onyxsh_core::APP_ID,
        os_info,
        platform::is_flatpak_sandbox(),
        mem.ram_used_bytes / (1024 * 1024),
        mem.ram_total_bytes / (1024 * 1024),
        mem.ram_percent,
        gpu_summary,
        platform::config_dir().display()
    );

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let tv = gtk4::TextView::new();
    tv.set_editable(false);
    tv.set_monospace(true);
    tv.buffer().set_text(&report);
    scroll.set_child(Some(&tv));
    content.append(&scroll);

    let btn_copy = gtk4::Button::with_label("📋 Copiar Relatório Sanitizado");
    btn_copy.add_css_class("suggested-action");
    {
        let rep = report;
        let st = status_lbl.clone();
        btn_copy.connect_clicked(move |btn| {
            btn.clipboard().set_text(&rep);
            st.set_text("Relatório de diagnóstico copiado para a área de transferência.");
        });
    }
    content.append(&btn_copy);

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_search_and_command_history_filtering() {
        let buffers = vec![(
            0u32,
            "Terminal 1".to_string(),
            "vagnarok@mint:~$ sudo dnf upgrade\nComplete!\n".to_string(),
        )];
        let sessions = vec![SessionItem {
            name: "Prod DB".to_string(),
            host: "10.0.0.50".to_string(),
            user: "admin".to_string(),
            port: 22,
            session_type: "ssh".to_string(),
            ..SessionItem::default()
        }];
        let history = vec![CommandHistoryEntry {
            command: "rpm -qa --last".to_string(),
            cwd: "/home/vagnarok".to_string(),
            host: "localhost".to_string(),
            execution_count: 5,
            exit_code: 0,
            pinned: true,
        }];

        let hits = compute_global_search_results(
            "dnf",
            GlobalSearchFilter::All,
            &buffers,
            &sessions,
            &history,
            &[],
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].tab_index, Some(0));

        let hist_hits =
            filter_command_history(&history, "rpm", "pinned", "/home/vagnarok", "localhost");
        assert_eq!(hist_hits.len(), 1);
        assert_eq!(hist_hits[0].command, "rpm -qa --last");
    }
}
