//! Native GTK4 + Libadwaita TFTP Server Configuration & Live Audit Dialog.
//!
//! Full native port of `src/onyxsh/ui/dialogs/tftp_server_dialog.py`:
//! - Configures bind address, UDP port (default 6969), Upload root (RRQ), and Download dir (WRQ).
//! - Persists preferences into `AppSettings` (`settings.json`).
//! - Starts/stops [`UdpTftpServer`] and streams live [`TftpTransferEvent`] logs.

use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform;
use onyxsh_filemanager::backend::{TftpServerConfig, TftpTransferEvent, UdpTftpServer};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_AUDIT_EVENTS: usize = 200;

/// Shared runtime state for the embedded TFTP server and its audit log.
pub struct TftpServerState {
    pub server: Option<UdpTftpServer>,
    pub events: Arc<Mutex<Vec<TftpTransferEvent>>>,
    pub active_dialog: Option<adw::Window>,
}

impl Default for TftpServerState {
    fn default() -> Self {
        Self {
            server: None,
            events: Arc::new(Mutex::new(Vec::new())),
            active_dialog: None,
        }
    }
}

impl TftpServerState {
    pub fn is_running(&self) -> bool {
        self.server.as_ref().is_some_and(|srv| srv.is_running())
    }

    pub fn active_bind_summary(&self) -> Option<String> {
        self.server.as_ref().and_then(|srv| {
            srv.local_addr()
                .ok()
                .map(|addr| addr.to_string())
                .or_else(|| {
                    Some(format!(
                        "{}:{}",
                        srv.config().bind_addr,
                        srv.config().port
                    ))
                })
        })
    }

    pub fn stop(&mut self) {
        if let Some(srv) = self.server.take() {
            srv.stop();
        }
    }
}

/// Validates bind address, port, and folder strings into a [`TftpServerConfig`].
pub fn build_tftp_config(
    bind_addr: &str,
    port: u16,
    upload_dir: &str,
    download_dir: &str,
) -> Result<TftpServerConfig, String> {
    let bind_trimmed = bind_addr.trim();
    let bind = if bind_trimmed.is_empty() {
        "0.0.0.0".to_string()
    } else {
        bind_trimmed.to_string()
    };
    if port == 0 {
        return Err(tr("Informe uma porta UDP válida (1–65535)."));
    }
    let home = platform::home_dir();
    let up = if upload_dir.trim().is_empty() {
        home.clone()
    } else {
        PathBuf::from(upload_dir.trim())
    };
    let down = if download_dir.trim().is_empty() {
        home
    } else {
        PathBuf::from(download_dir.trim())
    };
    if !up.exists() || !up.is_dir() {
        return Err(tr_fmt(
            "Diretório de upload (RRQ) inválido: {path}",
            &[("path", &up.display().to_string())],
        ));
    }
    if !down.exists() || !down.is_dir() {
        return Err(tr_fmt(
            "Diretório de download (WRQ) inválido: {path}",
            &[("path", &down.display().to_string())],
        ));
    }
    Ok(TftpServerConfig {
        bind_addr: bind,
        port,
        upload_dir: up,
        download_dir: down,
        ..TftpServerConfig::default()
    })
}

/// Formats a single TFTP transfer audit event for list display.
pub fn format_tftp_event_title(ev: &TftpTransferEvent) -> String {
    let icon = if ev.success { "✓" } else { "✗" };
    format!(
        "[{ts}] {icon} {op} {file} ({bytes} B)",
        ts = ev.timestamp,
        op = ev.operation,
        file = ev.filename,
        bytes = ev.bytes
    )
}

fn persist_tftp_settings(
    settings_rc: &Rc<RefCell<AppSettings>>,
    bind_addr: &str,
    port: u16,
    upload_dir: &str,
    download_dir: &str,
) {
    {
        let mut st = settings_rc.borrow_mut();
        st.tftp_server_bind_addr = bind_addr.trim().to_string();
        st.tftp_server_port = port;
        st.tftp_server_upload_dir = upload_dir.trim().to_string();
        st.tftp_server_download_dir = download_dir.trim().to_string();
    }
    let cfg_path = platform::config_dir().join("settings.json");
    let _ = settings_rc.borrow().save_to_path(&cfg_path);
}

