//! SSH Tunnel Manager & Port Forwarding UI Dialogs for OnyxSH.
//!
//! Full native port of `src/onyxsh/ui/dialogs/tunnel_manager_dialog.py` and
//! `src/onyxsh/ui/dialogs/tunnel_edit_dialog.py`:
//! - Real-time monitoring of Local (-L), Remote (-R), and Dynamic SOCKS5 (-D) port forwardings.
//! - Search/filtering by name, host, port, or type.
//! - Live status dots with tooltips and quick actions (Copy address, Edit, Delete, Toggle active).
//! - Add/Edit modal dialog with automatic input validation and dynamic mode-dependent field visibility.
//! - Automatic persistence to `~/.config/onyxsh/tunnels.json`.

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform::{self, CommandBuilder};
use onyxsh_core::sessions::{SessionItem, SessionsStorageData};
use onyxsh_terminal::tunnel_manager::{SshTunnel, SshTunnelRegistry};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::rc::Rc;

thread_local! {
    static ACTIVE_TUNNEL_PROCS: RefCell<HashMap<String, Child>> = RefCell::new(HashMap::new());
}

/// Resolves a saved `SessionItem` matching `tunnel.session_name`, falling back to `user@host` parsing.
pub fn resolve_tunnel_target_session(session_name: &str) -> Option<SessionItem> {
    let target = session_name.trim();
    if target.is_empty() {
        return None;
    }
    let sessions_path = platform::config_dir().join("sessions.json");
    if let Ok(storage) = SessionsStorageData::load_from_path(&sessions_path) {
        if let Some(found) = storage
            .sessions
            .into_iter()
            .find(|s| s.is_ssh() && (s.name == target || s.host == target))
        {
            return Some(found);
        }
    }
    for s in crate::session_sidebar::load_ssh_config_sessions() {
        if s.name == target || s.host == target {
            return Some(s);
        }
    }
    if target.contains('@') || target.contains('.') || !target.contains(' ') {
        let mut fallback = SessionItem::default();
        fallback.session_type = "ssh".to_string();
        if let Some((u, h)) = target.split_once('@') {
            fallback.user = u.trim().to_string();
            fallback.host = h.trim().to_string();
        } else {
            fallback.host = target.to_string();
        }
        return Some(fallback);
    }
    None
}

/// Builds the exact command + environment (`sshpass` if password auth, `host-spawn` if Flatpak)
/// for spawning a real SSH tunnel process.
pub fn build_tunnel_spawn_argv(
    tunnel: &SshTunnel,
    session: &SessionItem,
    password: Option<&str>,
    builder: &CommandBuilder,
) -> (Vec<String>, HashMap<String, String>) {
    let mut cmd = tunnel.build_ssh_command(session, "ssh");
    let mut env = HashMap::new();
    if session.uses_password_auth() {
        if let Some(pw) = password.filter(|p| !p.is_empty()) {
            // Strip BatchMode=yes so sshpass can feed password to SSH prompt
            let mut cleaned = Vec::with_capacity(cmd.len());
            let mut i = 0;
            while i < cmd.len() {
                if cmd[i] == "-o" && cmd.get(i + 1).is_some_and(|v| v == "BatchMode=yes") {
                    i += 2;
                    continue;
                }
                cleaned.push(cmd[i].clone());
                i += 1;
            }
            cmd = cleaned;
            cmd.insert(0, "-e".to_string());
            cmd.insert(0, "sshpass".to_string());
            env.insert("SSHPASS".to_string(), pw.to_string());
        }
    }
    let wrapped = builder.wrap_host_command(&cmd, None, if env.is_empty() { None } else { Some(&env) });
    (wrapped, env)
}

