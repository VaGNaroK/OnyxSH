//! Phase 11 (Etapa 2): Complete Session Panel (`src/onyxsh/sessions/tree.py`, `ui/sidebar_manager.py`,
//! `ui/dialogs/session_edit_dialog.py`, `ui/dialogs/folder_edit_dialog.py`).
//!
//! Provides:
//! - 5-button CRUD Toolbar (Add Session, Add Folder, Edit Selected, Save Layout, Remove Selected)
//! - Hierarchical Folder & Session Tree with `PROD` badges, `user@host:port` subtitles, and `.ssh/config` group
//! - Real-time bottom Search Entry (`sidebar_search_entry`) with automatic folder expansion
//! - Modal Dialogs for Creating/Editing Sessions (with AES-256-CBC V2 password encryption) and Folders

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::crypto::{decrypt_session_password, encrypt_session_password, is_securecrt_v2_password};
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::importers::{import_securecrt_directory, parse_connection_uri};
use onyxsh_core::keyring::{KeyringManager, SecretToolBackend};
use onyxsh_core::platform;
use onyxsh_core::security::{is_valid_host, is_valid_port, is_valid_username};
use onyxsh_core::sessions::{SessionFolder, SessionItem, SessionsStorageData};
use onyxsh_core::ssh_config::SshConfigParser;
use onyxsh_core::window_state::{TabNodeState, WindowSessionState};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use vte4::prelude::*;

thread_local! {
    static PANE_SESSION_META: RefCell<HashMap<String, SessionItem>> = RefCell::new(HashMap::new());
    static PANE_COUNTER: RefCell<u64> = const { RefCell::new(0) };
}

/// Associates live `SessionItem` metadata with a `vte4::Terminal` widget safely for split/layout capture.
pub fn register_pane_session(term: &vte4::Terminal, session: Option<SessionItem>) {
    let key = PANE_COUNTER.with(|c| {
        let mut guard = c.borrow_mut();
        *guard += 1;
        format!("onyx-pane-{}", *guard)
    });
    term.set_widget_name(&key);
    if let Some(sess) = session {
        PANE_SESSION_META.with(|map| {
            map.borrow_mut().insert(key, sess);
        });
    }
}

/// Returns the saved [`SessionItem`] metadata associated with a `vte4::Terminal`, if any.
pub fn get_pane_session(term: &vte4::Terminal) -> Option<SessionItem> {
    let key = term.widget_name();
    PANE_SESSION_META.with(|map| map.borrow().get(key.as_str()).cloned())
}

fn current_pane_cwd(term: &vte4::Terminal) -> Option<String> {
    let uri = term.current_directory_uri()?;
    let raw = uri.as_str();
    if let Some(stripped) = raw.strip_prefix("file://") {
        let path_part = match stripped.find('/') {
            Some(idx) => &stripped[idx..],
            None => stripped,
        };
        if !path_part.is_empty() {
            return Some(path_part.to_string());
        }
    }
    None
}

/// Recursively inspects a GTK4 tab child widget tree into a serializable [`TabNodeState`].
pub fn extract_tab_node_state(widget: &gtk4::Widget, fallback_title: &str) -> TabNodeState {
    if let Ok(bx) = widget.clone().downcast::<gtk4::Box>() {
        if let Some(inner) = bx.first_child() {
            return extract_tab_node_state(&inner, fallback_title);
        }
    }

    if let Ok(term) = widget.clone().downcast::<vte4::Terminal>() {
        let pane_key = term.widget_name().to_string();
        let sess = PANE_SESSION_META.with(|map| map.borrow().get(&pane_key).cloned());
        let cwd = current_pane_cwd(&term);
        let session_type = sess
            .as_ref()
            .map(|s| s.session_type.clone())
            .unwrap_or_else(|| "local".to_string());
        return TabNodeState::Terminal {
            session_type,
            title: fallback_title.to_string(),
            working_directory: cwd,
            session_data: sess,
        };
    }

    if let Ok(paned) = widget.clone().downcast::<gtk4::Paned>() {
        let orientation = if paned.orientation() == gtk4::Orientation::Vertical {
            "vertical".to_string()
        } else {
            "horizontal".to_string()
        };
        let first = paned
            .start_child()
            .map(|w| extract_tab_node_state(&w, fallback_title))
            .unwrap_or(TabNodeState::Terminal {
                session_type: "local".to_string(),
                title: fallback_title.to_string(),
                working_directory: None,
                session_data: None,
            });
        let second = paned
            .end_child()
            .map(|w| extract_tab_node_state(&w, fallback_title))
            .unwrap_or(TabNodeState::Terminal {
                session_type: "local".to_string(),
                title: fallback_title.to_string(),
                working_directory: None,
                session_data: None,
            });
        let total = if paned.orientation() == gtk4::Orientation::Vertical {
            paned.height()
        } else {
            paned.width()
        };
        let pos = paned.position();
        let ratio = if total > 0 {
            Some(((pos as f64) / (total as f64)).clamp(0.15, 0.85))
        } else {
            Some(0.5)
        };
        return TabNodeState::Paned {
            orientation,
            position: pos,
            position_ratio: ratio,
            child1: Box::new(first),
            child2: Box::new(second),
        };
    }

    TabNodeState::Terminal {
        session_type: "local".to_string(),
        title: fallback_title.to_string(),
        working_directory: None,
        session_data: None,
    }
}

/// Captures the full live `adw::TabView` + panel visibility into [`WindowSessionState`].
pub fn capture_window_session_state(
    tab_view: &adw::TabView,
    sidebar_visible: bool,
    ai_panel_visible: bool,
) -> WindowSessionState {
    let n_pages = tab_view.n_pages();
    let mut tabs = Vec::with_capacity(n_pages as usize);
    let mut active_tab_index = 0usize;
    let selected = tab_view.selected_page();
    for i in 0..n_pages {
        let page = tab_view.nth_page(i);
        if selected.as_ref().is_some_and(|sel| sel == &page) {
            active_tab_index = i as usize;
        }
        tabs.push(extract_tab_node_state(&page.child(), page.title().as_str()));
    }
    WindowSessionState {
        tabs,
        active_tab_index,
        sidebar_visible,
        ai_panel_visible,
    }
}

/// Represents a folder group and its matching sessions for rendering in the sidebar tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionFolderGroup {
    pub folder_path: String,
    pub display_name: String,
    pub sessions: Vec<SessionItem>,
    pub expanded: bool,
    pub is_readonly_ssh_config: bool,
}

