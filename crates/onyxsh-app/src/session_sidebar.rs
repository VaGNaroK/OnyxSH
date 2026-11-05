//! Phase 11 (Etapa 2): Complete Session Panel (`src/onyxsh/sessions/tree.py`, `ui/sidebar_manager.py`,
//! `ui/dialogs/session_edit_dialog.py`, `ui/dialogs/folder_edit_dialog.py`).
//!
//! Provides:
//! - 5-button CRUD Toolbar (Add Session, Add Folder, Edit Selected, Save Layout, Remove Selected)
//! - Hierarchical Folder & Session Tree with `PROD` badges, `user@host:port` subtitles, and `.ssh/config` group
//! - Real-time bottom Search Entry (`sidebar_search_entry`) with automatic folder expansion
//! - Modal Dialogs for Creating/Editing Sessions (with AES-256-CBC V2 password encryption) and Folders

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::crypto::{encrypt_session_password, is_securecrt_v2_password};
use onyxsh_core::platform;
use onyxsh_core::security::{is_valid_host, is_valid_port, is_valid_username};
use onyxsh_core::sessions::{SessionFolder, SessionItem, SessionsStorageData};
use onyxsh_core::ssh_config::SshConfigParser;
use onyxsh_core::window_state::{TabNodeState, WindowSessionState};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

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
            display_name: "Sessões Salvas".to_string(),
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