fn render_event_list(list_box: &gtk4::ListBox, events: &[TftpTransferEvent]) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
    if events.is_empty() {
        let row = adw::ActionRow::builder()
            .title(&tr("Nenhuma transferência registrada ainda."))
            .subtitle(&tr("Aguardando pacotes RRQ (leitura) ou WRQ (escrita)..."))
            .build();
        row.set_activatable(false);
        list_box.append(&row);
        return;
    }
    for ev in events.iter().rev() {
        let row = adw::ActionRow::builder()
            .title(&format_tftp_event_title(ev))
            .subtitle(&format!("{} — {}", ev.client, ev.detail))
            .build();
        row.set_activatable(false);
        if !ev.success {
            row.add_css_class("error");
        }
        list_box.append(&row);
    }
}

/// Opens or focuses the TFTP Server configuration & monitoring dialog.
pub fn show_tftp_server_dialog<F>(
    parent: &adw::ApplicationWindow,
    settings_rc: &Rc<RefCell<AppSettings>>,
    state_rc: &Rc<RefCell<TftpServerState>>,
    status_lbl: &gtk4::Label,
    on_status_changed: F,
) where
    F: Fn(Option<String>) + 'static,
{
    if let Some(existing) = state_rc.borrow().active_dialog.as_ref() {
        existing.present();
        return;
    }

    let on_status_changed: Rc<dyn Fn(Option<String>)> = Rc::new(on_status_changed);
    let dialog = adw::Window::builder()
        .title(&tr("Servidor TFTP Integrado (RFC 1350)"))
        .transient_for(parent)
        .modal(false)
        .default_width(640)
        .default_height(560)
        .build();
    state_rc.borrow_mut().active_dialog = Some(dialog.clone());

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let header = adw::HeaderBar::new();
    let badge_lbl = gtk4::Label::new(None);
    badge_lbl.add_css_class("caption");
    header.pack_end(&badge_lbl);
    root.append(&header);

    let scroller = gtk4::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .build();
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(18);
    content.set_margin_end(18);

    // Network Group
    let net_group = adw::PreferencesGroup::builder()
        .title(&tr("Rede e Porta UDP"))
        .description(&tr(
            "Nota: no Linux, portas abaixo de 1024 exigem privilégios elevados. Padrão recomendado: 6969.",
        ))
        .build();

    let (init_bind, init_port, init_up, init_down) = {
        let st = settings_rc.borrow();
        let home_str = platform::home_dir().display().to_string();
        (
            if st.tftp_server_bind_addr.trim().is_empty() {
                "0.0.0.0".to_string()
            } else {
                st.tftp_server_bind_addr.clone()
            },
            if st.tftp_server_port == 0 {
                6969
            } else {
                st.tftp_server_port
            },
            if st.tftp_server_upload_dir.trim().is_empty() {
                home_str.clone()
            } else {
                st.tftp_server_upload_dir.clone()
            },
            if st.tftp_server_download_dir.trim().is_empty() {
                home_str
            } else {
                st.tftp_server_download_dir.clone()
            },
        )
    };

    let bind_row = adw::EntryRow::builder()
        .title(&tr("Endereço de escuta (Bind)"))
        .text(&init_bind)
        .build();
    net_group.add(&bind_row);

    let port_adj = gtk4::Adjustment::new(f64::from(init_port), 1.0, 65535.0, 1.0, 10.0, 0.0);
    let port_spin = gtk4::SpinButton::builder()
        .adjustment(&port_adj)
        .numeric(true)
        .valign(gtk4::Align::Center)
        .build();
    let port_row = adw::ActionRow::builder()
        .title(&tr("Porta UDP"))
        .subtitle(&tr("Use 6969 para evitar necessidade de root, ou 69 com CAP_NET_BIND_SERVICE"))
        .build();
    port_row.add_suffix(&port_spin);
    net_group.add(&port_row);
    content.append(&net_group);

    // Folders Group
    let dir_group = adw::PreferencesGroup::builder()
        .title(&tr("Diretórios do Servidor"))
        .description(&tr("Todos os caminhos ficam confinados estritamente dentro destas pastas."))
        .build();

    let upload_entry = gtk4::Entry::builder()
        .text(&init_up)
        .hexpand(true)
        .valign(gtk4::Align::Center)
        .build();
    let btn_pick_upload = gtk4::Button::from_icon_name("folder-open-symbolic");
    btn_pick_upload.set_valign(gtk4::Align::Center);
    btn_pick_upload.set_tooltip_text(Some(&tr("Escolher pasta de Upload (RRQ)")));
    let up_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    up_box.set_size_request(300, -1);
    up_box.append(&upload_entry);
    up_box.append(&btn_pick_upload);
    let up_row = adw::ActionRow::builder()
        .title(&tr("Pasta de Upload (RRQ — clientes leem)"))
        .build();
    up_row.add_suffix(&up_box);
    dir_group.add(&up_row);

    let download_entry = gtk4::Entry::builder()
        .text(&init_down)
        .hexpand(true)
        .valign(gtk4::Align::Center)
        .build();
    let btn_pick_download = gtk4::Button::from_icon_name("folder-open-symbolic");
    btn_pick_download.set_valign(gtk4::Align::Center);
    btn_pick_download.set_tooltip_text(Some(&tr("Escolher pasta de Download (WRQ)")));
    let down_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    down_box.set_size_request(300, -1);
    down_box.append(&download_entry);
    down_box.append(&btn_pick_download);
    let down_row = adw::ActionRow::builder()
        .title(&tr("Pasta de Download (WRQ — clientes enviam)"))
        .build();
    down_row.add_suffix(&down_box);
    dir_group.add(&down_row);
    content.append(&dir_group);

    // Controls Bar
    let ctrl_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    let btn_start = gtk4::Button::with_label(&tr("Iniciar Servidor"));
    btn_start.add_css_class("suggested-action");
    let btn_stop = gtk4::Button::with_label(&tr("Parar Servidor"));
    btn_stop.add_css_class("destructive-action");
    let state_banner = gtk4::Label::new(None);
    state_banner.set_halign(gtk4::Align::Start);
    state_banner.set_hexpand(true);
    state_banner.set_wrap(true);
    ctrl_box.append(&btn_start);
    ctrl_box.append(&btn_stop);
    ctrl_box.append(&state_banner);
    content.append(&ctrl_box);

    // Audit Log
    let log_hdr = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let log_title = gtk4::Label::new(Some(&tr("Log de Transferências ao Vivo")));
    log_title.add_css_class("heading");
    log_title.set_halign(gtk4::Align::Start);
    log_title.set_hexpand(true);
    let btn_clear_log = gtk4::Button::with_label(&tr("Limpar Log"));
    btn_clear_log.add_css_class("flat");
    log_hdr.append(&log_title);
    log_hdr.append(&btn_clear_log);
    content.append(&log_hdr);

    let event_list = gtk4::ListBox::new();
    event_list.add_css_class("boxed-list");
    event_list.set_selection_mode(gtk4::SelectionMode::None);
    let log_frame = gtk4::ScrolledWindow::builder()
        .min_content_height(190)
        .vexpand(true)
        .child(&event_list)
        .build();
    content.append(&log_frame);

    scroller.set_child(Some(&content));
    root.append(&scroller);
    dialog.set_content(Some(&root));

    // Sync UI controls from server state
    let sync_controls: Rc<dyn Fn()> = {
        let state_rc = Rc::clone(state_rc);
        let btn_start = btn_start.clone();
        let btn_stop = btn_stop.clone();
        let bind_row = bind_row.clone();
        let port_spin = port_spin.clone();
        let upload_entry = upload_entry.clone();
        let download_entry = download_entry.clone();
        let btn_pick_upload = btn_pick_upload.clone();
        let btn_pick_download = btn_pick_download.clone();
        let badge_lbl = badge_lbl.clone();
        let state_banner = state_banner.clone();
        Rc::new(move || {
            let summary = state_rc.borrow().active_bind_summary();
            let running = summary.is_some();
            btn_start.set_sensitive(!running);
            btn_stop.set_sensitive(running);
            bind_row.set_sensitive(!running);
            port_spin.set_sensitive(!running);
            upload_entry.set_sensitive(!running);
            download_entry.set_sensitive(!running);
            btn_pick_upload.set_sensitive(!running);
            btn_pick_download.set_sensitive(!running);
            if let Some(addr) = summary {
                badge_lbl.set_text(&tr_fmt("Ativo: {addr}", &[("addr", &addr)]));
                state_banner.set_text(&tr_fmt(
                    "Servidor TFTP UDP ativo em {addr} (RRQ/WRQ).",
                    &[("addr", &addr)],
                ));
            } else {
                badge_lbl.set_text(&tr("Parado"));
                state_banner.set_text(&tr("Servidor TFTP parado."));
            }
        })
    };
    sync_controls();

    {
        let events_arc = Arc::clone(&state_rc.borrow().events);
        if let Ok(guard) = events_arc.lock() {
            render_event_list(&event_list, &guard);
        };
    }

    // Folder pickers via GTK 4.10 FileDialog
    {
        let dlg = dialog.clone();
        let entry = upload_entry.clone();
        btn_pick_upload.connect_clicked(move |_| {
            let fd = gtk4::FileDialog::builder()
                .title(&tr("Selecionar Pasta de Upload (RRQ)"))
                .modal(true)
                .build();
            let current = PathBuf::from(entry.text().as_str());
            if current.is_dir() {
                fd.set_initial_folder(Some(&gio::File::for_path(&current)));
            }
            let entry_c = entry.clone();
            fd.select_folder(Some(&dlg), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        entry_c.set_text(&path.display().to_string());
                    }
                }
            });
        });
    }
    {
        let dlg = dialog.clone();
        let entry = download_entry.clone();
        btn_pick_download.connect_clicked(move |_| {
            let fd = gtk4::FileDialog::builder()
                .title(&tr("Selecionar Pasta de Download (WRQ)"))
                .modal(true)
                .build();
            let current = PathBuf::from(entry.text().as_str());
            if current.is_dir() {
                fd.set_initial_folder(Some(&gio::File::for_path(&current)));
            }
            let entry_c = entry.clone();
            fd.select_folder(Some(&dlg), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(path) = file.path() {
                        entry_c.set_text(&path.display().to_string());
                    }
                }
            });
        });
    }

    // Start button
    {
        let state_rc = Rc::clone(state_rc);
        let settings_rc = Rc::clone(settings_rc);
        let bind_row = bind_row.clone();
        let port_spin = port_spin.clone();
        let upload_entry = upload_entry.clone();
        let download_entry = download_entry.clone();
        let state_banner = state_banner.clone();
        let status_lbl = status_lbl.clone();
        let sync_c = Rc::clone(&sync_controls);
        let on_changed = Rc::clone(&on_status_changed);
        btn_start.connect_clicked(move |_| {
            let bind_txt = bind_row.text().to_string();
            let port = port_spin.value() as u16;
            let up_txt = upload_entry.text().to_string();
            let down_txt = download_entry.text().to_string();
            persist_tftp_settings(&settings_rc, &bind_txt, port, &up_txt, &down_txt);

            let cfg = match build_tftp_config(&bind_txt, port, &up_txt, &down_txt) {
                Ok(c) => c,
                Err(msg) => {
                    state_banner.set_text(&msg);
                    status_lbl.set_text(&msg);
                    return;
                }
            };

            let events_arc = Arc::clone(&state_rc.borrow().events);
            let events_cb = Arc::clone(&events_arc);
            match UdpTftpServer::start(
                cfg,
                Some(Arc::new(move |ev| {
                    if let Ok(mut list) = events_cb.lock() {
                        list.push(ev);
                        if list.len() > MAX_AUDIT_EVENTS {
                            let excess = list.len() - MAX_AUDIT_EVENTS;
                            list.drain(0..excess);
                        }
                    }
                })),
            ) {
                Ok(srv) => {
                    let addr = srv
                        .local_addr()
                        .map(|a| a.to_string())
                        .unwrap_or_else(|_| format!("{bind_txt}:{port}"));
                    state_rc.borrow_mut().server = Some(srv);
                    status_lbl.set_text(&tr_fmt(
                        "Servidor TFTP ativo em {addr} (RRQ/WRQ).",
                        &[("addr", &addr)],
                    ));
                    on_changed(Some(addr));
                    sync_c();
                }
                Err(err) => {
                    let msg = tr_fmt(
                        "Falha ao iniciar servidor TFTP em {bind}:{port}: {err}",
                        &[
                            ("bind", &bind_txt),
                            ("port", &port.to_string()),
                            ("err", &err.to_string()),
                        ],
                    );
                    state_banner.set_text(&msg);
                    status_lbl.set_text(&msg);
                }
            }
        });
    }

    // Stop button
    {
        let state_rc = Rc::clone(state_rc);
        let status_lbl = status_lbl.clone();
        let sync_c = Rc::clone(&sync_controls);
        let on_changed = Rc::clone(&on_status_changed);
        btn_stop.connect_clicked(move |_| {
            state_rc.borrow_mut().stop();
            status_lbl.set_text(&tr("Servidor TFTP encerrado."));
            on_changed(None);
            sync_c();
        });
    }

    // Clear log button
    {
        let state_rc = Rc::clone(state_rc);
        let event_list = event_list.clone();
        btn_clear_log.connect_clicked(move |_| {
            if let Ok(mut list) = state_rc.borrow().events.lock() {
                list.clear();
            }
            render_event_list(&event_list, &[]);
        });
    }

    // Live polling timer for transfer log while dialog stays open
    let last_rendered_count = Rc::new(RefCell::new(0usize));
    let timer_source: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    {
        let state_rc_t = Rc::clone(state_rc);
        let event_list_t = event_list.clone();
        let last_count_t = Rc::clone(&last_rendered_count);
        let sid = glib::timeout_add_local(Duration::from_millis(400), move || {
            let events_arc = Arc::clone(&state_rc_t.borrow().events);
            if let Ok(guard) = events_arc.lock() {
                if guard.len() != *last_count_t.borrow() {
                    *last_count_t.borrow_mut() = guard.len();
                    render_event_list(&event_list_t, &guard);
                }
            }
            glib::ControlFlow::Continue
        });
        *timer_source.borrow_mut() = Some(sid);
    }

    {
        let state_rc_close = Rc::clone(state_rc);
        let settings_rc_close = Rc::clone(settings_rc);
        let bind_row = bind_row.clone();
        let port_spin = port_spin.clone();
        let upload_entry = upload_entry.clone();
        let download_entry = download_entry.clone();
        let timer_c = Rc::clone(&timer_source);
        dialog.connect_close_request(move |_| {
            if let Some(sid) = timer_c.borrow_mut().take() {
                sid.remove();
            }
            persist_tftp_settings(
                &settings_rc_close,
                &bind_row.text(),
                port_spin.value() as u16,
                &upload_entry.text(),
                &download_entry.text(),
            );
            state_rc_close.borrow_mut().active_dialog = None;
            glib::Propagation::Proceed
        });
    }

    dialog.present();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_build_tftp_config_validation() {
        let tmp = std::env::temp_dir().join(format!("onyxsh-tftp-dlg-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let err_port = build_tftp_config("0.0.0.0", 0, tmp.to_str().unwrap(), tmp.to_str().unwrap());
        assert!(err_port.is_err());

        let err_dir = build_tftp_config(
            "127.0.0.1",
            6969,
            "/nonexistent_onyxsh_tftp_dir_123",
            tmp.to_str().unwrap(),
        );
        assert!(err_dir.is_err());

        let ok_cfg =
            build_tftp_config("  ", 6969, tmp.to_str().unwrap(), tmp.to_str().unwrap()).unwrap();
        assert_eq!(ok_cfg.bind_addr, "0.0.0.0");
        assert_eq!(ok_cfg.port, 6969);
        assert_eq!(ok_cfg.upload_dir, tmp);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_format_tftp_event_title() {
        let ev = TftpTransferEvent {
            timestamp: "12:34:56 UTC".to_string(),
            client: "10.0.0.2:4455".to_string(),
            operation: "WRQ".to_string(),
            filename: "firmware.bin".to_string(),
            bytes: 2048,
            success: true,
            detail: "OK".to_string(),
        };
        let line = format_tftp_event_title(&ev);
        assert!(line.contains("✓ WRQ firmware.bin (2048 B)"));
    }
}