/// Spawns a background OpenSSH tunnel process (`ssh -N -L/-R/-D`) and stores its handle.
pub fn start_tunnel_process(tunnel: &SshTunnel) -> Result<(), String> {
    stop_tunnel_process(&tunnel.id);
    let session = resolve_tunnel_target_session(&tunnel.session_name).ok_or_else(|| {
        tr_fmt(
            "Sessão SSH '{name}' não encontrada para iniciar o túnel.",
            &[("name", &tunnel.session_name)],
        )
    })?;
    let pw = if session.uses_password_auth() {
        let s = crate::session_sidebar::resolve_session_secret(&session);
        if s.is_empty() { None } else { Some(s) }
    } else {
        None
    };
    let builder = CommandBuilder::new();
    let (argv, env) = build_tunnel_spawn_argv(tunnel, &session, pw.as_deref(), &builder);
    if argv.is_empty() {
        return Err(tr("Comando SSH vazio para o túnel."));
    }
    let mut proc_cmd = Command::new(&argv[0]);
    proc_cmd
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    for (k, v) in env {
        proc_cmd.env(k, v);
    }
    let child = proc_cmd
        .spawn()
        .map_err(|e| tr_fmt("Falha ao iniciar processo SSH do túnel: {err}", &[("err", &e.to_string())]))?;
    ACTIVE_TUNNEL_PROCS.with(|map| {
        map.borrow_mut().insert(tunnel.id.clone(), child);
    });
    Ok(())
}

/// Stops and reaps the child process for a given tunnel ID, if running.
pub fn stop_tunnel_process(tunnel_id: &str) {
    ACTIVE_TUNNEL_PROCS.with(|map| {
        if let Some(mut child) = map.borrow_mut().remove(tunnel_id) {
            let _ = child.kill();
            let _ = child.wait();
        }
    });
}

/// Stops all active SSH tunnel child processes (called on "Parar Todos" or window close).
pub fn stop_all_tunnel_processes() {
    ACTIVE_TUNNEL_PROCS.with(|map| {
        let mut guard = map.borrow_mut();
        for (_, mut child) in guard.drain() {
            let _ = child.kill();
            let _ = child.wait();
        }
    });
}

/// Polls running tunnel child processes and returns any tunnel IDs that exited early with error.
pub fn check_tunnel_process_exit(tunnel_id: &str) -> Option<Result<(), String>> {
    ACTIVE_TUNNEL_PROCS.with(|map| {
        let mut guard = map.borrow_mut();
        let child = guard.get_mut(tunnel_id)?;
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut exited = guard.remove(tunnel_id).unwrap();
                if status.success() {
                    Some(Ok(()))
                } else {
                    let mut err_buf = String::new();
                    if let Some(mut stderr) = exited.stderr.take() {
                        let _ = stderr.read_to_string(&mut err_buf);
                    }
                    let msg = err_buf.lines().last().unwrap_or("Tunnel exited").trim().to_string();
                    Some(Err(msg))
                }
            }
            _ => None,
        }
    })
}

/// Auto-starts any saved SSH tunnels linked to `session_name` with `auto_start == true`.
/// Returns the names of successfully started tunnels.
pub fn auto_start_tunnels_for_session(session_name: &str) -> Vec<String> {
    let mut reg = load_tunnel_registry();
    let matching: Vec<SshTunnel> = reg
        .all()
        .into_iter()
        .filter(|t| t.auto_start && t.session_name == session_name)
        .collect();
    let mut started = Vec::new();
    let mut changed = false;
    for t in matching {
        match start_tunnel_process(&t) {
            Ok(()) => {
                reg.set_status(&t.id, "active", "");
                started.push(t.name.clone());
                changed = true;
            }
            Err(err) => {
                reg.set_status(&t.id, "error", &err);
                changed = true;
            }
        }
    }
    if changed {
        let _ = reg.save_to_file(&tunnels_file_path());
    }
    started
}

/// Path to the JSON file storing user-configured tunnels.
pub fn tunnels_file_path() -> PathBuf {
    platform::config_dir().join("tunnels.json")
}