/// Loads `~/.ssh/config` hosts as `SessionItem`s.
pub fn load_ssh_config_sessions() -> Vec<SessionItem> {
    let ssh_cfg_path = platform::home_dir().join(".ssh/config");
    if !ssh_cfg_path.exists() {
        return Vec::new();
    }
    let mut parser = SshConfigParser::new();
    parser
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
        .collect()
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
        .title("Nova Pasta de Sessões")
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

    let lbl = gtk4::Label::new(Some("Caminho da Pasta (ex: Produção ou Cloud/AWS):"));
    lbl.set_halign(gtk4::Align::Start);
    content.append(&lbl);

    let entry = gtk4::Entry::new();
    entry.set_placeholder_text(Some("Produção/Servidores Web"));
    content.append(&entry);

    let btn_save = gtk4::Button::with_label("Criar Pasta");
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
            status_lbl.set_text(&format!("Pasta '{}' criada com sucesso.", path.trim()));
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
        .title(if is_edit {
            "Editar Sessão — OnyxSH"
        } else {
            "Nova Sessão — OnyxSH"
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
    name_entry.set_placeholder_text(Some("Ex: Produção Web-01"));
    if is_edit {
        name_entry.set_text(&seed.name);
    }
    form.append(&labeled_row("Nome da Sessão:", &name_entry));

    // 2. Protocol Dropdown (ssh, local, telnet, serial)
    let proto_list = gtk4::StringList::new(&["ssh", "local", "telnet", "serial"]);
    let proto_dd = gtk4::DropDown::new(Some(proto_list), gtk4::Expression::NONE);
    proto_dd.set_selected(match seed.session_type.as_str() {
        "local" => 1,
        "telnet" => 2,
        "serial" => 3,
        _ => 0,
    });
    form.append(&labeled_row("Protocolo:", &proto_dd));

    // 3. Folder Path
    let folder_entry = gtk4::Entry::new();
    folder_entry.set_placeholder_text(Some("Ex: Produção (deixe vazio para raiz)"));
    folder_entry.set_text(&seed.folder_path);
    form.append(&labeled_row("Pasta:", &folder_entry));

    // 4. Host / IP
    let host_entry = gtk4::Entry::new();
    host_entry.set_placeholder_text(Some("Ex: 10.0.0.15 ou srv.empresa.com"));
    host_entry.set_text(&seed.host);
    form.append(&labeled_row("Host / IP:", &host_entry));

    // 5. Port
    let port_spin = gtk4::SpinButton::with_range(1.0, 65535.0, 1.0);
    port_spin.set_value(if seed.port == 0 { 22.0 } else { seed.port as f64 });
    form.append(&labeled_row("Porta:", &port_spin));

    // 6. Username
    let user_entry = gtk4::Entry::new();
    user_entry.set_placeholder_text(Some("Ex: root ou ubuntu"));
    user_entry.set_text(&seed.user);
    form.append(&labeled_row("Usuário SSH:", &user_entry));

    // 7. Auth Type (key, password)
    let auth_list = gtk4::StringList::new(&["key", "password"]);
    let auth_dd = gtk4::DropDown::new(Some(auth_list), gtk4::Expression::NONE);
    auth_dd.set_selected(if seed.auth_type == "password" { 1 } else { 0 });
    form.append(&labeled_row("Autenticação:", &auth_dd));

    // 8. Key Path or Password
    let auth_val_entry = gtk4::PasswordEntry::new();
    auth_val_entry.set_show_peek_icon(true);
    auth_val_entry.set_text(&seed.auth_value);
    form.append(&labeled_row("Chave SSH / Senha:", &auth_val_entry));

    // 9. Post-login Command
    let post_cmd_entry = gtk4::Entry::new();
    post_cmd_entry.set_placeholder_text(Some("Ex: tmux attach || tmux new"));
    post_cmd_entry.set_text(&seed.post_login_command);
    form.append(&labeled_row("Comando Pós-Login:", &post_cmd_entry));

    // 10. X11 Forwarding Switch
    let x11_sw = gtk4::Switch::new();
    x11_sw.set_valign(gtk4::Align::Center);
    x11_sw.set_active(seed.x11_forwarding);
    form.append(&labeled_row("Encaminhamento X11 (-X):", &x11_sw));

    // 11. Production Environment Switch (`is_production`)
    let prod_sw = gtk4::Switch::new();
    prod_sw.set_valign(gtk4::Align::Center);
    prod_sw.set_active(seed.is_production);
    form.append(&labeled_row(
        "Ambiente de Produção (ProductionGuard):",
        &prod_sw,
    ));

    let err_lbl = gtk4::Label::new(None);
    err_lbl.add_css_class("error");
    err_lbl.set_halign(gtk4::Align::Start);
    form.append(&err_lbl);

    let btn_save = gtk4::Button::with_label(if is_edit {
        "Salvar Alterações"
    } else {
        "Criar Sessão"
    });
    btn_save.add_css_class("suggested-action");

    let dlg_clone = dlg.clone();
    btn_save.connect_clicked(move |_| {
        let name = name_entry.text().trim().to_string();
        if name.is_empty() {
            err_lbl.set_text("O nome da sessão não pode estar vazio.");
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
                err_lbl.set_text("Host ou endereço IP inválido.");
                return;
            }
            if !is_valid_port(port as u32) {
                err_lbl.set_text("Porta fora do intervalo permitido (1..65535).");
                return;
            }
            if !user.is_empty() && !is_valid_username(&user) {
                err_lbl.set_text("Nome de usuário SSH contém caracteres inválidos.");
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

        {
            let mut st = storage_rc.borrow_mut();
            upsert_session_in_storage(&mut st, orig_name.as_deref(), item);
            let sessions_path = platform::config_dir().join("sessions.json");
            let _ = st.save_to_path(&sessions_path);
        }
        on_saved();
        status_lbl.set_text(&format!("Sessão '{name}' salva em ~/.config/onyxsh/sessions.json."));
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
    btn_add_session.set_tooltip_text(Some("Adicionar Sessão"));

    let btn_add_folder = gtk4::Button::from_icon_name("folder-new-symbolic");
    btn_add_folder.add_css_class("flat");
    btn_add_folder.set_tooltip_text(Some("Adicionar Pasta"));

    let btn_edit_selected = gtk4::Button::from_icon_name("document-edit-symbolic");
    btn_edit_selected.add_css_class("flat");
    btn_edit_selected.set_tooltip_text(Some("Editar Sessão Selecionada"));

    let btn_save_layout = gtk4::Button::from_icon_name("document-save-symbolic");
    btn_save_layout.add_css_class("flat");
    btn_save_layout.set_tooltip_text(Some("Salvar Layout Atual de Abas"));

    let btn_remove_selected = gtk4::Button::from_icon_name("user-trash-symbolic");
    btn_remove_selected.add_css_class("flat");
    btn_remove_selected.add_css_class("destructive-action");
    btn_remove_selected.set_tooltip_text(Some("Remover Sessão Selecionada"));

    toolbar.append(&btn_add_session);
    toolbar.append(&btn_add_folder);
    toolbar.append(&btn_edit_selected);
    toolbar.append(&btn_save_layout);
    toolbar.append(&btn_remove_selected);
    sidebar_box.append(&toolbar);

    // Selected indicator label
    let selected_lbl = gtk4::Label::new(Some("Nenhuma sessão selecionada"));
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
    sidebar_search_entry.set_placeholder_text(Some("Buscar sessões ou hosts..."));
    sidebar_box.append(&sidebar_search_entry);

    // Tree rebuild closure
    let rebuild_tree: Rc<dyn Fn()> = {
        let tree_vbox = tree_vbox.clone();
        let storage_rc = Rc::clone(&storage_rc);
        let search_entry = sidebar_search_entry.clone();
        let selected_rc = Rc::clone(&selected_session);
        let selected_lbl = selected_lbl.clone();
        let on_spawn = Rc::clone(&on_spawn_session);

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
                    name_btn.set_tooltip_text(Some(
                        "Clique para conectar na nova aba (e selecionar esta sessão)",
                    ));
                    top_hbox.append(&name_btn);

                    if sess.is_production {
                        let prod_badge = gtk4::Label::new(Some("PROD"));
                        prod_badge.add_css_class("error");
                        prod_badge.add_css_class("caption");
                        prod_badge.set_tooltip_text(Some(
                            "Ambiente de Produção — Protegido pelo ProductionGuard",
                        ));
                        top_hbox.append(&prod_badge);
                    }

                    // Split connect button
                    let split_btn = gtk4::Button::from_icon_name("view-dual-symbolic");
                    split_btn.add_css_class("flat");
                    split_btn.set_tooltip_text(Some("Conectar em Painel Dividido (Split)"));
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
                            sel_lbl.set_text(&format!("Selecionado: {}", sess_clone.name));
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
                            sel_lbl.set_text(&format!("Selecionado: {}", sess_clone.name));
                            *sel_rc.borrow_mut() = Some(sess_clone.clone());
                            spawn_cb(&sess_clone, true);
                        });
                    }

                    group_list.append(&row_vbox);
                }

                expander.set_child(Some(&group_list));
                tree_vbox.append(&expander);
            }
        })
    };

    rebuild_tree();

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
                status.set_text("Selecione uma sessão na lista primeiro para editar.");
            }
        });
    }

    // Wire 4. Save Current Layout button (`state.json`)
    {
        let tv = tab_view.clone();
        let status = status_lbl.clone();
        btn_save_layout.connect_clicked(move |_| {
            let n_pages = tv.n_pages();
            let mut tabs = Vec::new();
            for i in 0..n_pages {
                let page = tv.nth_page(i);
                tabs.push(TabNodeState::Terminal {
                    session_type: "local".to_string(),
                    title: page.title().to_string(),
                    working_directory: None,
                    session_data: None,
                });
            }
            let win_state = WindowSessionState {
                tabs,
                active_tab_index: 0,
                sidebar_visible: true,
                ai_panel_visible: false,
            };
            let state_path = platform::config_dir().join("state.json");
            if win_state.save_to_path(&state_path).is_ok() {
                status.set_text(&format!(
                    "Layout de {n_pages} aba(s) salvo em {}",
                    state_path.display()
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
                    sel_lbl.set_text("Nenhuma sessão selecionada");
                    rt();
                    status.set_text(&format!("Sessão '{}' removida.", sess.name));
                } else {
                    status.set_text(
                        "Esta sessão vem do ~/.ssh/config (somente leitura) ou já foi removida.",
                    );
                }
            } else {
                status.set_text("Selecione uma sessão primeiro para remover.");
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
}
