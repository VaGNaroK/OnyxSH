//! Native GTK4 + Libadwaita + VTE4 Application Window (`src/onyxsh/window.py` & `ui/window_ui.py`).
//! Includes Phase 9 (Advanced VTE Search Bar with `Aa`, `\b`, `.*` & Match Counter)
//! and Phase 10 (Main Hamburger Menu with Theme Selector, Font Sizer, and 11 Actions).

use crate::main_menu::{
    apply_gtk_theme_mode, change_font_size, menu_custom_css, parse_font_string, GtkThemeMode,
};
use crate::terminal_search::{apply_vte_search, format_occurrence_counter};
use crate::theme_engine::{generate_theme_css, get_palette, is_light_hex_color};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::platform;
use onyxsh_core::sessions::SessionItem;
use onyxsh_core::{APP_ID, APP_NAME, APP_VERSION};
use onyxsh_filemanager::backend::UdpTftpServer;
use onyxsh_system::{
    calculate_cpu_usage_percent, parse_proc_meminfo, parse_proc_net_dev, parse_proc_stat,
};
use onyxsh_terminal::exporter::{export_markdown, ExportMetadata};
use std::cell::RefCell;
use std::rc::Rc;
use vte4::prelude::*;

/// Helper to parse `#RRGGBB` into `gdk::RGBA`.
fn parse_rgba(hex: &str) -> gdk::RGBA {
    gdk::RGBA::parse(hex).unwrap_or_else(|_| gdk::RGBA::new(0.06, 0.07, 0.09, 1.0))
}

/// Applies the palette and font from `AppSettings` to an existing `vte4::Terminal`.
fn apply_terminal_appearance(term: &vte4::Terminal, settings: &AppSettings) {
    let font_desc = pango::FontDescription::from_string(&settings.font);
    term.set_font(Some(&font_desc));
    term.set_scrollback_lines(settings.scrollback_lines as i64);

    let palette = get_palette(&settings.color_scheme);
    let fg = parse_rgba(palette.foreground);
    let bg = parse_rgba(palette.background);
    let cursor = parse_rgba(palette.cursor);
    let ansi_colors: Vec<gdk::RGBA> = palette.ansi.iter().map(|c| parse_rgba(c)).collect();
    let ansi_refs: Vec<&gdk::RGBA> = ansi_colors.iter().collect();

    term.set_colors(Some(&fg), Some(&bg), &ansi_refs);
    term.set_color_cursor(Some(&cursor));
}

/// Creates and configures a live `vte4::Terminal` widget and spawns `argv` inside a PTY.
fn create_vte_terminal(
    settings: &AppSettings,
    working_dir: Option<&str>,
    argv: &[&str],
    registry: &Rc<RefCell<Vec<vte4::Terminal>>>,
) -> vte4::Terminal {
    let term = vte4::Terminal::new();
    term.set_hexpand(true);
    term.set_vexpand(true);
    term.set_scroll_on_output(false);
    term.set_scroll_on_keystroke(true);
    term.set_audible_bell(false);

    apply_terminal_appearance(&term, settings);

    let default_shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let spawn_argv: Vec<&str> = if argv.is_empty() {
        vec![default_shell.as_str()]
    } else {
        argv.to_vec()
    };

    let home_dir = platform::home_dir().to_string_lossy().to_string();
    let cwd = working_dir.unwrap_or(&home_dir);

    term.spawn_async(
        vte4::PtyFlags::DEFAULT,
        Some(cwd),
        &spawn_argv,
        &[],
        glib::SpawnFlags::SEARCH_PATH,
        || {},
        -1,
        gio::Cancellable::NONE,
        |res| {
            if let Err(err) = res {
                tracing::warn!("Failed to spawn PTY process: {err}");
            }
        },
    );

    registry.borrow_mut().push(term.clone());
    term
}

/// Resolves the active `vte4::Terminal` inside the currently selected `adw::TabPage`.
fn get_active_vte_terminal(tab_view: &adw::TabView) -> Option<vte4::Terminal> {
    let page = tab_view.selected_page()?;
    let child = page.child();
    if let Ok(term) = child.clone().downcast::<vte4::Terminal>() {
        return Some(term);
    }
    if let Ok(paned) = child.downcast::<gtk4::Paned>() {
        if let Some(start) = paned.start_child() {
            if let Ok(term) = start.downcast::<vte4::Terminal>() {
                return Some(term);
            }
        }
    }
    None
}

/// Opens the Keyboard Shortcuts modal window (`win.shortcuts`).
fn show_shortcuts_dialog(parent: &adw::ApplicationWindow, settings: &AppSettings) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Atalhos de Teclado — OnyxSH")
        .default_width(520)
        .default_height(460)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let hb = adw::HeaderBar::new();
    vbox.append(&hb);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_margin_start(16);
    list.set_margin_end(16);
    list.set_margin_top(12);
    list.set_margin_bottom(12);

    let mut pairs: Vec<(&String, &String)> = settings.shortcuts.iter().collect();
    pairs.sort_by_key(|(k, _)| *k);

    for (action, accel) in pairs {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        row.set_margin_start(12);
        row.set_margin_end(12);
        row.set_margin_top(8);
        row.set_margin_bottom(8);

        let act_lbl = gtk4::Label::new(Some(action));
        act_lbl.set_hexpand(true);
        act_lbl.set_halign(gtk4::Align::Start);

        let key_lbl = gtk4::Label::new(Some(accel));
        key_lbl.add_css_class("monospace");
        key_lbl.add_css_class("dim-label");

        row.append(&act_lbl);
        row.append(&key_lbl);
        list.append(&row);
    }

    scroll.set_child(Some(&list));
    vbox.append(&scroll);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Opens the Preferences modal window (`win.preferences`).