/// Helper to load tunnels registry from standard path with default samples if absent.
pub fn load_tunnel_registry() -> SshTunnelRegistry {
    let mut reg = SshTunnelRegistry::new();
    let path = tunnels_file_path();
    let _ = reg.load_from_file(&path);
    if reg.all().is_empty() {
        // Register standard sample tunnels for first-time onboarding
        reg.register(SshTunnel {
            id: "tun_local_postgres".to_string(),
            name: tr("PostgreSQL Remoto"),
            session_name: "Bastion-Production".to_string(),
            tunnel_type: "local".to_string(),
            local_host: "127.0.0.1".to_string(),
            local_port: 5432,
            remote_host: "postgres-internal.db".to_string(),
            remote_port: 5432,
            auto_start: false,
            status: "stopped".to_string(),
            error_message: String::new(),
        });
        reg.register(SshTunnel {
            id: "tun_socks_proxy".to_string(),
            name: tr("SOCKS5 Proxy Corporativo"),
            session_name: "Edge-Router".to_string(),
            tunnel_type: "dynamic".to_string(),
            local_host: "127.0.0.1".to_string(),
            local_port: 1080,
            remote_host: String::new(),
            remote_port: 0,
            auto_start: false,
            status: "stopped".to_string(),
            error_message: String::new(),
        });
        let _ = reg.save_to_file(&path);
    }
    reg
}

