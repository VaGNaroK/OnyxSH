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
use onyxsh_core::platform;
use onyxsh_terminal::tunnel_manager::{SshTunnel, SshTunnelRegistry};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

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
            name: "PostgreSQL Remoto".to_string(),
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
            name: "SOCKS5 Proxy Corporativo".to_string(),
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
        .title("Gerenciador de Túneis SSH")
        .default_width(720)
        .default_height(580)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();

    // Add Tunnel Button
    let btn_add = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .tooltip_text("Adicionar Novo Túnel SSH")
        .build();
    btn_add.add_css_class("suggested-action");

    // Stop All Tunnels Button
    let btn_stop_all = gtk4::Button::builder()
        .icon_name("media-playback-stop-symbolic")
        .tooltip_text("Parar Todos os Túneis Ativos")
        .build();
    btn_stop_all.add_css_class("flat");

    header.pack_start(&btn_add);
    header.pack_end(&btn_stop_all);
    toolbar_view.add_top_bar(&header);

    // Search Bar
    let search_bar = gtk4::SearchBar::new();
    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Pesquisar túneis por nome, porta ou host...")
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
        .title("Nenhum Túnel SSH")
        .description("Crie um túnel Local (-L), Remoto (-R) ou Dinâmico SOCKS5 (-D) para encaminhar portas via SSH.")
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
                        dot.set_tooltip_text(Some(&format!("Ativo (Ouvindo em {})", tunnel.display_source())));
                    }
                    "starting" => {
                        dot.add_css_class("warning");
                        dot.set_tooltip_text(Some("Conectando..."));
                    }
                    "error" => {
                        dot.add_css_class("error");
                        dot.set_tooltip_text(Some(&format!("Erro: {}", tunnel.error_message)));
                    }
                    _ => {
                        dot.add_css_class("dim-label");
                        dot.set_tooltip_text(Some("Parado"));
                    }
                }
                row.add_prefix(&dot);

                // Suffix 1: Quick Copy Button
                let btn_copy = gtk4::Button::builder()
                    .icon_name("edit-copy-symbolic")
                    .valign(gtk4::Align::Center)
                    .tooltip_text(format!("Copiar endereço local (localhost:{})", tunnel.local_port))
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
                    .tooltip_text("Editar Configurações do Túnel")
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
                    .tooltip_text("Remover Túnel")
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

                // Suffix 4: Toggle Active Switch
                let sw = gtk4::Switch::builder()
                    .valign(gtk4::Align::Center)
                    .active(tunnel.status == "active")
                    .build();
                let tid_sw = tunnel.id.clone();
                let reg_sw = Rc::clone(&reg_clone);
                let dot_sw = dot.clone();
                let src_display = tunnel.display_source();
                sw.connect_state_set(move |_, state| {
                    let new_status = if state { "active" } else { "stopped" };
                    reg_sw.borrow_mut().set_status(&tid_sw, new_status, "");
                    let _ = reg_sw.borrow().save_to_file(&tunnels_file_path());
                    dot_sw.remove_css_class("success");
                    dot_sw.remove_css_class("error");
                    dot_sw.remove_css_class("warning");
                    dot_sw.remove_css_class("dim-label");
                    if state {
                        dot_sw.add_css_class("success");
                        dot_sw.set_tooltip_text(Some(&format!("Ativo (Ouvindo em {src_display})")));
                    } else {
                        dot_sw.add_css_class("dim-label");
                        dot_sw.set_tooltip_text(Some("Parado"));
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
            reg_stop.borrow_mut().stop_all();
            let _ = reg_stop.borrow().save_to_file(&tunnels_file_path());
            if let Some(f) = pop_ref.borrow().as_ref() {
                f();
            }
            status.set_text("Todos os túneis SSH foram parados.");
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
        .title(if is_new { "Adicionar Túnel SSH" } else { "Editar Túnel SSH" })
        .default_width(520)
        .default_height(560)
        .resizable(false)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();

    let btn_cancel = gtk4::Button::with_label("Cancelar");
    let btn_save = gtk4::Button::with_label("Salvar");
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
        .title("Modo de Encaminhamento")
        .build();
    let type_combo = adw::ComboRow::builder()
        .title("Tipo de Túnel")
        .subtitle("Escolha entre Local (-L), Remoto (-R) ou Dinâmico SOCKS5 (-D)")
        .build();
    let type_list = gtk4::StringList::new(&[
        "Local (-L) • Conectar porta local a serviço remoto",
        "Remoto (-R) • Expor porta local em servidor remoto",
        "Dinâmico (-D) • Proxy SOCKS5 através de SSH",
    ]);
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
        .title("Pontos de Extremidade e Portas")
        .build();

    let name_row = adw::EntryRow::builder()
        .title("Nome do Túnel")
        .text(&initial_data.name)
        .build();
    config_group.add(&name_row);

    let session_row = adw::EntryRow::builder()
        .title("Sessão SSH Vinculada")
        .text(&initial_data.session_name)
        .build();
    config_group.add(&session_row);

    let local_host_row = adw::EntryRow::builder()
        .title("Interface Local / Bind IP")
        .text(&initial_data.local_host)
        .build();
    config_group.add(&local_host_row);

    let local_port_row = adw::SpinRow::builder()
        .title("Porta Local")
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
        .title("Host de Destino")
        .text(&initial_data.remote_host)
        .build();
    config_group.add(&remote_host_row);

    let remote_port_row = adw::SpinRow::builder()
        .title("Porta de Destino")
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
        .title("Opções")
        .build();
    let auto_start_row = adw::SwitchRow::builder()
        .title("Iniciar Automaticamente com a Sessão")
        .subtitle("Abre este túnel automaticamente ao conectar ao host SSH")
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
                lh_row.set_title("Interface de Escuta SOCKS5");
                lp_row.set_title("Porta do Proxy SOCKS5 (-D)");
            } else if idx == 1 {
                // Remote
                rh_row.set_visible(true);
                rp_row.set_visible(true);
                lh_row.set_title("IP de Origem Local");
                lp_row.set_title("Porta de Origem Local");
                rh_row.set_title("Host Remoto para Bind");
                rp_row.set_title("Porta Exposta Remota (-R)");
            } else {
                // Local
                rh_row.set_visible(true);
                rp_row.set_visible(true);
                lh_row.set_title("Interface de Escuta Local");
                lp_row.set_title("Porta de Escuta Local (-L)");
                rh_row.set_title("Host de Destino Remoto");
                rp_row.set_title("Porta de Destino Remota");
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
                format!("{} Tunnel", t_type.to_uppercase())
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
}