fn show_preferences_dialog(
    parent: &adw::ApplicationWindow,
    settings: Rc<RefCell<AppSettings>>,
    terminals: Rc<RefCell<Vec<vte4::Terminal>>>,
    css_provider: gtk4::CssProvider,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Preferências — OnyxSH")
        .default_width(560)
        .default_height(440)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.set_margin_top(12);
    content.set_margin_bottom(18);

    // Color Scheme selector row
    let scheme_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let scheme_lbl = gtk4::Label::new(Some("Esquema de Cores do Terminal:"));
    scheme_lbl.set_hexpand(true);
    scheme_lbl.set_halign(gtk4::Align::Start);
    let scheme_list = gtk4::StringList::new(&["onyx_dark", "dracula", "solarized_light"]);
    let scheme_dd = gtk4::DropDown::new(Some(scheme_list), gtk4::Expression::NONE);
    let curr_scheme = settings.borrow().color_scheme.clone();
    scheme_dd.set_selected(match curr_scheme.as_str() {
        "dracula" => 1,
        "solarized_light" => 2,
        _ => 0,
    });
    scheme_row.append(&scheme_lbl);
    scheme_row.append(&scheme_dd);
    content.append(&scheme_row);

    // Scrollback row
    let sb_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let sb_lbl = gtk4::Label::new(Some("Linhas de Histórico (Scrollback):"));
    sb_lbl.set_hexpand(true);
    sb_lbl.set_halign(gtk4::Align::Start);
    let sb_spin = gtk4::SpinButton::with_range(500.0, 100_000.0, 500.0);
    sb_spin.set_value(settings.borrow().scrollback_lines as f64);
    sb_row.append(&sb_lbl);
    sb_row.append(&sb_spin);
    content.append(&sb_row);

    // OSC 52 Clipboard Switch
    let osc_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    let osc_lbl = gtk4::Label::new(Some("Permitir Clipboard Remoto (OSC 52 Zero-Trust):"));
    osc_lbl.set_hexpand(true);
    osc_lbl.set_halign(gtk4::Align::Start);
    let osc_sw = gtk4::Switch::new();
    osc_sw.set_active(settings.borrow().enable_osc52_clipboard);
    osc_row.append(&osc_lbl);
    osc_row.append(&osc_sw);
    content.append(&osc_row);

    // Save & Apply button
    let save_btn = gtk4::Button::with_label("Salvar e Aplicar");
    save_btn.add_css_class("suggested-action");
    let dlg_clone = dlg.clone();
    save_btn.connect_clicked(move |_| {
        {
            let mut st = settings.borrow_mut();
            st.color_scheme = match scheme_dd.selected() {
                1 => "dracula".to_string(),
                2 => "solarized_light".to_string(),
                _ => "onyx_dark".to_string(),
            };
            st.scrollback_lines = sb_spin.value() as u32;
            st.enable_osc52_clipboard = osc_sw.is_active();
            let cfg_path = platform::config_dir().join("settings.json");
            let _ = st.save_to_path(&cfg_path);

            let opacity_pct = 100u8.saturating_sub(st.transparency.min(100));
            let css = format!(
                "{}\n{}",
                generate_theme_css(&st.color_scheme, opacity_pct),
                menu_custom_css()
            );
            css_provider.load_from_string(&css);

            for term in terminals.borrow().iter() {
                apply_terminal_appearance(term, &st);
            }
        }
        dlg_clone.close();
    });
    content.append(&save_btn);

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Opens the SSH Tunnel Manager dialog (`win.show-tunnel-manager`).
fn show_tunnel_manager_dialog(parent: &adw::ApplicationWindow) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Gerenciador de Túneis SSH (-L / -R / -D)")
        .default_width(580)
        .default_height(380)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let info = gtk4::Label::new(Some(
        "Túneis SSH Ativos e Encaminhamento de Portas (Local -L, Remoto -R, SOCKS5 -D)",
    ));
    info.set_margin_top(12);
    vbox.append(&info);

    let list = gtk4::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_margin_start(16);
    list.set_margin_end(16);
    let sample_tunnel = onyxsh_terminal::tunnel_manager::SshTunnel::default();
    let row_lbl = gtk4::Label::new(Some(&format!(
        "Exemplo: [{}] {} -> {} (Status: {})",
        sample_tunnel.type_label(),
        sample_tunnel.display_source(),
        sample_tunnel.display_target(),
        sample_tunnel.status
    )));
    row_lbl.set_margin_top(10);
    row_lbl.set_margin_bottom(10);
    list.append(&row_lbl);
    vbox.append(&list);

    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Builds and presents the main `adw::ApplicationWindow`.
pub fn build_main_window(
    app: &adw::Application,
    initial_settings: AppSettings,
    initial_cwd: Option<String>,
) {
    let settings_rc = Rc::new(RefCell::new(initial_settings));
    let terminals_rc: Rc<RefCell<Vec<vte4::Terminal>>> = Rc::new(RefCell::new(Vec::new()));
    let active_tftp_server: Rc<RefCell<Option<UdpTftpServer>>> = Rc::new(RefCell::new(None));

    let style_manager = adw::StyleManager::default();
    style_manager.set_color_scheme(adw::ColorScheme::PreferDark);

    let css_provider = gtk4::CssProvider::new();
    {
        let st = settings_rc.borrow();
        let opacity_percent = 100u8.saturating_sub(st.transparency.min(100));
        let css_data = format!(
            "{}\n{}",
            generate_theme_css(&st.color_scheme, opacity_percent),
            menu_custom_css()
        );
        css_provider.load_from_string(&css_data);
    }
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("OnyxSH — Native Rust Terminal")
        .default_width(1280)
        .default_height(800)
        .build();

    let root_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // ── 1. HeaderBar ──────────────────────────────────────────────────────────
    let header_bar = adw::HeaderBar::new();

    let btn_sidebar = gtk4::ToggleButton::builder()
        .icon_name("sidebar-show-symbolic")
        .tooltip_text("Painel de Sessões")
        .active(true)
        .build();
    header_bar.pack_start(&btn_sidebar);

    let btn_fm = gtk4::ToggleButton::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text("Gerenciador de Arquivos Dual-Pane")
        .active(false)
        .build();
    header_bar.pack_start(&btn_fm);

    let btn_ai = gtk4::ToggleButton::builder()
        .icon_name("avatar-default-symbolic")
        .tooltip_text("Assistente de IA Zero-Trust")
        .active(false)
        .build();
    header_bar.pack_start(&btn_ai);

    // Phase 9: Search Toggle Button (Lupa)
    let btn_search = gtk4::ToggleButton::builder()
        .icon_name("edit-find-symbolic")
        .tooltip_text("Buscar no Terminal (Ctrl+Shift+F)")
        .active(false)
        .build();
    header_bar.pack_start(&btn_search);

    let btn_split = gtk4::Button::builder()
        .icon_name("view-dual-symbolic")
        .tooltip_text("Dividir Terminal Horizontalmente")
        .build();
    header_bar.pack_start(&btn_split);

    // Phase 13: Command Manager Dialog Button (`utilities-terminal-symbolic`)
    let btn_cmd_manager = gtk4::Button::builder()
        .icon_name("utilities-terminal-symbolic")
        .tooltip_text("Gerenciador de Comandos & Formulários Dinâmicos")
        .build();
    header_bar.pack_start(&btn_cmd_manager);

    // Phase 14: Real-Time Resource Monitor Button (`computer-symbolic`)
    let btn_resource_monitor = gtk4::Button::builder()
        .icon_name("computer-symbolic")
        .tooltip_text("Monitor de Recursos em Tempo Real (Sparklines Cairo)")
        .build();
    header_bar.pack_start(&btn_resource_monitor);

    // Phase 16: Autocomplete Assistant Button (`insert-text-symbolic`)
    let btn_autocomplete = gtk4::Button::builder()
        .icon_name("insert-text-symbolic")
        .tooltip_text("Autocompletar Inteligente (CompletionEngine)")
        .build();
    header_bar.pack_start(&btn_autocomplete);

    // Phase 16: Command Palette Button (`preferences-system-search-symbolic`)
    let btn_palette = gtk4::Button::builder()
        .icon_name("preferences-system-search-symbolic")
        .tooltip_text("Command Palette — Busca Unificada (Ctrl+Shift+P)")
        .build();
    header_bar.pack_start(&btn_palette);

    // Phase 10: Main Hamburger Menu Button (`open-menu-symbolic`)
    let btn_menu = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Menu Principal")
        .build();
    header_bar.pack_end(&btn_menu);

    let btn_new_tab = gtk4::Button::builder()
        .icon_name("tab-new-symbolic")
        .tooltip_text("Nova Aba de Terminal (Ctrl+Shift+T)")
        .build();
    header_bar.pack_end(&btn_new_tab);

    root_vbox.append(&header_bar);

    // ── 2. Phase 9: Terminal SearchBar (`Gtk.SearchBar` + `Aa`, `\b`, `.*` + Counter) ──
    let search_bar = gtk4::SearchBar::new();
    search_bar.set_show_close_button(true);

    let search_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    search_hbox.add_css_class("search-bar-box");

    let search_entry = gtk4::SearchEntry::new();
    search_entry.set_hexpand(true);
    search_entry.set_placeholder_text(Some("Buscar texto no terminal..."));
    search_bar.connect_entry(&search_entry);

    let search_occurrence_label = gtk4::Label::new(Some("0/0"));
    search_occurrence_label.add_css_class("dim-label");
    search_occurrence_label.add_css_class("monospace");
    search_occurrence_label.set_tooltip_text(Some("Ocorrências encontradas no terminal"));

    // Linked options box (`Aa`, `\b`, `.*`)
    let options_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    options_box.add_css_class("linked");

    let btn_case_sensitive = gtk4::ToggleButton::with_label("Aa");
    btn_case_sensitive.add_css_class("flat");
    btn_case_sensitive.set_tooltip_text(Some("Diferenciar Maiúsculas/Minúsculas (Match Case)"));
    btn_case_sensitive.set_active(settings_rc.borrow().search_case_sensitive);
    options_box.append(&btn_case_sensitive);

    let btn_whole_word = gtk4::ToggleButton::with_label(r"\b");
    btn_whole_word.add_css_class("flat");
    btn_whole_word.set_tooltip_text(Some("Palavra Inteira (Whole Word)"));
    btn_whole_word.set_active(settings_rc.borrow().search_whole_word);
    options_box.append(&btn_whole_word);

    let btn_regex = gtk4::ToggleButton::with_label(".*");
    btn_regex.add_css_class("flat");
    btn_regex.set_tooltip_text(Some("Usar Expressão Regular (Regex)"));
    btn_regex.set_active(settings_rc.borrow().search_use_regex);
    options_box.append(&btn_regex);

    let btn_search_prev = gtk4::Button::from_icon_name("go-up-symbolic");
    btn_search_prev.add_css_class("flat");
    btn_search_prev.set_tooltip_text(Some("Ocorrência Anterior (Shift+Enter)"));

    let btn_search_next = gtk4::Button::from_icon_name("go-down-symbolic");
    btn_search_next.add_css_class("flat");
    btn_search_next.set_tooltip_text(Some("Próxima Ocorrência (Enter)"));

    search_hbox.append(&search_entry);
    search_hbox.append(&search_occurrence_label);
    search_hbox.append(&options_box);
    search_hbox.append(&btn_search_prev);
    search_hbox.append(&btn_search_next);
    search_bar.set_child(Some(&search_hbox));
    root_vbox.append(&search_bar);

    // Synchronize Search Button <-> SearchBar search-mode-enabled
    {
        let sb = search_bar.clone();
        let se = search_entry.clone();
        btn_search.connect_toggled(move |btn| {
            let active = btn.is_active();
            if sb.is_search_mode() != active {
                sb.set_search_mode(active);
            }
            if active {
                se.grab_focus();
            }
        });
    }

    // ── 3. TabView + TabBar ───────────────────────────────────────────────────
    let tab_view = adw::TabView::new();
    tab_view.set_hexpand(true);
    tab_view.set_vexpand(true);

    let tab_bar = adw::TabBar::new();
    tab_bar.set_view(Some(&tab_view));
    root_vbox.append(&tab_bar);

    // Wire Search Execution State (`current_match_idx`, `total_matches`)
    let search_match_state = Rc::new(RefCell::new((0usize, 0usize)));
    let run_terminal_search: Rc<dyn Fn()> = {
        let tv = tab_view.clone();
        let se = search_entry.clone();
        let cs = btn_case_sensitive.clone();
        let ww = btn_whole_word.clone();
        let rx = btn_regex.clone();
        let lbl = search_occurrence_label.clone();
        let mstate = Rc::clone(&search_match_state);

        Rc::new(move || {
            let query = se.text().to_string();
            if let Some(term) = get_active_vte_terminal(&tv) {
                let total = apply_vte_search(
                    &term,
                    &query,
                    cs.is_active(),
                    ww.is_active(),
                    rx.is_active(),
                );
                *mstate.borrow_mut() = (0, total);
                lbl.set_text(&format_occurrence_counter(0, total));
            } else {
                *mstate.borrow_mut() = (0, 0);
                lbl.set_text("0/0");
            }
        })
    };

    {
        let run = Rc::clone(&run_terminal_search);
        search_entry.connect_search_changed(move |_| run());
    }
    {
        let run = Rc::clone(&run_terminal_search);
        btn_case_sensitive.connect_toggled(move |_| run());
    }
    {
        let run = Rc::clone(&run_terminal_search);
        btn_whole_word.connect_toggled(move |_| run());
    }
    {
        let run = Rc::clone(&run_terminal_search);
        btn_regex.connect_toggled(move |_| run());
    }
    {
        let tv = tab_view.clone();
        let lbl = search_occurrence_label.clone();
        let mstate = Rc::clone(&search_match_state);
        let next_fn = move || {
            if let Some(term) = get_active_vte_terminal(&tv) {
                term.search_find_next();
                let mut st = mstate.borrow_mut();
                if st.1 > 0 {
                    st.0 = (st.0 + 1) % st.1;
                }
                lbl.set_text(&format_occurrence_counter(st.0, st.1));
            }
        };
        let next_fn_rc = Rc::new(next_fn);
        let nf1 = Rc::clone(&next_fn_rc);
        btn_search_next.connect_clicked(move |_| nf1());
        let nf2 = Rc::clone(&next_fn_rc);
        search_entry.connect_activate(move |_| nf2());
    }
    {
        let tv = tab_view.clone();
        let lbl = search_occurrence_label.clone();
        let mstate = Rc::clone(&search_match_state);
        btn_search_prev.connect_clicked(move |_| {
            if let Some(term) = get_active_vte_terminal(&tv) {
                term.search_find_previous();
                let mut st = mstate.borrow_mut();
                if st.1 > 0 {
                    st.0 = if st.0 == 0 { st.1 - 1 } else { st.0 - 1 };
                }
                lbl.set_text(&format_occurrence_counter(st.0, st.1));
            }
        });
    }
    {
        let bs = btn_search.clone();
        let tv = tab_view.clone();
        search_bar.connect_search_mode_enabled_notify(move |sb| {
            let enabled = sb.is_search_mode();
            if bs.is_active() != enabled {
                bs.set_active(enabled);
            }
            if !enabled {
                if let Some(term) = get_active_vte_terminal(&tv) {
                    term.search_set_regex(None, 0);
                    term.unselect_all();
                }
            }
        });
    }

    // ── 4. Footer Telemetry Bar (`onyxsh-system`) ─────────────────────────────
    let footer_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
    footer_box.set_margin_start(12);
    footer_box.set_margin_end(12);
    footer_box.set_margin_top(4);
    footer_box.set_margin_bottom(4);

    let status_lbl = gtk4::Label::new(Some("OnyxSH Rust Engine v0.14.0 — Ready"));
    status_lbl.add_css_class("caption");
    status_lbl.set_halign(gtk4::Align::Start);
    status_lbl.set_hexpand(true);

    let metrics_lbl = gtk4::Label::new(Some("CPU: 0.0% | RAM: -- / -- MB"));
    metrics_lbl.add_css_class("caption");
    metrics_lbl.set_halign(gtk4::Align::End);

    footer_box.append(&status_lbl);
    footer_box.append(&metrics_lbl);

    // ── 5. Phase 11: Complete Session Sidebar (5-Button CRUD + Folder Tree + Search) ──
    let spawn_session_cb: Rc<dyn Fn(&SessionItem, bool)> = {
        let tv = tab_view.clone();
        let st_rc = Rc::clone(&settings_rc);
        let terms_rc = Rc::clone(&terminals_rc);
        let status = status_lbl.clone();

        Rc::new(move |sess: &SessionItem, in_split: bool| {
            let st = st_rc.borrow();
            let make_term = || {
                if sess.session_type == "ssh" && !sess.host.is_empty() {
                    let port_str = sess.port.to_string();
                    let target = if sess.user.is_empty() {
                        sess.host.clone()
                    } else {
                        format!("{}@{}", sess.user, sess.host)
                    };
                    let mut args = vec!["ssh".to_string(), "-p".to_string(), port_str];
                    if sess.x11_forwarding {
                        args.push("-X".to_string());
                    }
                    args.push(target);
                    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                    create_vte_terminal(&st, None, &arg_refs, &terms_rc)
                } else {
                    let cwd = if sess.local_working_directory.is_empty() {
                        None
                    } else {
                        Some(sess.local_working_directory.as_str())
                    };
                    create_vte_terminal(&st, cwd, &[], &terms_rc)
                }
            };

            if in_split {
                let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
                paned.set_wide_handle(true);
                let t1 = make_term();
                let t2 = make_term();
                paned.set_start_child(Some(&t1));
                paned.set_end_child(Some(&t2));
                let page = tv.append(&paned);
                page.set_title(&format!("{} (Split)", sess.name));
                tv.set_selected_page(&page);
            } else {
                let term = make_term();
                let page = tv.append(&term);
                let title = if sess.is_production {
                    format!("🔴 [PROD] {}", sess.name)
                } else {
                    sess.name.clone()
                };
                page.set_title(&title);
                tv.set_selected_page(&page);
            }
            status.set_text(&format!("Conectado à sessão: {}", sess.name));
        })
    };

    let (sidebar_box, populate_sessions_sidebar) =
        crate::session_sidebar::build_session_sidebar(
            &window,
            &tab_view,
            &status_lbl,
            spawn_session_cb,
        );

    // Phase 13: Command Toolbar (Pinned Buttons) + Broadcast Bar
    let cmd_buttons_rc = Rc::new(RefCell::new(
        crate::command_manager_ui::load_command_buttons(),
    ));
    let active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>> = {
        let tv = tab_view.clone();
        Rc::new(move || get_active_vte_terminal(&tv))
    };
    let (cmd_and_broadcast_bar, rebuild_cmd_toolbar, _btn_broadcast_toggle) =
        crate::command_manager_ui::build_command_and_broadcast_bars(
            &window,
            Rc::clone(&cmd_buttons_rc),
            Rc::clone(&terminals_rc),
            Rc::clone(&active_term_cb),
            &status_lbl,
        );

    {
        let win_ref = window.clone();
        let btns = Rc::clone(&cmd_buttons_rc);
        let rebuild = Rc::clone(&rebuild_cmd_toolbar);
        let terms_all = Rc::clone(&terminals_rc);
        let act_fn = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();

        btn_cmd_manager.connect_clicked(move |_| {
            let terms_inner = Rc::clone(&terms_all);
            let act_inner = Rc::clone(&act_fn);
            let status_inner = status.clone();
            let disp: Rc<dyn Fn(&str, bool, bool)> =
                Rc::new(move |cmd_str: &str, exec: bool, broadcast: bool| {
                    let payload = if exec {
                        format!("{cmd_str}\n")
                    } else {
                        cmd_str.to_string()
                    };
                    if broadcast {
                        for t in terms_inner.borrow().iter() {
                            t.feed_child(payload.as_bytes());
                        }
                        status_inner.set_text(&format!("Broadcast enviado: {cmd_str}"));
                    } else if let Some(t) = act_inner() {
                        t.feed_child(payload.as_bytes());
                        status_inner.set_text(&format!("Comando enviado: {cmd_str}"));
                    }
                });
            crate::command_manager_ui::show_command_manager_dialog(
                &win_ref,
                Rc::clone(&btns),
                Rc::clone(&rebuild),
                disp,
            );
        });
    }

    // Phase 14: Resource Monitor Dashboard Dialog
    {
        let win_ref = window.clone();
        btn_resource_monitor.connect_clicked(move |_| {
            crate::resource_dashboard::show_resource_dashboard_dialog(&win_ref);
        });
    }

    // Phase 16: Production Warning Banner + Toggle Button in HeaderBar
    let (prod_banner, btn_prod_shield) = crate::terminal_ux::build_production_banner(
        &window,
        Rc::clone(&active_term_cb),
        &status_lbl,
    );
    header_bar.pack_start(&btn_prod_shield);

    // Phase 16: Autocomplete Assistant Dialog
    {
        let win_ref = window.clone();
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();
        btn_autocomplete.connect_clicked(move |_| {
            crate::terminal_ux::show_autocomplete_assistant_dialog(
                &win_ref,
                Rc::clone(&act_term),
                &status,
            );
        });
    }

    // Center + Bottom File Manager Vertical Box
    let center_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    center_vbox.set_hexpand(true);
    center_vbox.set_vexpand(true);
    center_vbox.append(&prod_banner);
    center_vbox.append(&cmd_and_broadcast_bar);
    center_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    center_vbox.append(&tab_view);

    let cd_in_terminal_cb: Rc<dyn Fn(&std::path::Path)> = {
        let tv = tab_view.clone();
        let status = status_lbl.clone();
        Rc::new(move |dir_path: &std::path::Path| {
            if let Some(term) = get_active_vte_terminal(&tv) {
                let escaped = dir_path.to_string_lossy().replace('\'', "'\\''");
                let cmd = format!("cd '{escaped}'\n");
                term.feed_child(cmd.as_bytes());
                status.set_text(&format!("Diretório enviado ao terminal: {}", dir_path.display()));
            }
        })
    };

    let fm_widget = crate::filemanager_ui::build_complete_file_manager(
        &window,
        &status_lbl,
        cd_in_terminal_cb,
    );
    fm_widget.set_visible(false);
    center_vbox.append(&fm_widget);

    // Phase 15: Complete Zero-Trust AI Assistant Right Sidebar Panel
    let (ai_widget, submit_ai_prompt_cb) =
        crate::ai_assistant_ui::build_complete_ai_assistant_panel(
            &window,
            &settings_rc,
            Rc::clone(&active_term_cb),
            &status_lbl,
        );
    ai_widget.set_visible(false);

    // Main Horizontal Layout
    let main_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    main_hbox.set_hexpand(true);
    main_hbox.set_vexpand(true);
    main_hbox.append(&sidebar_box);
    main_hbox.append(&gtk4::Separator::new(gtk4::Orientation::Vertical));
    main_hbox.append(&center_vbox);
    main_hbox.append(&ai_widget);

    root_vbox.append(&main_hbox);
    root_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    root_vbox.append(&footer_box);

    // ── 6. Phase 10: Build Main Hamburger Menu Popover (`ui/menus.py`) ────────
    {
        let menu_popover = gtk4::Popover::new();
        menu_popover.set_has_arrow(true);
        let menu_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        menu_vbox.set_width_request(260);
        menu_vbox.set_margin_start(10);
        menu_vbox.set_margin_end(10);
        menu_vbox.set_margin_top(8);
        menu_vbox.set_margin_bottom(8);

        // 6.1 ThemeSelectorWidget (4 modes: System, Light, Dark, Terminal)
        let theme_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 16);
        theme_hbox.add_css_class("themeselector");
        theme_hbox.set_halign(gtk4::Align::Center);

        let rb_system = gtk4::CheckButton::builder()
            .tooltip_text("Seguir Estilo do Sistema")
            .build();
        rb_system.add_css_class("theme-selector");

        let rb_light = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text("Estilo Claro (Light)")
            .build();
        rb_light.add_css_class("theme-selector");

        let rb_dark = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text("Estilo Escuro (Dark)")
            .active(true)
            .build();
        rb_dark.add_css_class("theme-selector");

        let rb_terminal = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text("Combinar com Cores do Terminal")
            .build();
        rb_terminal.add_css_class("theme-selector");

        {
            let st = Rc::clone(&settings_rc);
            rb_system.connect_toggled(move |btn| {
                if btn.is_active() {
                    let pal = get_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::System, is_light_hex_color(pal.background));
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_light.connect_toggled(move |btn| {
                if btn.is_active() {
                    let pal = get_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::Light, is_light_hex_color(pal.background));
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_dark.connect_toggled(move |btn| {
                if btn.is_active() {
                    let pal = get_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::Dark, is_light_hex_color(pal.background));
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_terminal.connect_toggled(move |btn| {
                if btn.is_active() {
                    let pal = get_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(
                        GtkThemeMode::TerminalMatch,
                        is_light_hex_color(pal.background),
                    );
                }
            });
        }

        theme_hbox.append(&rb_system);
        theme_hbox.append(&rb_light);
        theme_hbox.append(&rb_dark);
        theme_hbox.append(&rb_terminal);
        menu_vbox.append(&theme_hbox);

        // 6.2 FontSizerWidget (`-`, `12 pt`, `+`)
        let font_center = gtk4::CenterBox::new();
        font_center.add_css_class("font-sizer");
        let zoom_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 14);

        let btn_zoom_out = gtk4::Button::from_icon_name("zoom-out-symbolic");
        btn_zoom_out.add_css_class("flat");
        btn_zoom_out.set_tooltip_text(Some("Diminuir Fonte (Ctrl+-)"));

        let (_, initial_pt) = parse_font_string(&settings_rc.borrow().font);
        let font_size_lbl = gtk4::Label::new(Some(&format!("{initial_pt} pt")));
        font_size_lbl.set_width_chars(6);
        let btn_zoom_reset = gtk4::Button::new();
        btn_zoom_reset.add_css_class("flat");
        btn_zoom_reset.set_tooltip_text(Some("Resetar Tamanho da Fonte (12 pt)"));
        btn_zoom_reset.set_child(Some(&font_size_lbl));

        let btn_zoom_in = gtk4::Button::from_icon_name("zoom-in-symbolic");
        btn_zoom_in.add_css_class("flat");
        btn_zoom_in.set_tooltip_text(Some("Aumentar Fonte (Ctrl++)"));

        let update_all_fonts = {
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let lbl = font_size_lbl.clone();
            Rc::new(move |delta: Option<i32>| {
                let mut st = st_rc.borrow_mut();
                let (new_font, new_pt) = match delta {
                    Some(d) => change_font_size(&st.font, d),
                    None => ("Monospace 12".to_string(), 12),
                };
                st.font = new_font;
                lbl.set_text(&format!("{new_pt} pt"));
                for term in terms_rc.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                let cfg_path = platform::config_dir().join("settings.json");
                let _ = st.save_to_path(&cfg_path);
            })
        };

        {
            let uf = Rc::clone(&update_all_fonts);
            btn_zoom_out.connect_clicked(move |_| uf(Some(-1)));
        }
        {
            let uf = Rc::clone(&update_all_fonts);
            btn_zoom_reset.connect_clicked(move |_| uf(None));
        }
        {
            let uf = Rc::clone(&update_all_fonts);
            btn_zoom_in.connect_clicked(move |_| uf(Some(1)));
        }

        zoom_box.append(&btn_zoom_out);
        zoom_box.append(&btn_zoom_reset);
        zoom_box.append(&btn_zoom_in);
        font_center.set_center_widget(Some(&zoom_box));
        menu_vbox.append(&font_center);
        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        // Helper to create flat menu action buttons
        let make_menu_btn = |label: &str, icon: Option<&str>| -> gtk4::Button {
            let btn = gtk4::Button::new();
            btn.add_css_class("flat");
            let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            if let Some(ic) = icon {
                hbox.append(&gtk4::Image::from_icon_name(ic));
            }
            let lbl = gtk4::Label::new(Some(label));
            lbl.set_halign(gtk4::Align::Start);
            lbl.set_hexpand(true);
            hbox.append(&lbl);
            btn.set_child(Some(&hbox));
            btn
        };

        // 1. Nova Janela
        let mi_new_win = make_menu_btn("Nova Janela", Some("window-new-symbolic"));
        {
            let app_clone = app.clone();
            let st_rc = Rc::clone(&settings_rc);
            let pop = menu_popover.clone();
            mi_new_win.connect_clicked(move |_| {
                pop.popdown();
                let st = st_rc.borrow().clone();
                build_main_window(&app_clone, st, None);
            });
        }
        menu_vbox.append(&mi_new_win);

        // 2. Exportar Saída do Terminal
        let mi_export = make_menu_btn(
            "Exportar Saída do Terminal...",
            Some("document-save-symbolic"),
        );
        {
            let tv = tab_view.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_export.connect_clicked(move |_| {
                pop.popdown();
                if let Some(term) = get_active_vte_terminal(&tv) {
                    let raw = term
                        .text_format(vte4::Format::Text)
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let exported = export_markdown(&raw, &ExportMetadata::default());
                    let out_path = platform::home_dir().join("onyxsh_terminal_export.md");
                    if std::fs::write(&out_path, exported).is_ok() {
                        status.set_text(&format!(
                            "Saída do terminal exportada para: {}",
                            out_path.display()
                        ));
                    }
                }
            });
        }
        menu_vbox.append(&mi_export);

        // 3. Preferências
        let mi_prefs = make_menu_btn("Preferências", Some("preferences-system-symbolic"));
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let css_prov = css_provider.clone();
            let pop = menu_popover.clone();
            mi_prefs.connect_clicked(move |_| {
                pop.popdown();
                show_preferences_dialog(
                    &win_clone,
                    Rc::clone(&st_rc),
                    Rc::clone(&terms_rc),
                    css_prov.clone(),
                );
            });
        }
        menu_vbox.append(&mi_prefs);

        // 4. Cores (Alternar Paleta do Terminal)
        let mi_colors = make_menu_btn(
            "Cores (Alternar Paleta)",
            Some("format-text-highlight-symbolic"),
        );
        {
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let css_prov = css_provider.clone();
            let status = status_lbl.clone();
            mi_colors.connect_clicked(move |_| {
                let mut st = st_rc.borrow_mut();
                st.color_scheme = match st.color_scheme.as_str() {
                    "onyx_dark" => "dracula".to_string(),
                    "dracula" => "solarized_light".to_string(),
                    _ => "onyx_dark".to_string(),
                };
                let opacity_pct = 100u8.saturating_sub(st.transparency.min(100));
                let css = format!(
                    "{}\n{}",
                    generate_theme_css(&st.color_scheme, opacity_pct),
                    menu_custom_css()
                );
                css_prov.load_from_string(&css);
                for term in terms_rc.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                status.set_text(&format!("Paleta ativa: {}", st.color_scheme));
            });
        }
        menu_vbox.append(&mi_colors);

        // 5. Assistente de IA
        let mi_ai = make_menu_btn("Assistente de IA", Some("avatar-default-symbolic"));
        {
            let bai = btn_ai.clone();
            let pop = menu_popover.clone();
            mi_ai.connect_clicked(move |_| {
                pop.popdown();
                bai.set_active(!bai.is_active());
            });
        }
        menu_vbox.append(&mi_ai);

        // 6. Atalhos de Teclado
        let mi_shortcuts = make_menu_btn(
            "Atalhos de Teclado",
            Some("preferences-desktop-keyboard-shortcuts-symbolic"),
        );
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let pop = menu_popover.clone();
            mi_shortcuts.connect_clicked(move |_| {
                pop.popdown();
                show_shortcuts_dialog(&win_clone, &st_rc.borrow());
            });
        }
        menu_vbox.append(&mi_shortcuts);

        // 7. Importar Sessões (~/.ssh/config)
        let mi_import = make_menu_btn(
            "Importar Sessões (~/.ssh/config)",
            Some("document-open-symbolic"),
        );
        {
            let pop_sess = Rc::clone(&populate_sessions_sidebar);
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_import.connect_clicked(move |_| {
                pop.popdown();
                pop_sess();
                status.set_text("Sessões sincronizadas a partir de ~/.config/onyxsh/sessions.json e ~/.ssh/config.");
            });
        }
        menu_vbox.append(&mi_import);

        // 8. Gerenciador de Túneis SSH
        let mi_tunnels = make_menu_btn(
            "Gerenciador de Túneis SSH",
            Some("network-vpn-symbolic"),
        );
        {
            let win_clone = window.clone();
            let pop = menu_popover.clone();
            mi_tunnels.connect_clicked(move |_| {
                pop.popdown();
                show_tunnel_manager_dialog(&win_clone);
            });
        }
        menu_vbox.append(&mi_tunnels);

        // 9. Iniciar / Parar Servidor TFTP
        let tftp_lbl = gtk4::Label::new(Some("Iniciar Servidor TFTP"));
        tftp_lbl.set_halign(gtk4::Align::Start);
        tftp_lbl.set_hexpand(true);
        let mi_tftp = gtk4::Button::new();
        mi_tftp.add_css_class("flat");
        let tftp_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        tftp_hbox.append(&gtk4::Image::from_icon_name("network-transmit-receive-symbolic"));
        tftp_hbox.append(&tftp_lbl);
        mi_tftp.set_child(Some(&tftp_hbox));
        {
            let tftp_rc = Rc::clone(&active_tftp_server);
            let lbl_clone = tftp_lbl.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_tftp.connect_clicked(move |_| {
                pop.popdown();
                let mut guard = tftp_rc.borrow_mut();
                if guard.is_some() {
                    *guard = None;
                    lbl_clone.set_text("Iniciar Servidor TFTP");
                    status.set_text("Servidor TFTP encerrado.");
                } else {
                    let root = platform::home_dir();
                    if let Ok(srv) = UdpTftpServer::bind("127.0.0.1:0", &root) {
                        let addr = srv
                            .local_addr()
                            .map(|a| a.to_string())
                            .unwrap_or_else(|_| "127.0.0.1:6969".to_string());
                        lbl_clone.set_text(&format!("Parar Servidor TFTP ({addr})"));
                        status.set_text(&format!(
                            "Servidor TFTP Read-Only ativo em {addr} (raiz: {})",
                            root.display()
                        ));
                        *guard = Some(srv);
                    }
                }
            });
        }
        menu_vbox.append(&mi_tftp);

        // 10. Sobre
        let mi_about = make_menu_btn("Sobre o OnyxSH", Some("help-about-symbolic"));
        {
            let win_clone = window.clone();
            let pop = menu_popover.clone();
            mi_about.connect_clicked(move |_| {
                pop.popdown();
                let about = adw::AboutWindow::builder()
                    .transient_for(&win_clone)
                    .application_name(APP_NAME)
                    .application_icon(APP_ID)
                    .version(APP_VERSION)
                    .developer_name("Leonardo Berbert")
                    .license_type(gtk4::License::Gpl30)
                    .comments(
                        "Emulador de Terminal Nativo em Rust (GTK4 + Libadwaita + VTE4) \
                         com ProductionGuard, Dual-Pane SFTP/TFTP e Agente de IA Zero-Trust.",
                    )
                    .build();
                about.present();
            });
        }
        menu_vbox.append(&mi_about);

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        // 11. Sair
        let mi_quit = make_menu_btn("Sair", Some("application-exit-symbolic"));
        {
            let app_clone = app.clone();
            mi_quit.connect_clicked(move |_| {
                app_clone.quit();
            });
        }
        menu_vbox.append(&mi_quit);

        menu_popover.set_child(Some(&menu_vbox));
        btn_menu.set_popover(Some(&menu_popover));
    }

    // ── 7. Spawn Initial Terminal Tab & Wire Window Shortcuts / Buttons ───────
    let initial_term = create_vte_terminal(
        &settings_rc.borrow(),
        initial_cwd.as_deref(),
        &[],
        &terminals_rc,
    );
    crate::terminal_ux::attach_vte_ux_controllers(
        &initial_term,
        &window,
        &ai_widget,
        &btn_ai,
        Rc::clone(&submit_ai_prompt_cb),
        &status_lbl,
    );
    let first_page = tab_view.append(&initial_term);
    first_page.set_title("Terminal 1");
    tab_view.set_selected_page(&first_page);

    {
        let sb = sidebar_box.clone();
        btn_sidebar.connect_toggled(move |btn| {
            sb.set_visible(btn.is_active());
        });
    }
    {
        let fm = fm_widget.clone();
        btn_fm.connect_toggled(move |btn| {
            fm.set_visible(btn.is_active());
        });
    }
    {
        let ai = ai_widget.clone();
        btn_ai.connect_toggled(move |btn| {
            ai.set_visible(btn.is_active());
        });
    }
    {
        let tv = tab_view.clone();
        let st = Rc::clone(&settings_rc);
        let terms = Rc::clone(&terminals_rc);
        let win_ref = window.clone();
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub_ai = Rc::clone(&submit_ai_prompt_cb);
        let status = status_lbl.clone();
        let counter = Rc::new(RefCell::new(1u32));
        btn_new_tab.connect_clicked(move |_| {
            let mut n = counter.borrow_mut();
            *n += 1;
            let term = create_vte_terminal(&st.borrow(), None, &[], &terms);
            crate::terminal_ux::attach_vte_ux_controllers(
                &term,
                &win_ref,
                &ai_w,
                &ai_b,
                Rc::clone(&sub_ai),
                &status,
            );
            let page = tv.append(&term);
            page.set_title(&format!("Terminal {}", *n));
            tv.set_selected_page(&page);
        });
    }
    {
        let tv = tab_view.clone();
        let st = Rc::clone(&settings_rc);
        let terms = Rc::clone(&terminals_rc);
        let win_ref = window.clone();
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub_ai = Rc::clone(&submit_ai_prompt_cb);
        let status = status_lbl.clone();
        btn_split.connect_clicked(move |_| {
            let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
            paned.set_wide_handle(true);
            let t1 = create_vte_terminal(&st.borrow(), None, &[], &terms);
            let t2 = create_vte_terminal(&st.borrow(), None, &[], &terms);
            crate::terminal_ux::attach_vte_ux_controllers(
                &t1,
                &win_ref,
                &ai_w,
                &ai_b,
                Rc::clone(&sub_ai),
                &status,
            );
            crate::terminal_ux::attach_vte_ux_controllers(
                &t2,
                &win_ref,
                &ai_w,
                &ai_b,
                Rc::clone(&sub_ai),
                &status,
            );
            paned.set_start_child(Some(&t1));
            paned.set_end_child(Some(&t2));
            let page = tv.append(&paned);
            page.set_title("Split Terminal");
            tv.set_selected_page(&page);
        });
    }

    // Phase 16: Unified Command Palette (`Ctrl+Shift+P`)
    {
        let win_ref = window.clone();
        let cmds_rc = Rc::clone(&cmd_buttons_rc);
        let b_nt = btn_new_tab.clone();
        let b_sp = btn_split.clone();
        let b_fm = btn_fm.clone();
        let b_ai = btn_ai.clone();
        let b_rm = btn_resource_monitor.clone();
        let b_ac = btn_autocomplete.clone();
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();

        btn_palette.connect_clicked(move |_| {
            let b_nt_c = b_nt.clone();
            let b_sp_c = b_sp.clone();
            let b_fm_c = b_fm.clone();
            let b_ai_c = b_ai.clone();
            let b_rm_c = b_rm.clone();
            let b_ac_c = b_ac.clone();
            let st_act = status.clone();
            let at_act = Rc::clone(&act_term);

            let on_act: Rc<dyn Fn(&str)> = Rc::new(move |act: &str| match act {
                "new-tab" => b_nt_c.emit_clicked(),
                "split" => b_sp_c.emit_clicked(),
                "toggle-fm" => b_fm_c.set_active(!b_fm_c.is_active()),
                "toggle-ai" => b_ai_c.set_active(!b_ai_c.is_active()),
                "resource-monitor" => b_rm_c.emit_clicked(),
                "autocomplete" => b_ac_c.emit_clicked(),
                "git-assistant" => {
                    let cwd = std::env::current_dir().unwrap_or_else(|_| platform::home_dir());
                    if let Some(git_sum) =
                        onyxsh_terminal::git_assistant::inspect_git_repository(&cwd)
                    {
                        let commit_msg =
                            onyxsh_terminal::git_assistant::suggest_conventional_commit(&git_sum);
                        if let Some(term) = at_act() {
                            let cmd = format!("git commit -m \"{commit_msg}\"");
                            term.feed_child(cmd.as_bytes());
                        }
                        st_act.set_text(&format!("Git Assistant: {commit_msg}"));
                    } else {
                        st_act.set_text("Git Assistant: diretório atual não é um repositório Git.");
                    }
                }
                "backup-config" => {
                    let cfg_dir = platform::config_dir();
                    let out_path = platform::home_dir().join("onyxsh_config_backup.json");
                    match onyxsh_core::backup::create_config_backup(&cfg_dir, &out_path) {
                        Ok(bundle) => st_act.set_text(&format!(
                            "Backup SHA-256 criado ({} arquivos): {}",
                            bundle.files.len(),
                            out_path.display()
                        )),
                        Err(e) => st_act.set_text(&format!("Erro ao criar backup: {e}")),
                    }
                }
                _ => {}
            });

            let at_1 = Rc::clone(&act_term);
            let st_1 = status.clone();
            let on_sess: Rc<dyn Fn(&SessionItem)> = Rc::new(move |sess: &SessionItem| {
                if let Some(term) = at_1() {
                    if sess.session_type == "ssh" && !sess.host.is_empty() {
                        let cmd = format!("ssh -p {} {}\n", sess.port, sess.connection_string());
                        term.feed_child(cmd.as_bytes());
                    }
                    st_1.set_text(&format!("Conectando via Command Palette: {}", sess.name));
                }
            });

            let at_2 = Rc::clone(&act_term);
            let st_2 = status.clone();
            let on_cmd: Rc<dyn Fn(&str)> = Rc::new(move |cmd_tpl: &str| {
                if let Some(term) = at_2() {
                    term.feed_child(format!("{cmd_tpl}\n").as_bytes());
                    st_2.set_text(&format!("Comando executado via Palette: {cmd_tpl}"));
                }
            });

            crate::terminal_ux::show_command_palette_dialog(
                &win_ref,
                &cmds_rc.borrow(),
                on_act,
                on_sess,
                on_cmd,
            );
        });
    }

    // Window-level keyboard controller for Ctrl+Shift+F (Find), Ctrl+Shift+T (New Tab), Ctrl+Shift+P (Palette)
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        let bs = btn_search.clone();
        let bnt = btn_new_tab.clone();
        let bp = btn_palette.clone();
        key_ctrl.connect_key_pressed(move |_, keyval, _, state| {
            let ctrl_shift = state.contains(
                gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
            );
            if ctrl_shift && (keyval == gdk::Key::F || keyval == gdk::Key::f) {
                bs.set_active(!bs.is_active());
                return glib::Propagation::Stop;
            }
            if ctrl_shift && (keyval == gdk::Key::T || keyval == gdk::Key::t) {
                bnt.emit_clicked();
                return glib::Propagation::Stop;
            }
            if ctrl_shift && (keyval == gdk::Key::P || keyval == gdk::Key::p) {
                bp.emit_clicked();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(key_ctrl);
    }

    // Live `/proc` Telemetry Polling Timer (every 2 seconds)
    let prev_cpu = Rc::new(RefCell::new(
        std::fs::read_to_string("/proc/stat")
            .ok()
            .and_then(|s| parse_proc_stat(&s))
            .unwrap_or_default(),
    ));
    let metrics_lbl_clone = metrics_lbl.clone();
    glib::timeout_add_seconds_local(2, move || {
        let cpu_pct = if let Ok(stat_str) = std::fs::read_to_string("/proc/stat") {
            if let Some(curr) = parse_proc_stat(&stat_str) {
                let mut prev = prev_cpu.borrow_mut();
                let pct = calculate_cpu_usage_percent(&prev, &curr);
                *prev = curr;
                pct
            } else {
                0.0
            }
        } else {
            0.0
        };

        let mem = std::fs::read_to_string("/proc/meminfo")
            .map(|s| parse_proc_meminfo(&s))
            .unwrap_or_default();
        let net = std::fs::read_to_string("/proc/net/dev")
            .map(|s| parse_proc_net_dev(&s))
            .unwrap_or_default();

        let used_mb = mem.ram_used_bytes / (1024 * 1024);
        let total_mb = mem.ram_total_bytes / (1024 * 1024);
        let rx_mb = net.rx_bytes / (1024 * 1024);
        let tx_mb = net.tx_bytes / (1024 * 1024);

        metrics_lbl_clone.set_text(&format!(
            "CPU: {cpu_pct:.1}% | RAM: {used_mb}/{total_mb} MB ({:.0}%) | NET: ↓{rx_mb}MB ↑{tx_mb}MB",
            mem.ram_percent
        ));
        glib::ControlFlow::Continue
    });

    window.set_content(Some(&root_vbox));
    window.present();
}