/// Filters and groups saved sessions and `~/.ssh/config` entries by folder, matching `query`
/// (case-insensitive) against session name, host, user, or folder path.
pub fn filter_and_group_sessions(
    storage: &SessionsStorageData,
    ssh_config_items: &[SessionItem],
    query: &str,
) -> Vec<SessionFolderGroup> {
    let q = query.trim().to_lowercase();
    let matches_query = |s: &SessionItem| -> bool {
        if q.is_empty() {
            return true;
        }
        s.name.to_lowercase().contains(&q)
            || s.host.to_lowercase().contains(&q)
            || s.user.to_lowercase().contains(&q)
            || s.folder_path.to_lowercase().contains(&q)
    };

    let mut folder_map: BTreeMap<String, Vec<SessionItem>> = BTreeMap::new();

    // Seed explicit folders from storage when query is empty so empty user folders remain visible
    if q.is_empty() {
        for f in &storage.folders {
            let path = f.path.trim_matches('/').to_string();
            if !path.is_empty() {
                folder_map.entry(path).or_default();
            }
        }
    }

    let mut root_sessions = Vec::new();
    for s in &storage.sessions {
        if !matches_query(s) {
            continue;
        }
        let fp = s.folder_path.trim_matches('/').to_string();
        if fp.is_empty() {
            root_sessions.push(s.clone());
        } else {
            folder_map.entry(fp).or_default().push(s.clone());
        }
    }

    let mut groups = Vec::new();

    if !root_sessions.is_empty() {
        groups.push(SessionFolderGroup {
            folder_path: String::new(),
            display_name: tr("Sessões Salvas"),
            sessions: root_sessions,
            expanded: true,
            is_readonly_ssh_config: false,
        });
    }

    for (path, items) in folder_map {
        if !q.is_empty() && items.is_empty() {
            continue;
        }
        groups.push(SessionFolderGroup {
            display_name: path.clone(),
            folder_path: path,
            sessions: items,
            expanded: true,
            is_readonly_ssh_config: false,
        });
    }

    let filtered_ssh: Vec<SessionItem> = ssh_config_items
        .iter()
        .filter(|s| matches_query(s))
        .cloned()
        .collect();

    if !filtered_ssh.is_empty() {
        groups.push(SessionFolderGroup {
            folder_path: ".ssh/config".to_string(),
            display_name: "OpenSSH (~/.ssh/config)".to_string(),
            sessions: filtered_ssh,
            expanded: !q.is_empty(),
            is_readonly_ssh_config: true,
        });
    }

    groups
}

/// Inserts or updates a [`SessionItem`] inside [`SessionsStorageData`], encrypting plaintext
/// passwords with SecureCRT V2 AES-256-CBC (`02:<hex>`).
pub fn upsert_session_in_storage(
    storage: &mut SessionsStorageData,
    original_name: Option<&str>,
    mut item: SessionItem,
) {
    if item.auth_type == "password"
        && !item.auth_value.is_empty()
        && !is_securecrt_v2_password(&item.auth_value)
    {
        if let Ok(enc) = encrypt_session_password(&item.auth_value, "") {
            item.auth_value = enc;
        }
    }

    let clean_folder = item.folder_path.trim_matches('/').to_string();
    item.folder_path = clean_folder.clone();
    if !clean_folder.is_empty()
        && !storage
            .folders
            .iter()
            .any(|f| f.path.trim_matches('/') == clean_folder)
    {
        storage.folders.push(SessionFolder {
            name: clean_folder
                .rsplit('/')
                .next()
                .unwrap_or(&clean_folder)
                .to_string(),
            path: clean_folder,
            ..SessionFolder::default()
        });
    }

    if let Some(orig) = original_name {
        if let Some(existing) = storage.sessions.iter_mut().find(|s| s.name == orig) {
            *existing = item;
            return;
        }
    }
    if let Some(existing) = storage.sessions.iter_mut().find(|s| s.name == item.name) {
        *existing = item;
    } else {
        storage.sessions.push(item);
    }
}

/// Adds a new [`SessionFolder`] to [`SessionsStorageData`] if not already present.
pub fn upsert_folder_in_storage(storage: &mut SessionsStorageData, folder_path: &str) -> bool {
    let clean = folder_path.trim().trim_matches('/').to_string();
    if clean.is_empty() {
        return false;
    }
    if storage
        .folders
        .iter()
        .any(|f| f.path.trim_matches('/') == clean)
    {
        return false;
    }
    let name = clean.rsplit('/').next().unwrap_or(&clean).to_string();
    storage.folders.push(SessionFolder {
        name,
        path: clean,
        ..SessionFolder::default()
    });
    true
}

/// Removes a session by name from [`SessionsStorageData`].
pub fn remove_session_from_storage(storage: &mut SessionsStorageData, session_name: &str) -> bool {
    let before = storage.sessions.len();
    storage.sessions.retain(|s| s.name != session_name);
    storage.sessions.len() < before
}

/// Duplicates an existing session in [`SessionsStorageData`] under a unique name (`"<name> (Cópia)"`).
pub fn clone_session_in_storage(
    storage: &mut SessionsStorageData,
    source_name: &str,
) -> Option<SessionItem> {
    let original = storage.sessions.iter().find(|s| s.name == source_name)?.clone();
    let mut candidate = format!("{} (Cópia)", original.name);
    let mut idx = 2usize;
    while storage.sessions.iter().any(|s| s.name == candidate) {
        candidate = format!("{} (Cópia {})", original.name, idx);
        idx += 1;
    }
    let mut cloned = original;
    cloned.name = candidate;
    cloned.source = "manual".to_string();
    storage.sessions.push(cloned.clone());
    Some(cloned)
}

/// Moves a session into `target_folder` (normalized; empty string moves to root).
pub fn move_session_to_folder(
    storage: &mut SessionsStorageData,
    session_name: &str,
    target_folder: &str,
) -> bool {
    let clean_target = target_folder.trim().trim_matches('/').to_string();
    let Some(pos) = storage.sessions.iter().position(|s| s.name == session_name) else {
        return false;
    };
    if !clean_target.is_empty() {
        upsert_folder_in_storage(storage, &clean_target);
    }
    storage.sessions[pos].folder_path = clean_target;
    true
}

/// Renames a custom folder and updates nested folder paths and contained sessions.
pub fn rename_folder_in_storage(
    storage: &mut SessionsStorageData,
    old_path: &str,
    new_name: &str,
) -> Option<String> {
    let clean_old = old_path.trim().trim_matches('/').to_string();
    let clean_new_leaf = new_name.trim().trim_matches('/').replace('/', "_");
    if clean_old.is_empty() || clean_new_leaf.is_empty() {
        return None;
    }
    let new_path = match clean_old.rsplit_once('/') {
        Some((parent, _)) if !parent.is_empty() => format!("{}/{}", parent, clean_new_leaf),
        _ => clean_new_leaf.clone(),
    };
    if new_path == clean_old {
        return Some(new_path);
    }
    if storage
        .folders
        .iter()
        .any(|f| f.path.trim_matches('/') == new_path)
    {
        return None;
    }
    let mut touched = false;
    let old_prefix = format!("{}/", clean_old);
    let new_prefix = format!("{}/", new_path);

    for folder in &mut storage.folders {
        let cur = folder.path.trim_matches('/').to_string();
        if cur == clean_old {
            folder.name = clean_new_leaf.clone();
            folder.path = new_path.clone();
            touched = true;
        } else if let Some(rest) = cur.strip_prefix(&old_prefix) {
            folder.path = format!("{}{}", new_prefix, rest);
            touched = true;
        }
    }
    if !touched {
        return None;
    }
    for sess in &mut storage.sessions {
        let cur = sess.folder_path.trim_matches('/').to_string();
        if cur == clean_old {
            sess.folder_path = new_path.clone();
        } else if let Some(rest) = cur.strip_prefix(&old_prefix) {
            sess.folder_path = format!("{}{}", new_prefix, rest);
        }
    }
    Some(new_path)
}

/// Removes a folder (and nested subfolders); moves its sessions to root or deletes them.
pub fn remove_folder_from_storage(
    storage: &mut SessionsStorageData,
    folder_path: &str,
    move_sessions_to_root: bool,
) -> bool {
    let clean = folder_path.trim().trim_matches('/').to_string();
    if clean.is_empty() {
        return false;
    }
    let prefix = format!("{}/", clean);
    let before = storage.folders.len();
    storage.folders.retain(|f| {
        let cur = f.path.trim_matches('/');
        cur != clean && !cur.starts_with(&prefix)
    });
    if storage.folders.len() == before {
        return false;
    }
    if move_sessions_to_root {
        for sess in &mut storage.sessions {
            let cur = sess.folder_path.trim_matches('/');
            if cur == clean || cur.starts_with(&prefix) {
                sess.folder_path.clear();
            }
        }
    } else {
        storage.sessions.retain(|sess| {
            let cur = sess.folder_path.trim_matches('/');
            cur != clean && !cur.starts_with(&prefix)
        });
    }
    true
}