/// Presents the full SSH Tunnel Manager Window (`win.show-tunnel-manager`).
pub fn show_tunnel_manager_window(
    parent: &adw::ApplicationWindow,
    status_label: &gtk4::Label,
) {
    let reg_rc = Rc::new(RefCell::new(load_tunnel_registry()));
    let win = adw::Window::builder()
        .transient_for(parent)
        .modal(false)
        .title(&tr("SSH Tunnel Manager"))
        .default_width(720)
        .default_height(580)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();

    // Add Tunnel Button
    let btn_add = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text(&tr("Adicionar Novo Túnel SSH"))
        .build();
    btn_add.add_css_class("suggested-action");

    // Stop All Tunnels Button
    let btn_stop_all = gtk4::Button::builder()
        .icon_name("media-playback-stop-symbolic")
        .tooltip_text(&tr("Stop All Active Tunnels"))
        .build();
    btn_stop_all.add_css_class("flat");

    header.pack_start(&btn_add);
    header.pack_end(&btn_stop_all);
    toolbar_view.add_top_bar(&header);

    // Search Bar
    let search_bar = gtk4::SearchBar::new();
    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text(&tr("Pesquisar túneis por nome, porta ou host..."))
        .hexpand(true)
        .build();
    search_bar.set_child(Some(&search_entry));
    search_bar.set_key_capture_widget(Some(&win));
    search_bar.set_search_mode(true);
    toolbar_view.add_top_bar(&search_bar);

    // Main content
    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();

    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    main_box.set_margin_start(18);
    main_box.set_margin_end(18);
    main_box.set_margin_top(16);
    main_box.set_margin_bottom(18);

    let tunnel_list = gtk4::ListBox::new();
    tunnel_list.set_selection_mode(gtk4::SelectionMode::None);
    tunnel_list.add_css_class("boxed-list");

    let status_page = adw::StatusPage::builder()
        .icon_name("network-workgroup-symbolic")
        .title(&tr("No SSH Tunnels"))
        .description(&tr(
            "Crie um túnel Local (-L), Remoto (-R) ou Dinâmico SOCKS5 (-D) para encaminhar portas via SSH.",
        ))
        .vexpand(true)
        .build();

    main_box.append(&tunnel_list);
    main_box.append(&status_page);
    scrolled.set_child(Some(&main_box));
    toolbar_view.set_content(Some(&scrolled));
    win.set_content(Some(&toolbar_view));

    // Closure to re-populate the list
    let populate_list: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    {
        let reg_clone = Rc::clone(&reg_rc);
        let list_clone = tunnel_list.clone();
        let status_p_clone = status_page.clone();
        let status_lbl_outer = status_label.clone();
        let win_clone = win.clone();
        let query_entry = search_entry.clone();
        let pop_weak = Rc::downgrade(&populate_list);

        let pop_func = Rc::new(move || {
            // Clear existing rows
            list_clone.remove_all();

            let tunnels = reg_clone.borrow().all();
            let query = query_entry.text().to_lowercase();
            let mut visible_count = 0;

            for tunnel in &tunnels {
                let matches_query = query.is_empty()
                    || tunnel.name.to_lowercase().contains(&query)
                    || tunnel.session_name.to_lowercase().contains(&query)
                    || tunnel.local_port.to_string().contains(&query)
                    || tunnel.remote_port.to_string().contains(&query)
                    || tunnel.remote_host.to_lowercase().contains(&query)
                    || tunnel.tunnel_type.to_lowercase().contains(&query);

                if !matches_query {
                    continue;
                }
                visible_count += 1;

                let row = adw::ActionRow::new();
                let type_badge = format!("[{}]", tunnel.tunnel_type.to_uppercase());
                row.set_title(&format!("<b>{}</b>  <small>{}</small>", glib::markup_escape_text(&tunnel.name), type_badge));
                row.set_use_markup(true);

                let mut subtitle = format!("{}  ➔  {}", tunnel.display_source(), tunnel.display_target());
                if !tunnel.session_name.is_empty() {
                    subtitle.push_str(&format!("  ({})", tunnel.session_name));
                }
                row.set_subtitle(&subtitle);

                // Prefix: Status Dot
                let dot = gtk4::Image::from_icon_name("media-record-symbolic");
                dot.set_pixel_size(12);
                match tunnel.status.as_str() {
                    "active" => {
                        dot.add_css_class("success");
                        dot.set_tooltip_text(Some(&tr_fmt(
                            "Ativo (Ouvindo em {src})",
                            &[("src", &tunnel.display_source())],
                        )));
                    }
                    "starting" => {
                        dot.add_css_class("warning");
                        dot.set_tooltip_text(Some(&tr("Connecting...")));
                    }
                    "error" => {
                        dot.add_css_class("error");
                        dot.set_tooltip_text(Some(&tr_fmt(
                            "Erro: {err}",
                            &[("err", &tunnel.error_message)],
                        )));
                    }
                    _ => {
                        dot.add_css_class("dim-label");
                        dot.set_tooltip_text(Some(&tr("Stopped")));
                    }
                }
                row.add_prefix(&dot);

                // Suffix 1: Quick Copy Button
                let btn_copy = gtk4::Button::builder()
                    .icon_name("edit-copy-symbolic")
                    .valign(gtk4::Align::Center)
                    .tooltip_text(&tr_fmt(
                        "Copiar endereço local (localhost:{port})",
                        &[("port", &tunnel.local_port.to_string())],
                    ))
                    .build();
                btn_copy.add_css_class("flat");
                btn_copy.add_css_class("circular");
                let port_copy = tunnel.local_port;
                btn_copy.connect_clicked(move |_| {
                    if let Some(display) = gdk::Display::default() {
                        display.clipboard().set_text(&format!("localhost:{port_copy}"));
                    }
                });
                row.add_suffix(&btn_copy);

                // Suffix 2: Edit Button
                let btn_edit = gtk4::Button::builder()
                    .icon_name("document-edit-symbolic")
                    .valign(gtk4::Align::Center)
                    .tooltip_text(&tr("Editar Configurações do Túnel"))
                    .build();
                btn_edit.add_css_class("flat");
                btn_edit.add_css_class("circular");
                let tid_edit = tunnel.id.clone();
                let reg_edit = Rc::clone(&reg_clone);
                let win_edit = win_clone.clone();
                let pop_weak_edit = pop_weak.clone();
                btn_edit.connect_clicked(move |_| {
                    let existing = reg_edit.borrow().get(&tid_edit).cloned();
                    if let Some(t) = existing {
                        let reg_save = Rc::clone(&reg_edit);
                        let pop_weak_sub = pop_weak_edit.clone();
                        show_tunnel_edit_dialog(
                            &win_edit,
                            Some(&t),
                            move |updated| {
                                reg_save.borrow_mut().register(updated);
                                let _ = reg_save.borrow().save_to_file(&tunnels_file_path());
                                if let Some(pop_rc) = pop_weak_sub.upgrade() {
                                    if let Some(f) = pop_rc.borrow().as_ref() {
                                        f();
                                    }
                                }
                            },
                        );
                    }
                });
                row.add_suffix(&btn_edit);

                // Suffix 3: Delete Button
                let btn_del = gtk4::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .valign(gtk4::Align::Center)
                    .tooltip_text(&tr("Remover Túnel"))
                    .build();
                btn_del.add_css_class("flat");
                btn_del.add_css_class("circular");
                let tid_del = tunnel.id.clone();
                let reg_del = Rc::clone(&reg_clone);
                let pop_weak_del = pop_weak.clone();
                btn_del.connect_clicked(move |_| {
                    reg_del.borrow_mut().unregister(&tid_del);
                    let _ = reg_del.borrow().save_to_file(&tunnels_file_path());
                    if let Some(pop_rc) = pop_weak_del.upgrade() {
                        if let Some(f) = pop_rc.borrow().as_ref() {
                            f();
                        }
                    }
                });
                row.add_suffix(&btn_del);

                // Suffix 4: Toggle Active Switch (spawns/stops real OpenSSH child process)
                let sw = gtk4::Switch::builder()
                    .valign(gtk4::Align::Center)
                    .active(tunnel.status == "active")
                    .build();
                let tid_sw = tunnel.id.clone();
                let tun_clone = tunnel.clone();
                let reg_sw = Rc::clone(&reg_clone);
                let dot_sw = dot.clone();
                let src_display = tunnel.display_source();
                let st_sw = status_lbl_outer.clone();
                let pop_weak_sw = pop_weak.clone();
                sw.connect_state_set(move |sw_ref, state| {
                    dot_sw.remove_css_class("success");
                    dot_sw.remove_css_class("error");
                    dot_sw.remove_css_class("warning");
                    dot_sw.remove_css_class("dim-label");
                    if state {
                        match start_tunnel_process(&tun_clone) {
                            Ok(()) => {
                                reg_sw.borrow_mut().set_status(&tid_sw, "active", "");
                                let _ = reg_sw.borrow().save_to_file(&tunnels_file_path());
                                dot_sw.add_css_class("success");
                                dot_sw.set_tooltip_text(Some(&tr_fmt(
                                    "Ativo (Ouvindo em {src})",
                                    &[("src", &src_display)],
                                )));
                                st_sw.set_text(&tr_fmt(
                                    "Túnel ativo em {src}",
                                    &[("src", &src_display)],
                                ));
                                // Check after 450ms if SSH failed to bind port or auth failed
                                let tid_chk = tid_sw.clone();
                                let reg_chk = Rc::clone(&reg_sw);
                                let st_chk = st_sw.clone();
                                let pw_chk = pop_weak_sw.clone();
                                glib::timeout_add_local_once(
                                    std::time::Duration::from_millis(450),
                                    move || {
                                        if let Some(res) = check_tunnel_process_exit(&tid_chk) {
                                            match res {
                                                Ok(()) => {
                                                    reg_chk.borrow_mut().set_status(&tid_chk, "stopped", "");
                                                }
                                                Err(err) => {
                                                    reg_chk.borrow_mut().set_status(&tid_chk, "error", &err);
                                                    st_chk.set_text(&tr_fmt(
                                                        "Erro no túnel SSH: {err}",
                                                        &[("err", &err)],
                                                    ));
                                                }
                                            }
                                            let _ = reg_chk.borrow().save_to_file(&tunnels_file_path());
                                            if let Some(rc) = pw_chk.upgrade() {
                                                if let Some(f) = rc.borrow().as_ref() {
                                                    f();
                                                }
                                            }
                                        }
                                    },
                                );
                            }
                            Err(err) => {
                                reg_sw.borrow_mut().set_status(&tid_sw, "error", &err);
                                let _ = reg_sw.borrow().save_to_file(&tunnels_file_path());
                                dot_sw.add_css_class("error");
                                dot_sw.set_tooltip_text(Some(&err));
                                st_sw.set_text(&err);
                                sw_ref.set_active(false);
                                return glib::Propagation::Stop;
                            }
                        }
                    } else {
                        stop_tunnel_process(&tid_sw);
                        reg_sw.borrow_mut().set_status(&tid_sw, "stopped", "");
                        let _ = reg_sw.borrow().save_to_file(&tunnels_file_path());
                        dot_sw.add_css_class("dim-label");
                        dot_sw.set_tooltip_text(Some(&tr("Stopped")));
                    }
                    glib::Propagation::Proceed
                });
                row.add_suffix(&sw);

                list_clone.append(&row);
            }

            status_p_clone.set_visible(visible_count == 0);
            list_clone.set_visible(visible_count > 0);
        });

        *populate_list.borrow_mut() = Some(pop_func);
    }

    // Call populate once
    if let Some(f) = populate_list.borrow().as_ref() {
        f();
    }

    // Connect search entry filter
    {
        let pop_ref = Rc::clone(&populate_list);
        search_entry.connect_search_changed(move |_| {
            if let Some(f) = pop_ref.borrow().as_ref() {
                f();
            }
        });
    }

    // Connect Add button
    {
        let reg_add = Rc::clone(&reg_rc);
        let win_add = win.clone();
        let pop_ref = Rc::clone(&populate_list);
        btn_add.connect_clicked(move |_| {
            let reg_save = Rc::clone(&reg_add);
            let pop_save = Rc::clone(&pop_ref);
            show_tunnel_edit_dialog(
                &win_add,
                None,
                move |new_tunnel| {
                    reg_save.borrow_mut().register(new_tunnel);
                    let _ = reg_save.borrow().save_to_file(&tunnels_file_path());
                    if let Some(f) = pop_save.borrow().as_ref() {
                        f();
                    }
                },
            );
        });
    }

    // Connect Stop All button
    {
        let reg_stop = Rc::clone(&reg_rc);
        let pop_ref = Rc::clone(&populate_list);
        let status = status_label.clone();
        btn_stop_all.connect_clicked(move |_| {
            stop_all_tunnel_processes();
            reg_stop.borrow_mut().stop_all();
            let _ = reg_stop.borrow().save_to_file(&tunnels_file_path());
            if let Some(f) = pop_ref.borrow().as_ref() {
                f();
            }
            status.set_text(&tr("Todos os túneis SSH foram parados."));
        });
    }

    win.present();
}