/// Resolves the plaintext password/key secret of a session (`02:` AES decrypt + system keyring fallback).
pub fn resolve_session_secret(item: &SessionItem) -> String {
    if item.auth_type != "password" {
        return item.auth_value.clone();
    }
    if is_securecrt_v2_password(&item.auth_value) {
        if let Ok(plain) = decrypt_session_password(&item.auth_value, "") {
            return plain;
        }
    }
    let mgr = KeyringManager::new(SecretToolBackend, "onyxsh");
    mgr.resolve_secret(&item.name, &item.auth_value)
}

/// Filters SSH config items using an ignored host list.
pub fn filter_ignored_ssh_hosts(items: Vec<SessionItem>, ignored: &[String]) -> Vec<SessionItem> {
    if ignored.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| !ignored.iter().any(|ig| ig == &item.name))
        .collect()
}

/// Persists an OpenSSH config host alias in `settings.ignored_ssh_hosts` so it stays hidden.
pub fn hide_ssh_config_host(host_name: &str) -> std::io::Result<()> {
    let cfg_path = platform::config_dir().join("settings.json");
    let mut settings = AppSettings::load_from_path(&cfg_path).unwrap_or_default();
    if !settings.ignored_ssh_hosts.iter().any(|h| h == host_name) {
        settings.ignored_ssh_hosts.push(host_name.to_string());
        settings.save_to_path(&cfg_path)?;
    }
    Ok(())
}

/// Returns the directory holding named workspace layouts (`~/.config/onyxsh/layouts`).
pub fn layouts_dir() -> PathBuf {
    platform::config_dir().join("layouts")
}