/// Modal dialog for creating or editing an individual SSH port forwarding tunnel.
pub fn show_tunnel_edit_dialog<F>(
    parent: &adw::Window,
    existing: Option<&SshTunnel>,
    on_save: F,
) where
    F: Fn(SshTunnel) + 'static,
{
    let is_new = existing.is_none();
    let initial_data = existing.cloned().unwrap_or_default();

    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&if is_new {
            tr("Add SSH Tunnel")
        } else {
            tr("Edit SSH Tunnel")
        })
        .default_width(520)
        .default_height(560)
        .resizable(false)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();

    let btn_cancel = gtk4::Button::with_label(&tr("Cancelar"));
    let btn_save = gtk4::Button::with_label(&tr("Save"));
    btn_save.add_css_class("suggested-action");

    header.pack_start(&btn_cancel);
    header.pack_end(&btn_save);
    toolbar_view.add_top_bar(&header);

    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();

    let page = adw::PreferencesPage::new();
    scrolled.set_child(Some(&page));
    toolbar_view.set_content(Some(&scrolled));
    dlg.set_content(Some(&toolbar_view));

    // Group 1: Forwarding Mode
    let type_group = adw::PreferencesGroup::builder()
        .title(&tr("Forwarding Mode"))
        .build();
    let type_combo = adw::ComboRow::builder()
        .title(&tr("Tunnel Type"))
        .subtitle(&tr(
            "Escolha entre Local (-L), Remoto (-R) ou Dinâmico SOCKS5 (-D)",
        ))
        .build();
    let t_opt_local = tr("Local (-L) • Connect local port to remote service");
    let t_opt_remote = tr("Remoto (-R) • Expor porta local em servidor remoto");
    let t_opt_dynamic = tr("Dinâmico (-D) • Proxy SOCKS5 através de SSH");
    let type_list = gtk4::StringList::new(&[&t_opt_local, &t_opt_remote, &t_opt_dynamic]);
    type_combo.set_model(Some(&type_list));
    type_combo.set_selected(match initial_data.tunnel_type.as_str() {
        "remote" => 1,
        "dynamic" => 2,
        _ => 0,
    });
    type_group.add(&type_combo);
    page.add(&type_group);

    // Group 2: Endpoints & Ports
    let config_group = adw::PreferencesGroup::builder()
        .title(&tr("Pontos de Extremidade e Portas"))
        .build();

    let name_row = adw::EntryRow::builder()
        .title(&tr("Tunnel Name"))
        .text(&initial_data.name)
        .build();
    config_group.add(&name_row);

    let session_row = adw::EntryRow::builder()
        .title(&tr("Sessão SSH Vinculada"))
        .text(&initial_data.session_name)
        .build();
    config_group.add(&session_row);

    let local_host_row = adw::EntryRow::builder()
        .title(&tr("Interface Local / Bind IP"))
        .text(&initial_data.local_host)
        .build();
    config_group.add(&local_host_row);

    let local_port_row = adw::SpinRow::builder()
        .title(&tr("Local Port"))
        .adjustment(&gtk4::Adjustment::new(
            initial_data.local_port as f64,
            1.0,
            65535.0,
            1.0,
            10.0,
            0.0,
        ))
        .build();
    config_group.add(&local_port_row);

    let remote_host_row = adw::EntryRow::builder()
        .title(&tr("Destination Host"))
        .text(&initial_data.remote_host)
        .build();
    config_group.add(&remote_host_row);

    let remote_port_row = adw::SpinRow::builder()
        .title(&tr("Destination Port"))
        .adjustment(&gtk4::Adjustment::new(
            if initial_data.remote_port > 0 { initial_data.remote_port as f64 } else { 80.0 },
            1.0,
            65535.0,
            1.0,
            10.0,
            0.0,
        ))
        .build();
    config_group.add(&remote_port_row);

    page.add(&config_group);

    // Group 3: Options
    let opt_group = adw::PreferencesGroup::builder()
        .title(&tr("Options"))
        .build();
    let auto_start_row = adw::SwitchRow::builder()
        .title(&tr("Auto-start with Session"))
        .subtitle(&tr("Abre este túnel automaticamente ao conectar ao host SSH"))
        .active(initial_data.auto_start)
        .build();
    opt_group.add(&auto_start_row);
    page.add(&opt_group);

    // Function to adjust labels and visibility based on selected mode
    let update_visibility = {
        let tc = type_combo.clone();
        let lh_row = local_host_row.clone();
        let lp_row = local_port_row.clone();
        let rh_row = remote_host_row.clone();
        let rp_row = remote_port_row.clone();

        Rc::new(move || {
            let idx = tc.selected();
            if idx == 2 {
                // Dynamic SOCKS5
                rh_row.set_visible(false);
                rp_row.set_visible(false);
                lh_row.set_title(&tr("SOCKS5 Bind Interface"));
                lp_row.set_title(&tr("Porta do Proxy SOCKS5 (-D)"));
            } else if idx == 1 {
                // Remote
                rh_row.set_visible(true);
                rp_row.set_visible(true);
                lh_row.set_title(&tr("Local Source IP"));
                lp_row.set_title(&tr("Local Source Port"));
                rh_row.set_title(&tr("Host Remoto para Bind"));
                rp_row.set_title(&tr("Porta Exposta Remota (-R)"));
            } else {
                // Local
                rh_row.set_visible(true);
                rp_row.set_visible(true);
                lh_row.set_title(&tr("Interface de Escuta Local"));
                lp_row.set_title(&tr("Porta de Escuta Local (-L)"));
                rh_row.set_title(&tr("Remote Destination Host"));
                rp_row.set_title(&tr("Porta de Destino Remota"));
            }
        })
    };

    update_visibility();
    {
        let uv = Rc::clone(&update_visibility);
        type_combo.connect_selected_notify(move |_| {
            uv();
        });
    }

    // Cancel action
    {
        let d = dlg.clone();
        btn_cancel.connect_clicked(move |_| {
            d.close();
        });
    }

    // Save action
    {
        let d = dlg.clone();
        let id_val = if is_new {
            format!("tun_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis())
        } else {
            initial_data.id.clone()
        };
        let status_val = initial_data.status.clone();

        btn_save.connect_clicked(move |_| {
            let idx = type_combo.selected();
            let t_type = match idx {
                1 => "remote",
                2 => "dynamic",
                _ => "local",
            };

            let name = name_row.text().to_string();
            let final_name = if name.trim().is_empty() {
                tr_fmt("{type} Tunnel", &[("type", &t_type.to_uppercase())])
            } else {
                name.trim().to_string()
            };

            let local_host = local_host_row.text().to_string();
            let final_lh = if local_host.trim().is_empty() {
                "127.0.0.1".to_string()
            } else {
                local_host.trim().to_string()
            };

            let remote_host = remote_host_row.text().to_string();
            let final_rh = if t_type == "dynamic" {
                String::new()
            } else if remote_host.trim().is_empty() {
                "localhost".to_string()
            } else {
                remote_host.trim().to_string()
            };

            let tunnel = SshTunnel {
                id: id_val.clone(),
                name: final_name,
                session_name: session_row.text().trim().to_string(),
                tunnel_type: t_type.to_string(),
                local_host: final_lh,
                local_port: local_port_row.value() as u16,
                remote_host: final_rh,
                remote_port: if t_type == "dynamic" { 0 } else { remote_port_row.value() as u16 },
                auto_start: auto_start_row.is_active(),
                status: status_val.clone(),
                error_message: String::new(),
            };

            d.close();
            on_save(tunnel);
        });
    }

    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tunnel_registry_roundtrip() {
        let mut reg = SshTunnelRegistry::new();
        let t = SshTunnel {
            id: "tun_test_1".to_string(),
            name: "API Proxy".to_string(),
            session_name: "Bastion".to_string(),
            tunnel_type: "local".to_string(),
            local_host: "127.0.0.1".to_string(),
            local_port: 8080,
            remote_host: "api.internal".to_string(),
            remote_port: 80,
            auto_start: true,
            status: "active".to_string(),
            error_message: String::new(),
        };

        reg.register(t.clone());
        assert_eq!(reg.all().len(), 1);
        assert_eq!(reg.get("tun_test_1").unwrap().name, "API Proxy");

        let temp_dir = std::env::temp_dir().join(format!("onyxsh_test_tun_{}", std::process::id()));
        let file = temp_dir.join("tunnels.json");
        assert!(reg.save_to_file(&file).is_ok());

        let mut loaded = SshTunnelRegistry::new();
        assert!(loaded.load_from_file(&file).is_ok());
        assert_eq!(loaded.all().len(), 1);
        assert_eq!(loaded.get("tun_test_1").unwrap().display_source(), "127.0.0.1:8080");

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_build_tunnel_spawn_argv_with_password_and_flatpak() {
        let tunnel = SshTunnel {
            id: "tun_pg".to_string(),
            name: "PG".to_string(),
            session_name: "prod".to_string(),
            tunnel_type: "local".to_string(),
            local_host: "127.0.0.1".to_string(),
            local_port: 15432,
            remote_host: "10.0.0.5".to_string(),
            remote_port: 5432,
            auto_start: true,
            status: "stopped".to_string(),
            error_message: String::new(),
        };
        let mut sess = SessionItem::default();
        sess.session_type = "ssh".to_string();
        sess.host = "bastion.internal".to_string();
        sess.user = "ops".to_string();
        sess.auth_type = "password".to_string();
        let builder = CommandBuilder {
            force_flatpak: Some(false),
            host_spawn_path: None,
            flatpak_spawn_available: None,
        };
        let (argv, env) = build_tunnel_spawn_argv(&tunnel, &sess, Some("pw123"), &builder);
        assert_eq!(argv[0], "sshpass");
        assert!(!argv.contains(&"BatchMode=yes".to_string()));
        assert!(argv.contains(&"127.0.0.1:15432:10.0.0.5:5432".to_string()));
        assert_eq!(env.get("SSHPASS").map(String::as_str), Some("pw123"));
    }
}