/// Lists saved named layout identifiers (`layouts/<name>.json`).
pub fn list_saved_layouts() -> Vec<String> {
    let dir = layouts_dir();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("json") {
                p.file_stem().and_then(|s| s.to_str()).map(str::to_string)
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

/// Saves a named layout file under `~/.config/onyxsh/layouts/<name>.json`.
pub fn save_named_layout(name: &str, state: &WindowSessionState) -> std::io::Result<PathBuf> {
    let safe_name: String = name
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let stem = if safe_name.is_empty() {
        "layout".to_string()
    } else {
        safe_name
    };
    let dir = layouts_dir();
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(format!("{}.json", stem));
    state.save_to_path(&target)?;
    Ok(target)
}

/// Loads `~/.ssh/config` hosts as `SessionItem`s, excluding `settings.ignored_ssh_hosts`.
pub fn load_ssh_config_sessions() -> Vec<SessionItem> {
    let ssh_cfg_path = platform::home_dir().join(".ssh/config");
    if !ssh_cfg_path.exists() {
        return Vec::new();
    }
    let cfg_path = platform::config_dir().join("settings.json");
    let ignored = AppSettings::load_from_path(&cfg_path)
        .map(|s| s.ignored_ssh_hosts)
        .unwrap_or_default();
    let mut parser = SshConfigParser::new();
    let items: Vec<SessionItem> = parser
        .parse(&ssh_cfg_path)
        .into_iter()
        .map(|host_entry| {
            let mut item = SessionItem::default();
            item.name = host_entry.alias.clone();
            item.session_type = "ssh".to_string();
            item.host = host_entry.hostname.unwrap_or(host_entry.alias);
            item.user = host_entry.user.unwrap_or_default();
            item.port = host_entry.port.unwrap_or(22);
            item.auth_value = host_entry.identity_file.unwrap_or_default();
            item.x11_forwarding = host_entry.forward_x11.unwrap_or(false);
            item.source = "ssh_config".to_string();
            item
        })
        .collect();
    filter_ignored_ssh_hosts(items, &ignored)
}

/// Opens the Create Folder modal dialog (`show_folder_create_dialog`).
fn show_folder_create_dialog(
    parent: &adw::ApplicationWindow,
    storage_rc: Rc<RefCell<SessionsStorageData>>,
    on_saved: Rc<dyn Fn()>,
    status_lbl: gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Nova Pasta de Sessões"))
        .default_width(400)
        .default_height(200)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(8);
    content.set_margin_bottom(16);

    let lbl = gtk4::Label::new(Some(&tr(
        "Caminho da Pasta (ex: Produção ou Cloud/AWS):",
    )));
    lbl.set_halign(gtk4::Align::Start);
    content.append(&lbl);

    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some(&tr("Produção/Servidores Web")));
    content.append(&entry);

    let btn_save = gtk4::Button::with_label(&tr("Create Folder"));
    btn_save.add_css_class("suggested-action");

    let dlg_clone = dlg.clone();
    btn_save.connect_clicked(move |_| {
        let path = entry.text().to_string();
        let mut st = storage_rc.borrow_mut();
        if upsert_folder_in_storage(&mut st, &path) {
            let sessions_path = platform::config_dir().join("sessions.json");
            let _ = st.save_to_path(&sessions_path);
            drop(st);
            on_saved();
            status_lbl.set_text(&tr_fmt(
                "Pasta '{path}' criada com sucesso.",
                &[("path", path.trim())],
            ));
            dlg_clone.close();
        }
    });

    content.append(&btn_save);
    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Opens the Create / Edit Session modal dialog (`ui/dialogs/session_edit_dialog.py`).
pub fn show_session_edit_dialog(
    parent: &adw::ApplicationWindow,
    existing: Option<SessionItem>,
    storage_rc: Rc<RefCell<SessionsStorageData>>,
    on_saved: Rc<dyn Fn()>,
    status_lbl: gtk4::Label,
) {
    let is_edit = existing.is_some();
    let seed = existing.clone().unwrap_or_default();
    let orig_name = existing.map(|s| s.name);

    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&if is_edit {
            tr("Editar Sessão — OnyxSH")
        } else {
            tr("Nova Sessão — OnyxSH")
        })
        .default_width(540)
        .default_height(560)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    vbox.append(&adw::HeaderBar::new());

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);

    let form = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    form.set_margin_start(18);
    form.set_margin_end(18);
    form.set_margin_top(14);
    form.set_margin_bottom(18);

    // 1. Session Name
    let name_entry = gtk4::Entry::new();
    name_entry.set_placeholder_text(Some(&tr("Ex: Produção Web-01")));
    if is_edit {
        name_entry.set_text(&seed.name);
    }
    form.append(&labeled_row(&tr("Nome da Sessão:"), &name_entry));

    // 2. Protocol Dropdown (ssh, local, telnet, serial)
    let proto_list = gtk4::StringList::new(&["ssh", "local", "telnet", "serial"]);
    let proto_dd = gtk4::DropDown::new(Some(proto_list), gtk4::Expression::NONE);
    proto_dd.set_selected(match seed.session_type.as_str() {
        "local" => 1,
        "telnet" => 2,
        "serial" => 3,
        _ => 0,
    });
    form.append(&labeled_row(&tr("Protocolo:"), &proto_dd));

    // 3. Folder Path
    let folder_entry = gtk4::Entry::new();
    folder_entry.set_placeholder_text(Some(&tr("Ex: Produção (deixe vazio para raiz)")));
    folder_entry.set_text(&seed.folder_path);
    form.append(&labeled_row(&tr("Pasta:"), &folder_entry));

    // 4. Host / IP
    let host_entry = gtk4::Entry::new();
    host_entry.set_placeholder_text(Some(&tr("Ex: 10.0.0.15 ou srv.empresa.com")));
    host_entry.set_text(&seed.host);
    form.append(&labeled_row(&tr("Host / IP:"), &host_entry));

    // 5. Port
    let port_spin = gtk4::SpinButton::with_range(1.0, 65535.0, 1.0);
    port_spin.set_value(if seed.port == 0 { 22.0 } else { seed.port as f64 });
    form.append(&labeled_row(&tr("Porta:"), &port_spin));

    // 6. Username
    let user_entry = gtk4::Entry::new();
    user_entry.set_placeholder_text(Some(&tr("Ex: root ou ubuntu")));
    user_entry.set_text(&seed.user);
    form.append(&labeled_row(&tr("Usuário SSH:"), &user_entry));

    // 7. Auth Type (key, password)
    let auth_list = gtk4::StringList::new(&["key", "password"]);
    let auth_dd = gtk4::DropDown::new(Some(auth_list), gtk4::Expression::NONE);
    auth_dd.set_selected(if seed.auth_type == "password" { 1 } else { 0 });
    form.append(&labeled_row(&tr("Autenticação:"), &auth_dd));

    // 8. Key Path or Password
    let auth_val_entry = gtk4::PasswordEntry::new();
    auth_val_entry.set_show_peek_icon(true);
    auth_val_entry.set_text(&resolve_session_secret(&seed));
    form.append(&labeled_row(&tr("Chave SSH / Senha:"), &auth_val_entry));

    // 9. Post-login Command
    let post_cmd_entry = gtk4::Entry::new();
    post_cmd_entry.set_placeholder_text(Some("Ex: tmux attach || tmux new"));
    post_cmd_entry.set_text(&seed.post_login_command);
    form.append(&labeled_row(&tr("Comando Pós-Login:"), &post_cmd_entry));

    // 10. X11 Forwarding Switch
    let x11_sw = gtk4::Switch::new();
    x11_sw.set_valign(gtk4::Align::Center);
    x11_sw.set_active(seed.x11_forwarding);
    form.append(&labeled_row(&tr("Encaminhamento X11 (-X):"), &x11_sw));

    // 11. Production Environment Switch (`is_production`)
    let prod_sw = gtk4::Switch::new();
    prod_sw.set_valign(gtk4::Align::Center);
    prod_sw.set_active(seed.is_production);
    form.append(&labeled_row(
        &tr("Ambiente de Produção (ProductionGuard):"),
        &prod_sw,
    ));

    let err_lbl = gtk4::Label::new(None);
    err_lbl.add_css_class("error");
    err_lbl.set_halign(gtk4::Align::Start);
    form.append(&err_lbl);

    let btn_save = gtk4::Button::with_label(&if is_edit {
        tr("Salvar Alterações")
    } else {
        tr("Criar Sessão")
    });
    btn_save.add_css_class("suggested-action");

    let dlg_clone = dlg.clone();
    btn_save.connect_clicked(move |_| {
        let name = name_entry.text().trim().to_string();
        if name.is_empty() {
            err_lbl.set_text(&tr("O nome da sessão não pode estar vazio."));
            return;
        }
        let session_type = match proto_dd.selected() {
            1 => "local",
            2 => "telnet",
            3 => "serial",
            _ => "ssh",
        }
        .to_string();

        let host = host_entry.text().trim().to_string();
        let port = port_spin.value() as u16;
        let user = user_entry.text().trim().to_string();

        if session_type == "ssh" || session_type == "telnet" {
            if !is_valid_host(&host) {
                err_lbl.set_text(&tr("Host ou endereço IP inválido."));
                return;
            }
            if !is_valid_port(port as u32) {
                err_lbl.set_text(&tr("Porta fora do intervalo permitido (1..65535)."));
                return;
            }
            if !user.is_empty() && !is_valid_username(&user) {
                err_lbl.set_text(&tr("Nome de usuário SSH contém caracteres inválidos."));
                return;
            }
        }

        let post_cmd = post_cmd_entry.text().trim().to_string();
        let mut item = SessionItem::default();
        item.name = name.clone();
        item.session_type = session_type;
        item.folder_path = folder_entry.text().trim().to_string();
        item.host = host;
        item.port = port;
        item.user = user;
        item.auth_type = if auth_dd.selected() == 1 {
            "password".to_string()
        } else {
            "key".to_string()
        };
        item.auth_value = auth_val_entry.text().to_string();
        item.post_login_command_enabled = !post_cmd.is_empty();
        item.post_login_command = post_cmd;
        item.x11_forwarding = x11_sw.is_active();
        item.is_production = prod_sw.is_active();

        if item.auth_type == "password" && !item.auth_value.is_empty() {
            let mgr = KeyringManager::new(SecretToolBackend, "onyxsh");
            let stored = mgr.store_or_encrypt(&item.name, &item.auth_value);
            if !stored.is_empty() {
                item.auth_value = stored;
            }
        }

        {
            let mut st = storage_rc.borrow_mut();
            upsert_session_in_storage(&mut st, orig_name.as_deref(), item);
            let sessions_path = platform::config_dir().join("sessions.json");
            let _ = st.save_to_path(&sessions_path);
        }
        on_saved();
        status_lbl.set_text(&tr_fmt(
            "Sessão '{name}' salva em ~/.config/onyxsh/sessions.json.",
            &[("name", &name)],
        ));
        dlg_clone.close();
    });

    form.append(&btn_save);
    scroll.set_child(Some(&form));
    vbox.append(&scroll);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

fn labeled_row(label_text: &str, widget: &impl IsA<gtk4::Widget>) -> gtk4::Box {
    let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let lbl = gtk4::Label::new(Some(label_text));
    lbl.set_halign(gtk4::Align::Start);
    lbl.set_hexpand(true);
    widget.set_hexpand(true);
    row.append(&lbl);
    row.append(widget);
    row
}

fn show_uri_import_dialog(
    parent: &adw::ApplicationWindow,
    storage_rc: Rc<RefCell<SessionsStorageData>>,
    on_saved: Rc<dyn Fn()>,
    status_lbl: gtk4::Label,
    on_spawn: Rc<dyn Fn(&SessionItem, bool)>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Conectar por URI"))
        .default_width(420)
        .default_height(210)
        .build();
    dlg.add_css_class("onyxsh-dialog");

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    let header = adw::HeaderBar::new();
    vbox.append(&header);

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    body.set_margin_start(14);
    body.set_margin_end(14);
    body.set_margin_bottom(14);

    let uri_entry = gtk4::Entry::new();
    uri_entry.set_placeholder_text(Some("ssh://user@host:22 | serial:///dev/ttyUSB0?baud=115200"));
    body.append(&labeled_row(&tr("URI:"), &uri_entry));

    let save_chk = gtk4::CheckButton::with_label(&tr("Salvar sessão na barra lateral"));
    save_chk.set_active(true);
    body.append(&save_chk);

    let btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_row.set_halign(gtk4::Align::End);
    let btn_connect = gtk4::Button::with_label(&tr("Connect"));
    btn_connect.add_css_class("suggested-action");
    btn_row.append(&btn_connect);
    body.append(&btn_row);

    let dlg_c = dlg.clone();
    btn_connect.connect_clicked(move |_| {
        let raw = uri_entry.text().trim().to_string();
        match parse_connection_uri(&raw) {
            Ok(item) => {
                if save_chk.is_active() {
                    let mut st = storage_rc.borrow_mut();
                    upsert_session_in_storage(&mut st, None, item.clone());
                    let sessions_path = platform::config_dir().join("sessions.json");
                    let _ = st.save_to_path(&sessions_path);
                    drop(st);
                    on_saved();
                }
                on_spawn(&item, false);
                dlg_c.close();
            }
            Err(err) => {
                status_lbl.set_text(&err);
            }
        }
    });

    vbox.append(&body);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

fn show_rename_folder_dialog(
    parent: &adw::ApplicationWindow,
    old_path: String,
    storage_rc: Rc<RefCell<SessionsStorageData>>,
    on_saved: Rc<dyn Fn()>,
    status_lbl: gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Rename"))
        .default_width(360)
        .default_height(170)
        .build();
    dlg.add_css_class("onyxsh-dialog");

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    body.set_margin_start(14);
    body.set_margin_end(14);
    body.set_margin_bottom(14);

    let leaf = old_path.rsplit('/').next().unwrap_or(&old_path);
    let name_entry = gtk4::Entry::new();
    name_entry.set_text(leaf);
    body.append(&labeled_row(&tr("Nome:"), &name_entry));

    let btn_save = gtk4::Button::with_label(&tr("Save"));
    btn_save.add_css_class("suggested-action");
    body.append(&btn_save);

    let dlg_c = dlg.clone();
    btn_save.connect_clicked(move |_| {
        let new_name = name_entry.text().trim().to_string();
        let mut st = storage_rc.borrow_mut();
        if let Some(updated) = rename_folder_in_storage(&mut st, &old_path, &new_name) {
            let sessions_path = platform::config_dir().join("sessions.json");
            let _ = st.save_to_path(&sessions_path);
            drop(st);
            on_saved();
            status_lbl.set_text(&updated);
            dlg_c.close();
        }
    });

    vbox.append(&body);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

fn show_move_session_dialog(
    parent: &adw::ApplicationWindow,
    session_name: String,
    storage_rc: Rc<RefCell<SessionsStorageData>>,
    on_saved: Rc<dyn Fn()>,
    status_lbl: gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Mover para Pasta"))
        .default_width(360)
        .default_height(180)
        .build();
    dlg.add_css_class("onyxsh-dialog");

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    body.set_margin_start(14);
    body.set_margin_end(14);
    body.set_margin_bottom(14);

    let folder_entry = gtk4::Entry::new();
    folder_entry.set_placeholder_text(Some("Ex: Produção/Web"));
    body.append(&labeled_row(&tr("Pasta:"), &folder_entry));

    let btn_move = gtk4::Button::with_label(&tr("Save"));
    btn_move.add_css_class("suggested-action");
    body.append(&btn_move);

    let dlg_c = dlg.clone();
    btn_move.connect_clicked(move |_| {
        let target = folder_entry.text().to_string();
        let mut st = storage_rc.borrow_mut();
        if move_session_to_folder(&mut st, &session_name, &target) {
            let sessions_path = platform::config_dir().join("sessions.json");
            let _ = st.save_to_path(&sessions_path);
            drop(st);
            on_saved();
            status_lbl.set_text(&session_name);
            dlg_c.close();
        }
    });

    vbox.append(&body);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

fn show_save_named_layout_dialog(
    parent: &adw::ApplicationWindow,
    tab_view: &adw::TabView,
    status_lbl: gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Salvar Layout Atual de Abas"))
        .default_width(360)
        .default_height(170)
        .build();
    dlg.add_css_class("onyxsh-dialog");

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    body.set_margin_start(14);
    body.set_margin_end(14);
    body.set_margin_bottom(14);

    let name_entry = gtk4::Entry::new();
    name_entry.set_placeholder_text(Some("Ex: prod-cluster"));
    body.append(&labeled_row(&tr("Nome:"), &name_entry));

    let btn_save = gtk4::Button::with_label(&tr("Save"));
    btn_save.add_css_class("suggested-action");
    body.append(&btn_save);

    let dlg_c = dlg.clone();
    let tv = tab_view.clone();
    btn_save.connect_clicked(move |_| {
        let name = name_entry.text().trim().to_string();
        let win_state = capture_window_session_state(&tv, true, false);
        let state_path = platform::config_dir().join("state.json");
        let _ = win_state.save_to_path(&state_path);
        if !name.is_empty() {
            if let Ok(path) = save_named_layout(&name, &win_state) {
                status_lbl.set_text(&path.display().to_string());
            }
        }
        dlg_c.close();
    });

    vbox.append(&body);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Builds the complete Phase 11 Session Sidebar (`toolbar` + `tree` + `sidebar_search_entry`).
/// Returns `(sidebar_root_widget, refresh_closure)`.
pub fn build_session_sidebar(
    parent_window: &adw::ApplicationWindow,
    tab_view: &adw::TabView,
    status_lbl: &gtk4::Label,
    on_spawn_session: Rc<dyn Fn(&SessionItem, bool)>,
) -> (gtk4::Box, Rc<dyn Fn()>) {
    let sidebar_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    sidebar_box.set_width_request(275);
    sidebar_box.set_margin_start(8);
    sidebar_box.set_margin_end(8);
    sidebar_box.set_margin_top(6);
    sidebar_box.set_margin_bottom(6);

    let sessions_path = platform::config_dir().join("sessions.json");
    let storage_rc = Rc::new(RefCell::new(
        SessionsStorageData::load_from_path(&sessions_path).unwrap_or_default(),
    ));
    let selected_session: Rc<RefCell<Option<SessionItem>>> = Rc::new(RefCell::new(None));

    // ── 1. Top 5-Button CRUD Toolbar (`window_ui.py` L707-L740) ──────────────
    let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    toolbar.set_halign(gtk4::Align::Center);
    toolbar.add_css_class("toolbar");

    let btn_add_session = gtk4::Button::from_icon_name("list-add-symbolic");
    btn_add_session.add_css_class("flat");
    btn_add_session.set_tooltip_text(Some(&tr("Add Session")));

    let btn_add_folder = gtk4::Button::from_icon_name("folder-new-symbolic");
    btn_add_folder.add_css_class("flat");
    btn_add_folder.set_tooltip_text(Some(&tr("Add Folder")));

    let btn_edit_selected = gtk4::Button::from_icon_name("document-edit-symbolic");
    btn_edit_selected.add_css_class("flat");
    btn_edit_selected.set_tooltip_text(Some(&tr("Editar Sessão Selecionada")));

    let btn_save_layout = gtk4::Button::from_icon_name("document-save-symbolic");
    btn_save_layout.add_css_class("flat");
    btn_save_layout.set_tooltip_text(Some(&tr("Salvar Layout Atual de Abas")));

    let btn_remove_selected = gtk4::Button::from_icon_name("user-trash-symbolic");
    btn_remove_selected.add_css_class("flat");
    btn_remove_selected.add_css_class("destructive-action");
    btn_remove_selected.set_tooltip_text(Some(&tr("Remover Sessão Selecionada")));

    let btn_more = gtk4::MenuButton::new();
    btn_more.set_icon_name("view-more-symbolic");
    btn_more.add_css_class("flat");
    btn_more.set_tooltip_text(Some(&tr("Importar Sessões do SecureCRT")));

    let more_menu = gio::Menu::new();
    more_menu.append(Some(&tr("Conectar por URI")), Some("sidebar.connect-uri"));
    more_menu.append(
        Some(&tr("Importar Sessões do SecureCRT")),
        Some("sidebar.import-securecrt"),
    );
    more_menu.append(
        Some(&tr("Salvar Layout Atual de Abas")),
        Some("sidebar.save-named-layout"),
    );
    btn_more.set_menu_model(Some(&more_menu));

    toolbar.append(&btn_add_session);
    toolbar.append(&btn_add_folder);
    toolbar.append(&btn_edit_selected);
    toolbar.append(&btn_save_layout);
    toolbar.append(&btn_remove_selected);
    toolbar.append(&btn_more);
    sidebar_box.append(&toolbar);

    // Selected indicator label
    let selected_lbl = gtk4::Label::new(Some(&tr("Nenhuma sessão selecionada")));
    selected_lbl.add_css_class("caption");
    selected_lbl.add_css_class("dim-label");
    selected_lbl.set_halign(gtk4::Align::Start);
    sidebar_box.append(&selected_lbl);

    // ── 2. Scrolled Hierarchical Session & Folder Tree ────────────────────────
    let tree_scroll = gtk4::ScrolledWindow::new();
    tree_scroll.set_vexpand(true);
    let tree_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    tree_scroll.set_child(Some(&tree_vbox));
    sidebar_box.append(&tree_scroll);

    // ── 3. Bottom Search Entry (`sidebar_search_entry`) ───────────────────────
    let sidebar_search_entry = gtk4::SearchEntry::new();
    sidebar_search_entry.set_placeholder_text(Some(&tr("Buscar sessões ou hosts...")));
    sidebar_box.append(&sidebar_search_entry);

    let rebuild_ref: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    // Tree rebuild closure
    let rebuild_tree: Rc<dyn Fn()> = {
        let tree_vbox = tree_vbox.clone();
        let storage_rc = Rc::clone(&storage_rc);
        let search_entry = sidebar_search_entry.clone();
        let selected_rc = Rc::clone(&selected_session);
        let selected_lbl = selected_lbl.clone();
        let on_spawn = Rc::clone(&on_spawn_session);
        let parent_win = parent_window.clone();
        let status_ref = status_lbl.clone();
        let r_ref = Rc::clone(&rebuild_ref);

        Rc::new(move || {
            while let Some(child) = tree_vbox.first_child() {
                tree_vbox.remove(&child);
            }

            let sessions_path = platform::config_dir().join("sessions.json");
            if let Ok(fresh) = SessionsStorageData::load_from_path(&sessions_path) {
                *storage_rc.borrow_mut() = fresh;
            }
            let ssh_items = load_ssh_config_sessions();
            let query = search_entry.text().to_string();
            let groups = filter_and_group_sessions(&storage_rc.borrow(), &ssh_items, &query);

            for group in groups {
                let header_text = format!("{} ({})", group.display_name, group.sessions.len());
                let expander = gtk4::Expander::new(Some(&header_text));
                expander.set_expanded(group.expanded);

                if !group.is_readonly_ssh_config && !group.folder_path.is_empty() {
                    let f_menu = gio::Menu::new();
                    f_menu.append(Some(&tr("Rename")), Some("folder.rename"));
                    f_menu.append(Some(&tr("Delete")), Some("folder.delete"));
                    let f_pop = gtk4::PopoverMenu::from_model(Some(&f_menu));
                    f_pop.set_parent(&expander);
                    f_pop.set_has_arrow(false);

                    let f_group = gio::SimpleActionGroup::new();
                    {
                        let pw = parent_win.clone();
                        let fp = group.folder_path.clone();
                        let st_c = Rc::clone(&storage_rc);
                        let rr = Rc::clone(&r_ref);
                        let sl = status_ref.clone();
                        let act = gio::SimpleAction::new("rename", None);
                        act.connect_activate(move |_, _| {
                            let rr_cb = Rc::clone(&rr);
                            show_rename_folder_dialog(
                                &pw,
                                fp.clone(),
                                Rc::clone(&st_c),
                                Rc::new(move || {
                                    if let Some(ref f) = *rr_cb.borrow() {
                                        f();
                                    }
                                }),
                                sl.clone(),
                            );
                        });
                        f_group.add_action(&act);
                    }
                    {
                        let fp = group.folder_path.clone();
                        let st_c = Rc::clone(&storage_rc);
                        let rr = Rc::clone(&r_ref);
                        let sl = status_ref.clone();
                        let act = gio::SimpleAction::new("delete", None);
                        act.connect_activate(move |_, _| {
                            let mut st = st_c.borrow_mut();
                            if remove_folder_from_storage(&mut st, &fp, true) {
                                let sessions_path = platform::config_dir().join("sessions.json");
                                let _ = st.save_to_path(&sessions_path);
                                drop(st);
                                if let Some(ref f) = *rr.borrow() {
                                    f();
                                }
                                sl.set_text(&fp);
                            }
                        });
                        f_group.add_action(&act);
                    }
                    expander.insert_action_group("folder", Some(&f_group));

                    let click = gtk4::GestureClick::builder().button(3).build();
                    let pop_c = f_pop.clone();
                    click.connect_pressed(move |_, _, x, y| {
                        pop_c.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(
                            x as i32, y as i32, 1, 1,
                        )));
                        pop_c.popup();
                    });
                    expander.add_controller(click);
                }

                let group_list = gtk4::ListBox::new();
                group_list.set_selection_mode(gtk4::SelectionMode::None);
                group_list.add_css_class("boxed-list");
                group_list.set_margin_top(4);
                group_list.set_margin_bottom(4);

                for sess in group.sessions {
                    let row_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                    row_vbox.set_margin_start(8);
                    row_vbox.set_margin_end(8);
                    row_vbox.set_margin_top(6);
                    row_vbox.set_margin_bottom(6);

                    let top_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                    let icon = gtk4::Image::from_icon_name(if sess.session_type == "local" {
                        "utilities-terminal-symbolic"
                    } else {
                        "network-server-symbolic"
                    });
                    top_hbox.append(&icon);

                    let name_btn = gtk4::Button::with_label(&sess.name);
                    name_btn.add_css_class("flat");
                    name_btn.set_hexpand(true);
                    name_btn.set_halign(gtk4::Align::Start);
                    name_btn.set_tooltip_text(Some(&tr(
                        "Clique para conectar na nova aba (e selecionar esta sessão)",
                    )));
                    top_hbox.append(&name_btn);

                    if sess.is_production {
                        let prod_badge = gtk4::Label::new(Some("PROD"));
                        prod_badge.add_css_class("error");
                        prod_badge.add_css_class("caption");
                        prod_badge.set_tooltip_text(Some(&tr(
                            "Ambiente de Produção — Protegido pelo ProductionGuard",
                        )));
                        top_hbox.append(&prod_badge);
                    }

                    // Split connect button
                    let split_btn = gtk4::Button::from_icon_name("view-dual-symbolic");
                    split_btn.add_css_class("flat");
                    split_btn.set_tooltip_text(Some(&tr("Conectar em Painel Dividido (Split)")));
                    top_hbox.append(&split_btn);

                    row_vbox.append(&top_hbox);

                    if !sess.host.is_empty() {
                        let sub_text = if sess.user.is_empty() {
                            format!("{}:{}", sess.host, sess.port)
                        } else {
                            format!("{}@{}:{}", sess.user, sess.host, sess.port)
                        };
                        let sub_lbl = gtk4::Label::new(Some(&sub_text));
                        sub_lbl.add_css_class("caption");
                        sub_lbl.add_css_class("dim-label");
                        sub_lbl.set_halign(gtk4::Align::Start);
                        sub_lbl.set_margin_start(24);
                        row_vbox.append(&sub_lbl);
                    }

                    {
                        let sess_clone = sess.clone();
                        let sel_rc = Rc::clone(&selected_rc);
                        let sel_lbl = selected_lbl.clone();
                        let spawn_cb = Rc::clone(&on_spawn);
                        name_btn.connect_clicked(move |_| {
                            sel_lbl.set_text(&tr_fmt(
                                "Selecionado: {name}",
                                &[("name", &sess_clone.name)],
                            ));
                            *sel_rc.borrow_mut() = Some(sess_clone.clone());
                            spawn_cb(&sess_clone, false);
                        });
                    }
                    {
                        let sess_clone = sess.clone();
                        let sel_rc = Rc::clone(&selected_rc);
                        let sel_lbl = selected_lbl.clone();
                        let spawn_cb = Rc::clone(&on_spawn);
                        split_btn.connect_clicked(move |_| {
                            sel_lbl.set_text(&tr_fmt(
                                "Selecionado: {name}",
                                &[("name", &sess_clone.name)],
                            ));
                            *sel_rc.borrow_mut() = Some(sess_clone.clone());
                            spawn_cb(&sess_clone, true);
                        });
                    }

                    group_list.append(&row_vbox);

                    // Inline right-click context menu per session row
                    let s_menu = gio::Menu::new();
                    s_menu.append(Some(&tr("Connect")), Some("sess.connect"));
                    s_menu.append(Some(&tr("Split")), Some("sess.split"));
                    if !group.is_readonly_ssh_config {
                        s_menu.append(Some(&tr("Edit")), Some("sess.edit"));
                        s_menu.append(Some(&tr("Duplicate")), Some("sess.clone"));
                        s_menu.append(Some(&tr("Mover para Pasta")), Some("sess.move"));
                        s_menu.append(Some(&tr("Delete")), Some("sess.delete"));
                    } else {
                        s_menu.append(Some(&tr("Hide")), Some("sess.hide-ssh"));
                    }
                    let s_pop = gtk4::PopoverMenu::from_model(Some(&s_menu));
                    s_pop.set_parent(&row_vbox);
                    s_pop.set_has_arrow(false);

                    let s_group = gio::SimpleActionGroup::new();
                    {
                        let sc = sess.clone();
                        let sp = Rc::clone(&on_spawn);
                        let act = gio::SimpleAction::new("connect", None);
                        act.connect_activate(move |_, _| sp(&sc, false));
                        s_group.add_action(&act);
                    }
                    {
                        let sc = sess.clone();
                        let sp = Rc::clone(&on_spawn);
                        let act = gio::SimpleAction::new("split", None);
                        act.connect_activate(move |_, _| sp(&sc, true));
                        s_group.add_action(&act);
                    }
                    {
                        let sc = sess.clone();
                        let pw = parent_win.clone();
                        let st_c = Rc::clone(&storage_rc);
                        let rr = Rc::clone(&r_ref);
                        let sl = status_ref.clone();
                        let act = gio::SimpleAction::new("edit", None);
                        act.connect_activate(move |_, _| {
                            let rr_cb = Rc::clone(&rr);
                            show_session_edit_dialog(
                                &pw,
                                Some(sc.clone()),
                                Rc::clone(&st_c),
                                Rc::new(move || {
                                    if let Some(ref f) = *rr_cb.borrow() {
                                        f();
                                    }
                                }),
                                sl.clone(),
                            );
                        });
                        s_group.add_action(&act);
                    }
                    {
                        let sname = sess.name.clone();
                        let st_c = Rc::clone(&storage_rc);
                        let rr = Rc::clone(&r_ref);
                        let sl = status_ref.clone();
                        let act = gio::SimpleAction::new("clone", None);
                        act.connect_activate(move |_, _| {
                            let mut st = st_c.borrow_mut();
                            if let Some(cloned) = clone_session_in_storage(&mut st, &sname) {
                                let sessions_path = platform::config_dir().join("sessions.json");
                                let _ = st.save_to_path(&sessions_path);
                                drop(st);
                                if let Some(ref f) = *rr.borrow() {
                                    f();
                                }
                                sl.set_text(&cloned.name);
                            }
                        });
                        s_group.add_action(&act);
                    }
                    {
                        let sname = sess.name.clone();
                        let pw = parent_win.clone();
                        let st_c = Rc::clone(&storage_rc);
                        let rr = Rc::clone(&r_ref);
                        let sl = status_ref.clone();
                        let act = gio::SimpleAction::new("move", None);
                        act.connect_activate(move |_, _| {
                            let rr_cb = Rc::clone(&rr);
                            show_move_session_dialog(
                                &pw,
                                sname.clone(),
                                Rc::clone(&st_c),
                                Rc::new(move || {
                                    if let Some(ref f) = *rr_cb.borrow() {
                                        f();
                                    }
                                }),
                                sl.clone(),
                            );
                        });
                        s_group.add_action(&act);
                    }
                    {
                        let sname = sess.name.clone();
                        let st_c = Rc::clone(&storage_rc);
                        let rr = Rc::clone(&r_ref);
                        let sl = status_ref.clone();
                        let act = gio::SimpleAction::new("delete", None);
                        act.connect_activate(move |_, _| {
                            let mut st = st_c.borrow_mut();
                            if remove_session_from_storage(&mut st, &sname) {
                                let sessions_path = platform::config_dir().join("sessions.json");
                                let _ = st.save_to_path(&sessions_path);
                                drop(st);
                                if let Some(ref f) = *rr.borrow() {
                                    f();
                                }
                                sl.set_text(&tr_fmt(
                                    "Sessão '{name}' removida.",
                                    &[("name", &sname)],
                                ));
                            }
                        });
                        s_group.add_action(&act);
                    }
                    {
                        let sname = sess.name.clone();
                        let rr = Rc::clone(&r_ref);
                        let sl = status_ref.clone();
                        let act = gio::SimpleAction::new("hide-ssh", None);
                        act.connect_activate(move |_, _| {
                            if hide_ssh_config_host(&sname).is_ok() {
                                if let Some(ref f) = *rr.borrow() {
                                    f();
                                }
                                sl.set_text(&sname);
                            }
                        });
                        s_group.add_action(&act);
                    }
                    row_vbox.insert_action_group("sess", Some(&s_group));

                    let click = gtk4::GestureClick::builder().button(3).build();
                    let pop_c = s_pop.clone();
                    click.connect_pressed(move |_, _, x, y| {
                        pop_c.set_pointing_to(Some(&gtk4::gdk::Rectangle::new(
                            x as i32, y as i32, 1, 1,
                        )));
                        pop_c.popup();
                    });
                    row_vbox.add_controller(click);
                }

                expander.set_child(Some(&group_list));
                tree_vbox.append(&expander);
            }
        })
    };

    *rebuild_ref.borrow_mut() = Some(Rc::clone(&rebuild_tree));
    rebuild_tree();

    // Wire sidebar toolbar overflow actions (URI, SecureCRT, Named Layout)
    {
        let sb_group = gio::SimpleActionGroup::new();
        {
            let win = parent_window.clone();
            let st_rc = Rc::clone(&storage_rc);
            let rt = Rc::clone(&rebuild_tree);
            let status = status_lbl.clone();
            let spawn = Rc::clone(&on_spawn_session);
            let act = gio::SimpleAction::new("connect-uri", None);
            act.connect_activate(move |_, _| {
                show_uri_import_dialog(
                    &win,
                    Rc::clone(&st_rc),
                    Rc::clone(&rt),
                    status.clone(),
                    Rc::clone(&spawn),
                );
            });
            sb_group.add_action(&act);
        }
        {
            let win = parent_window.clone();
            let st_rc = Rc::clone(&storage_rc);
            let rt = Rc::clone(&rebuild_tree);
            let status = status_lbl.clone();
            let act = gio::SimpleAction::new("import-securecrt", None);
            act.connect_activate(move |_, _| {
                let fd = gtk4::FileDialog::builder()
                    .title(&tr("Importar Sessões do SecureCRT"))
                    .build();
                let st_c = Rc::clone(&st_rc);
                let rt_c = Rc::clone(&rt);
                let sl = status.clone();
                fd.select_folder(Some(&win), gio::Cancellable::NONE, move |res| {
                    if let Ok(folder) = res {
                        if let Some(path) = folder.path() {
                            let mut st = st_c.borrow_mut();
                            match import_securecrt_directory(&path, &mut st) {
                                Ok(report) => {
                                    let sessions_path = platform::config_dir().join("sessions.json");
                                    let _ = st.save_to_path(&sessions_path);
                                    drop(st);
                                    rt_c();
                                    sl.set_text(&format!(
                                        "SecureCRT: {} session(s), {} folder(s)",
                                        report.imported_sessions, report.created_folders
                                    ));
                                }
                                Err(err) => sl.set_text(&err),
                            }
                        }
                    }
                });
            });
            sb_group.add_action(&act);
        }
        {
            let win = parent_window.clone();
            let tv = tab_view.clone();
            let status = status_lbl.clone();
            let act = gio::SimpleAction::new("save-named-layout", None);
            act.connect_activate(move |_, _| {
                show_save_named_layout_dialog(&win, &tv, status.clone());
            });
            sb_group.add_action(&act);
        }
        sidebar_box.insert_action_group("sidebar", Some(&sb_group));
    }

    // Wire search entry to live tree filter
    {
        let rt = Rc::clone(&rebuild_tree);
        sidebar_search_entry.connect_search_changed(move |_| rt());
    }

    // Wire 1. Add Session button
    {
        let win = parent_window.clone();
        let st_rc = Rc::clone(&storage_rc);
        let rt = Rc::clone(&rebuild_tree);
        let status = status_lbl.clone();
        btn_add_session.connect_clicked(move |_| {
            show_session_edit_dialog(
                &win,
                None,
                Rc::clone(&st_rc),
                Rc::clone(&rt),
                status.clone(),
            );
        });
    }

    // Wire 2. Add Folder button
    {
        let win = parent_window.clone();
        let st_rc = Rc::clone(&storage_rc);
        let rt = Rc::clone(&rebuild_tree);
        let status = status_lbl.clone();
        btn_add_folder.connect_clicked(move |_| {
            show_folder_create_dialog(&win, Rc::clone(&st_rc), Rc::clone(&rt), status.clone());
        });
    }

    // Wire 3. Edit Selected button
    {
        let win = parent_window.clone();
        let st_rc = Rc::clone(&storage_rc);
        let sel_rc = Rc::clone(&selected_session);
        let rt = Rc::clone(&rebuild_tree);
        let status = status_lbl.clone();
        btn_edit_selected.connect_clicked(move |_| {
            let current = sel_rc.borrow().clone();
            if let Some(sess) = current {
                show_session_edit_dialog(
                    &win,
                    Some(sess),
                    Rc::clone(&st_rc),
                    Rc::clone(&rt),
                    status.clone(),
                );
            } else {
                status.set_text(&tr("Selecione uma sessão na lista primeiro para editar."));
            }
        });
    }

    // Wire 4. Save Current Layout button (`state.json`)
    {
        let tv = tab_view.clone();
        let status = status_lbl.clone();
        btn_save_layout.connect_clicked(move |_| {
            let win_state = capture_window_session_state(&tv, true, false);
            let n_pages = win_state.tabs.len();
            let state_path = platform::config_dir().join("state.json");
            if win_state.save_to_path(&state_path).is_ok() {
                status.set_text(&tr_fmt(
                    "Layout de {n_pages} aba(s) salvo em {path}",
                    &[
                        ("n_pages", &n_pages.to_string()),
                        ("path", &state_path.display().to_string()),
                    ],
                ));
            }
        });
    }

    // Wire 5. Remove Selected button
    {
        let st_rc = Rc::clone(&storage_rc);
        let sel_rc = Rc::clone(&selected_session);
        let sel_lbl = selected_lbl.clone();
        let rt = Rc::clone(&rebuild_tree);
        let status = status_lbl.clone();
        btn_remove_selected.connect_clicked(move |_| {
            let target_opt = sel_rc.borrow().clone();
            if let Some(sess) = target_opt {
                let mut st = st_rc.borrow_mut();
                if remove_session_from_storage(&mut st, &sess.name) {
                    let sessions_path = platform::config_dir().join("sessions.json");
                    let _ = st.save_to_path(&sessions_path);
                    drop(st);
                    *sel_rc.borrow_mut() = None;
                    sel_lbl.set_text(&tr("Nenhuma sessão selecionada"));
                    rt();
                    status.set_text(&tr_fmt(
                        "Sessão '{name}' removida.",
                        &[("name", &sess.name)],
                    ));
                } else {
                    status.set_text(&tr(
                        "Esta sessão vem do ~/.ssh/config (somente leitura) ou já foi removida.",
                    ));
                }
            } else {
                status.set_text(&tr("Selecione uma sessão primeiro para remover."));
            }
        });
    }

    (sidebar_box, rebuild_tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_and_group_sessions_and_crud() {
        let mut storage = SessionsStorageData::default();
        upsert_folder_in_storage(&mut storage, "Produção");

        let mut s1 = SessionItem::default();
        s1.name = "Web-01".to_string();
        s1.host = "10.0.1.10".to_string();
        s1.user = "ubuntu".to_string();
        s1.folder_path = "Produção".to_string();
        s1.auth_type = "password".to_string();
        s1.auth_value = "my-secret".to_string();
        s1.is_production = true;

        upsert_session_in_storage(&mut storage, None, s1);
        assert_eq!(storage.sessions.len(), 1);
        // Password must be encrypted with SecureCRT V2 prefix `02:`
        assert!(storage.sessions[0].auth_value.starts_with("02:"));

        let mut ssh_host = SessionItem::default();
        ssh_host.name = "bastion-aws".to_string();
        ssh_host.host = "54.210.1.1".to_string();

        // Unfiltered: 2 groups ("Produção" and "OpenSSH (~/.ssh/config)")
        let groups = filter_and_group_sessions(&storage, &[ssh_host.clone()], "");
        assert_eq!(groups.len(), 2);

        // Filtered by "bastion"
        let filtered = filter_and_group_sessions(&storage, &[ssh_host], "bastion");
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].is_readonly_ssh_config);

        assert!(remove_session_from_storage(&mut storage, "Web-01"));
        assert!(storage.sessions.is_empty());
    }

    #[test]
    fn test_session_clone_move_and_folder_rename_delete() {
        let mut storage = SessionsStorageData::default();
        upsert_folder_in_storage(&mut storage, "Infra/DB");
        let mut s = SessionItem::default();
        s.name = "Primary".to_string();
        s.host = "10.0.0.5".to_string();
        s.folder_path = "Infra/DB".to_string();
        upsert_session_in_storage(&mut storage, None, s);

        let cloned = clone_session_in_storage(&mut storage, "Primary").expect("clone session");
        assert_eq!(cloned.name, "Primary (Cópia)");
        assert_eq!(storage.sessions.len(), 2);

        upsert_folder_in_storage(&mut storage, "Staging");
        assert!(move_session_to_folder(&mut storage, "Primary (Cópia)", "Staging"));
        assert_eq!(
            storage
                .sessions
                .iter()
                .find(|s| s.name == "Primary (Cópia)")
                .unwrap()
                .folder_path,
            "Staging"
        );

        let renamed = rename_folder_in_storage(&mut storage, "Infra", "Production").expect("rename");
        assert_eq!(renamed, "Production");
        assert_eq!(
            storage
                .sessions
                .iter()
                .find(|s| s.name == "Primary")
                .unwrap()
                .folder_path,
            "Production/DB"
        );

        assert!(remove_folder_from_storage(&mut storage, "Production", true));
        assert_eq!(
            storage
                .sessions
                .iter()
                .find(|s| s.name == "Primary")
                .unwrap()
                .folder_path,
            ""
        );

        let filtered = filter_ignored_ssh_hosts(
            vec![SessionItem {
                name: "bastion".to_string(),
                ..SessionItem::default()
            }],
            &["bastion".to_string()],
        );
        assert!(filtered.is_empty());
    }
}
