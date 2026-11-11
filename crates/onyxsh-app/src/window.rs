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
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform;
use onyxsh_core::sessions::SessionItem;
use onyxsh_core::{
    APP_ID, APP_NAME, APP_VERSION, COPYRIGHT, DEVELOPERS, DEVELOPER_NAME, ISSUE_URL, WEBSITE,
};
use onyxsh_filemanager::backend::UdpTftpServer;
use onyxsh_system::{
    calculate_cpu_usage_percent, parse_proc_meminfo, parse_proc_net_dev, parse_proc_stat,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use vte4::prelude::*;

/// Applies the palette, font, cursor, and VTE options from `AppSettings` to an existing `vte4::Terminal`.
fn apply_terminal_appearance(term: &vte4::Terminal, settings: &AppSettings) {
    crate::preferences_ui::apply_all_settings_to_terminal(term, settings);
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

/// Opens the SSH Tunnel Manager dialog (`win.show-tunnel-manager`).
fn show_tunnel_manager_dialog(parent: &adw::ApplicationWindow, status_label: &gtk4::Label) {
    crate::tunnel_ui::show_tunnel_manager_window(parent, status_label);
}

/// Registers the terminal context menu actions on the window and configures accelerators
/// matching `CORRIGIR/menucontext3.jpg`, `src/onyxsh/ui/menus.py` and `src/onyxsh/app.py`.
fn setup_terminal_window_actions(
    app: &adw::Application,
    window: &adw::ApplicationWindow,
    tab_view: &adw::TabView,
    active_term_cb: &Rc<dyn Fn() -> Option<vte4::Terminal>>,
    ai_widget: &gtk4::Box,
    btn_ai: &gtk4::ToggleButton,
    submit_ai_prompt_cb: &Rc<dyn Fn(&str)>,
    btn_split: &gtk4::Button,
    settings_rc: &Rc<RefCell<AppSettings>>,
    status_lbl: &gtk4::Label,
    clicked_url_rc: &Rc<RefCell<Option<String>>>,
) {
    // 1. Copy (`win.copy`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("copy", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                term.copy_clipboard_format(vte4::Format::Text);
                st.set_text(&tr("Seleção copiada."));
            }
        });
        window.add_action(&action);
    }

    // 2. Paste (`win.paste`)
    {
        let act = Rc::clone(active_term_cb);
        let action = gio::SimpleAction::new("paste", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                term.paste_clipboard();
            }
        });
        window.add_action(&action);
    }

    // 3. Select All (`win.select-all`)
    {
        let act = Rc::clone(active_term_cb);
        let action = gio::SimpleAction::new("select-all", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                term.select_all();
            }
        });
        window.add_action(&action);
    }

    // 4. Export Terminal Buffer (`win.export-terminal-buffer`)
    {
        let win_ref = window.clone();
        let act = Rc::clone(active_term_cb);
        let st_lbl = status_lbl.clone();
        let action = gio::SimpleAction::new("export-terminal-buffer", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                crate::export_dialog::show_export_terminal_dialog(
                    &win_ref, &term, None, false, &st_lbl,
                );
            }
        });
        window.add_action(&action);
    }

    // 5. Export Terminal Runbook (`win.export-terminal-runbook`)
    {
        let win_ref = window.clone();
        let act = Rc::clone(active_term_cb);
        let st_lbl = status_lbl.clone();
        let action = gio::SimpleAction::new("export-terminal-runbook", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                crate::export_dialog::show_export_terminal_dialog(
                    &win_ref, &term, None, true, &st_lbl,
                );
            }
        });
        window.add_action(&action);
    }

    // 6. Clear Session (`win.clear-session`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("clear-session", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                term.reset(true, true);
                term.feed_child(b"\x0c");
                st.set_text(&tr("Sessão limpa."));
            }
        });
        window.add_action(&action);
    }

    // 7. Jump Previous Prompt (`win.jump-previous-prompt`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("jump-previous-prompt", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                if let Some(adj) = term.vadjustment() {
                    let step = adj.step_increment() * 5.0;
                    let new_val = (adj.value() - step).max(adj.lower());
                    adj.set_value(new_val);
                    st.set_text(&tr("Pular para o prompt anterior."));
                }
            }
        });
        window.add_action(&action);
    }

    // 8. Jump Next Prompt (`win.jump-next-prompt`)
    {
        let act = Rc::clone(active_term_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("jump-next-prompt", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                if let Some(adj) = term.vadjustment() {
                    let step = adj.step_increment() * 5.0;
                    let new_val = (adj.value() + step).min(adj.upper() - adj.page_size());
                    adj.set_value(new_val);
                    st.set_text(&tr("Pular para o próximo prompt."));
                }
            }
        });
        window.add_action(&action);
    }

    // 9. Resolve Error with OpenCode (`win.resolve-error-opencode`)
    {
        let act = Rc::clone(active_term_cb);
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub_ai = Rc::clone(submit_ai_prompt_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("resolve-error-opencode", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                let text = crate::global_search_and_shortcuts::extract_vte_text(&term);
                let recent_lines = text
                    .lines()
                    .rev()
                    .take(25)
                    .collect::<Vec<&str>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<&str>>()
                    .join("\n");
                let prompt = if recent_lines.trim().is_empty() {
                    tr("Analise o projeto e elabore as correções necessárias com OpenCode.")
                } else {
                    tr_fmt(
                        "O comando executado no terminal falhou. Analise os arquivos do projeto e elabore as correções necessárias com OpenCode:\n\n<untrusted>\n{lines}\n</untrusted>",
                        &[("lines", recent_lines.trim())],
                    )
                };
                ai_w.set_visible(true);
                ai_b.set_active(true);
                sub_ai(&prompt);
                st.set_text(&tr("Resolvendo erro do terminal com OpenCode / IA..."));
            }
        });
        window.add_action(&action);
    }

    // 10. Ask AI Selection (`win.ask-ai-selection`)
    {
        let act = Rc::clone(active_term_cb);
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub_ai = Rc::clone(submit_ai_prompt_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("ask-ai-selection", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                let selected = term
                    .text_selected(vte4::Format::Text)
                    .map(|g| g.to_string())
                    .unwrap_or_default();
                let prompt = tr_fmt(
                    "Por favor, analise a seguinte seleção do terminal:\n\n<untrusted>\n{sel}\n</untrusted>",
                    &[("sel", selected.trim())],
                );
                ai_w.set_visible(true);
                ai_b.set_active(true);
                sub_ai(&prompt);
                st.set_text(&tr("Seleção enviada para análise no Assistente de IA."));
            }
        });
        window.add_action(&action);
    }

    // 11. Refactor Selection with OpenCode (`win.opencode-selection`)
    {
        let act = Rc::clone(active_term_cb);
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub_ai = Rc::clone(submit_ai_prompt_cb);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("opencode-selection", None);
        action.connect_activate(move |_, _| {
            if let Some(term) = act() {
                let selected = term
                    .text_selected(vte4::Format::Text)
                    .map(|g| g.to_string())
                    .unwrap_or_default();
                let prompt = tr_fmt(
                    "Por favor, refatore o seguinte trecho de código com OpenCode:\n\n<untrusted>\n{sel}\n</untrusted>",
                    &[("sel", selected.trim())],
                );
                ai_w.set_visible(true);
                ai_b.set_active(true);
                sub_ai(&prompt);
                st.set_text(&tr("Seleção enviada para refatoração com OpenCode."));
            }
        });
        window.add_action(&action);
    }

    // 12. Open URL (`win.open-url`)
    {
        let c_url = Rc::clone(clicked_url_rc);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("open-url", None);
        action.connect_activate(move |_, _| {
            if let Some(url) = c_url.borrow().as_deref() {
                let _ = gio::AppInfo::launch_default_for_uri(url, None::<&gio::AppLaunchContext>);
                st.set_text(&tr_fmt("Abrindo link: {url}", &[("url", url)]));
            }
        });
        window.add_action(&action);
    }

    // 13. Copy URL (`win.copy-url`)
    {
        let c_url = Rc::clone(clicked_url_rc);
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("copy-url", None);
        action.connect_activate(move |_, _| {
            if let Some(url) = c_url.borrow().as_deref() {
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(url);
                    st.set_text(&tr("Link copiado para a área de transferência."));
                }
            }
        });
        window.add_action(&action);
    }

    // 14. Split Horizontal (`win.split-horizontal`)
    {
        let bsp = btn_split.clone();
        let action = gio::SimpleAction::new("split-horizontal", None);
        action.connect_activate(move |_, _| {
            bsp.emit_clicked();
        });
        window.add_action(&action);
    }

    // 15. Split Vertical (`win.split-vertical`)
    {
        let bsp = btn_split.clone();
        let action = gio::SimpleAction::new("split-vertical", None);
        action.connect_activate(move |_, _| {
            bsp.emit_clicked();
        });
        window.add_action(&action);
    }

    // 16. Close Pane (`win.close-pane`)
    {
        let tv = tab_view.clone();
        let st = status_lbl.clone();
        let action = gio::SimpleAction::new("close-pane", None);
        action.connect_activate(move |_, _| {
            if let Some(page) = tv.selected_page() {
                if tv.n_pages() > 1 {
                    tv.close_page(&page);
                    st.set_text(&tr("Painel/Aba encerrado."));
                }
            }
        });
        window.add_action(&action);
    }

    // Bind standard accelerators matching `CORRIGIR/menucontext3.jpg` and `src/onyxsh/app.py`
    let st = settings_rc.borrow();
    let copy_accel = st.shortcuts.get("copy").cloned().unwrap_or_else(|| "<Control><Shift>Insert".to_string());
    let paste_accel = st.shortcuts.get("paste").cloned().unwrap_or_else(|| "<Control><Shift>v".to_string());
    let sel_accel = st.shortcuts.get("select-all").cloned().unwrap_or_else(|| "<Control><Shift>a".to_string());
    let clear_accel = st.shortcuts.get("clear-session").cloned().unwrap_or_else(|| "<Control><Shift>l".to_string());
    let prev_p_accel = st.shortcuts.get("jump-previous-prompt").cloned().unwrap_or_else(|| "<Alt>Page_Up".to_string());
    let next_p_accel = st.shortcuts.get("jump-next-prompt").cloned().unwrap_or_else(|| "<Alt>Page_Down".to_string());
    let split_h_accel = st.shortcuts.get("split-horizontal").cloned().unwrap_or_else(|| "<Control><Shift>parenleft".to_string());
    let split_v_accel = st.shortcuts.get("split-vertical").cloned().unwrap_or_else(|| "<Control><Shift>parenright".to_string());
    let close_p_accel = st.shortcuts.get("close-pane").cloned().unwrap_or_else(|| "<Control><Shift>k".to_string());

    let mut copy_accels = vec!["<Control><Shift>Insert".to_string(), "<Control><Shift>c".to_string()];
    if !copy_accels.contains(&copy_accel) {
        copy_accels.insert(0, copy_accel);
    }
    let copy_refs: Vec<&str> = copy_accels.iter().map(String::as_str).collect();
    app.set_accels_for_action("win.copy", &copy_refs);

    let paste_accels = vec![paste_accel, "<Shift>Insert".to_string()];
    let paste_refs: Vec<&str> = paste_accels.iter().map(String::as_str).collect();
    app.set_accels_for_action("win.paste", &paste_refs);

    app.set_accels_for_action("win.select-all", &[sel_accel.as_str()]);
    app.set_accels_for_action("win.clear-session", &[clear_accel.as_str()]);

    let mut prev_p_accels = vec!["<Alt>Page_Up".to_string(), "<Alt>Up".to_string()];
    if !prev_p_accels.contains(&prev_p_accel) {
        prev_p_accels.insert(0, prev_p_accel);
    }
    let prev_p_refs: Vec<&str> = prev_p_accels.iter().map(String::as_str).collect();
    app.set_accels_for_action("win.jump-previous-prompt", &prev_p_refs);

    let mut next_p_accels = vec!["<Alt>Page_Down".to_string(), "<Alt>Down".to_string()];
    if !next_p_accels.contains(&next_p_accel) {
        next_p_accels.insert(0, next_p_accel);
    }
    let next_p_refs: Vec<&str> = next_p_accels.iter().map(String::as_str).collect();
    app.set_accels_for_action("win.jump-next-prompt", &next_p_refs);

    app.set_accels_for_action("win.split-horizontal", &[split_h_accel.as_str()]);
    app.set_accels_for_action("win.split-vertical", &[split_v_accel.as_str()]);
    app.set_accels_for_action("win.close-pane", &[close_p_accel.as_str()]);
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
    let clicked_url_rc: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    {
        let initial_theme_mode = GtkThemeMode::from_setting(&settings_rc.borrow().gtk_theme);
        let initial_pal = get_palette(&settings_rc.borrow().color_scheme);
        apply_gtk_theme_mode(
            initial_theme_mode,
            is_light_hex_color(initial_pal.background),
        );
    }

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
        .tooltip_text(&tr("Exibir Painel de Sessões"))
        .active(false)
        .build();
    header_bar.pack_start(&btn_sidebar);

    let btn_fm = gtk4::ToggleButton::builder()
        .icon_name("folder-open-symbolic")
        .tooltip_text(&tr("Gerenciador de Arquivos Dual-Pane"))
        .active(false)
        .build();
    header_bar.pack_start(&btn_fm);

    let btn_ai = gtk4::ToggleButton::builder()
        .icon_name("avatar-default-symbolic")
        .tooltip_text(&tr("Assistente de IA Zero-Trust"))
        .active(false)
        .build();
    header_bar.pack_start(&btn_ai);

    // Phase 9: Search Toggle Button (Lupa)
    let btn_search = gtk4::ToggleButton::builder()
        .icon_name("edit-find-symbolic")
        .tooltip_text(&tr("Buscar no Terminal (Ctrl+Shift+F)"))
        .active(false)
        .build();
    header_bar.pack_start(&btn_search);

    let btn_split = gtk4::Button::builder()
        .icon_name("view-dual-symbolic")
        .tooltip_text(&tr("Dividir Terminal Horizontalmente"))
        .build();
    header_bar.pack_start(&btn_split);

    // Phase 13: Command Manager Dialog Button (`utilities-terminal-symbolic`)
    let btn_cmd_manager = gtk4::Button::builder()
        .icon_name("utilities-terminal-symbolic")
        .tooltip_text(&tr("Gerenciador de Comandos & Formulários Dinâmicos"))
        .build();
    header_bar.pack_start(&btn_cmd_manager);

    // Phase 14: Real-Time Resource Monitor Button (`computer-symbolic`)
    let btn_resource_monitor = gtk4::Button::builder()
        .icon_name("computer-symbolic")
        .tooltip_text(&tr("Monitor de Recursos em Tempo Real (Sparklines Cairo)"))
        .build();
    header_bar.pack_start(&btn_resource_monitor);

    // Phase 16: Autocomplete Assistant Button (`insert-text-symbolic`)
    let btn_autocomplete = gtk4::Button::builder()
        .icon_name("insert-text-symbolic")
        .tooltip_text(&tr("Autocompletar Inteligente (CompletionEngine)"))
        .build();
    header_bar.pack_start(&btn_autocomplete);

    // Global Multi-Tab / Multi-Source Search Button (`system-search-symbolic` — Ctrl+Shift+G)
    let btn_global_search = gtk4::Button::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text(&tr(
            "Busca Global Multi-Abas, Sessões, Histórico & Snippets (Ctrl+Shift+G)",
        ))
        .build();
    header_bar.pack_start(&btn_global_search);

    // Enriched Command History Button (`document-open-recent-symbolic` — Ctrl+H)
    let btn_cmd_history = gtk4::Button::builder()
        .icon_name("document-open-recent-symbolic")
        .tooltip_text(&tr("Histórico Enriquecido de Comandos (Ctrl+H)"))
        .build();
    header_bar.pack_start(&btn_cmd_history);

    // Phase 16: Command Palette Button (`preferences-system-search-symbolic`)
    let btn_palette = gtk4::Button::builder()
        .icon_name("preferences-system-search-symbolic")
        .tooltip_text(&tr("Command Palette — Busca Unificada (Ctrl+Shift+P)"))
        .build();
    header_bar.pack_start(&btn_palette);

    // Phase 10: Main Hamburger Menu Button (`open-menu-symbolic`)
    let btn_menu = gtk4::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text(&tr("Main Menu"))
        .build();
    header_bar.pack_end(&btn_menu);

    let btn_new_tab = gtk4::Button::builder()
        .icon_name("tab-new-symbolic")
        .tooltip_text(&tr("Nova Aba de Terminal (Ctrl+Shift+T)"))
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
    search_entry.set_placeholder_text(Some(&tr("Buscar texto no terminal...")));
    search_bar.connect_entry(&search_entry);

    let search_occurrence_label = gtk4::Label::new(Some("0/0"));
    search_occurrence_label.add_css_class("dim-label");
    search_occurrence_label.add_css_class("monospace");
    search_occurrence_label.set_tooltip_text(Some(&tr("Ocorrências encontradas no terminal")));

    // Linked options box (`Aa`, `\b`, `.*`)
    let options_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    options_box.add_css_class("linked");

    let btn_case_sensitive = gtk4::ToggleButton::with_label("Aa");
    btn_case_sensitive.add_css_class("flat");
    btn_case_sensitive.set_tooltip_text(Some(&tr(
        "Diferenciar Maiúsculas/Minúsculas (Match Case)",
    )));
    btn_case_sensitive.set_active(settings_rc.borrow().search_case_sensitive);
    options_box.append(&btn_case_sensitive);

    let btn_whole_word = gtk4::ToggleButton::with_label(r"\b");
    btn_whole_word.add_css_class("flat");
    btn_whole_word.set_tooltip_text(Some(&tr("Palavra Inteira (Whole Word)")));
    btn_whole_word.set_active(settings_rc.borrow().search_whole_word);
    options_box.append(&btn_whole_word);

    let btn_regex = gtk4::ToggleButton::with_label(".*");
    btn_regex.add_css_class("flat");
    btn_regex.set_tooltip_text(Some(&tr("Use Regular Expression (Regex)")));
    btn_regex.set_active(settings_rc.borrow().search_use_regex);
    options_box.append(&btn_regex);

    let btn_search_prev = gtk4::Button::from_icon_name("go-up-symbolic");
    btn_search_prev.add_css_class("flat");
    btn_search_prev.set_tooltip_text(Some(&tr("Ocorrência Anterior (Shift+Enter)")));

    let btn_search_next = gtk4::Button::from_icon_name("go-down-symbolic");
    btn_search_next.add_css_class("flat");
    btn_search_next.set_tooltip_text(Some(&tr("Próxima Ocorrência (Enter)")));

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

    let status_lbl = gtk4::Label::new(Some(&tr("OnyxSH Rust Engine v0.14.0 — Ready")));
    status_lbl.add_css_class("caption");
    status_lbl.set_halign(gtk4::Align::Start);
    status_lbl.set_hexpand(true);

    let metrics_lbl = gtk4::Label::new(Some("CPU: 0.0% | RAM: -- / -- MB"));
    metrics_lbl.add_css_class("caption");
    metrics_lbl.set_halign(gtk4::Align::End);

    footer_box.append(&status_lbl);
    footer_box.append(&metrics_lbl);

    // ── 5. Phase 11: Complete Session Sidebar (5-Button CRUD + Folder Tree + Search) ──
    let attach_ux_holder: Rc<RefCell<Option<Rc<dyn Fn(&vte4::Terminal)>>>> =
        Rc::new(RefCell::new(None));

    let spawn_session_cb: Rc<dyn Fn(&SessionItem, bool)> = {
        let tv = tab_view.clone();
        let st_rc = Rc::clone(&settings_rc);
        let terms_rc = Rc::clone(&terminals_rc);
        let status = status_lbl.clone();
        let att_holder = Rc::clone(&attach_ux_holder);

        Rc::new(move |sess: &SessionItem, in_split: bool| {
            let st = st_rc.borrow();
            let make_term = || {
                let term = if sess.session_type == "ssh" && !sess.host.is_empty() {
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
                };
                if let Some(ref att) = *att_holder.borrow() {
                    att(&term);
                }
                term
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
            status.set_text(&tr_fmt(
                "Conectado à sessão: {name}",
                &[("name", &sess.name)],
            ));
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
    let (cmd_and_broadcast_bar, rebuild_cmd_toolbar, btn_broadcast_toggle) =
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
                        status_inner.set_text(&tr_fmt(
                            "Broadcast enviado: {cmd}",
                            &[("cmd", cmd_str)],
                        ));
                    } else if let Some(t) = act_inner() {
                        t.feed_child(payload.as_bytes());
                        status_inner.set_text(&tr_fmt(
                            "Comando enviado: {cmd}",
                            &[("cmd", cmd_str)],
                        ));
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

    let on_close_ai: Rc<dyn Fn()> = {
        let bai = btn_ai.clone();
        Rc::new(move || {
            bai.set_active(false);
        })
    };

    // Phase 15: Complete Zero-Trust AI Assistant Bottom Horizontal Panel (Python window_ui.py specification)
    let (ai_widget, submit_ai_prompt_cb) =
        crate::ai_assistant_ui::build_complete_ai_assistant_panel(
            &window,
            &settings_rc,
            Rc::clone(&active_term_cb),
            &status_lbl,
            Some(on_close_ai),
        );
    ai_widget.set_visible(false);

    let cd_in_terminal_cb: Rc<dyn Fn(&std::path::Path)> = {
        let tv = tab_view.clone();
        let status = status_lbl.clone();
        Rc::new(move |dir_path: &std::path::Path| {
            if let Some(term) = get_active_vte_terminal(&tv) {
                let escaped = dir_path.to_string_lossy().replace('\'', "'\\''");
                let cmd = format!("cd '{escaped}'\n");
                term.feed_child(cmd.as_bytes());
                status.set_text(&tr_fmt(
                    "Diretório enviado ao terminal: {dir}",
                    &[("dir", &dir_path.display().to_string())],
                ));
            }
        })
    };

    let feed_terminal_cb: Rc<dyn Fn(&str)> = {
        let tv = tab_view.clone();
        let status = status_lbl.clone();
        Rc::new(move |cmd_str: &str| {
            if let Some(term) = get_active_vte_terminal(&tv) {
                term.feed_child(cmd_str.as_bytes());
                status.set_text(&tr_fmt(
                    "Comando enviado ao terminal: {cmd}",
                    &[("cmd", cmd_str.trim())],
                ));
            }
        })
    };

    let ai_prompt_cb: Rc<dyn Fn(&str)> = {
        let ai_w = ai_widget.clone();
        let bai = btn_ai.clone();
        let sub = Rc::clone(&submit_ai_prompt_cb);
        Rc::new(move |prompt: &str| {
            ai_w.set_visible(true);
            bai.set_active(true);
            sub(prompt);
        })
    };

    let fm_widget = crate::filemanager_ui::build_complete_file_manager(
        &window,
        &status_lbl,
        cd_in_terminal_cb,
        feed_terminal_cb,
        Some(ai_prompt_cb),
    );
    fm_widget.set_visible(false);

    // Vertical GtkPaned allowing user to resize file manager height up/down
    let fm_paned = gtk4::Paned::new(gtk4::Orientation::Vertical);
    fm_paned.add_css_class("filemanager-paned");
    fm_paned.set_hexpand(true);
    fm_paned.set_vexpand(true);
    fm_paned.set_wide_handle(true);
    fm_paned.set_start_child(Some(&tab_view));
    fm_paned.set_end_child(Some(&fm_widget));
    fm_paned.set_resize_start_child(true);
    fm_paned.set_shrink_start_child(false);
    fm_paned.set_resize_end_child(true);
    fm_paned.set_shrink_end_child(false);

    // Vertical GtkPaned for Zero-Trust AI Assistant bottom horizontal panel (Python window_ui.py specification)
    let ai_paned = gtk4::Paned::new(gtk4::Orientation::Vertical);
    ai_paned.add_css_class("ai-paned");
    ai_paned.set_hexpand(true);
    ai_paned.set_vexpand(true);
    ai_paned.set_wide_handle(true);
    ai_paned.set_start_child(Some(&fm_paned));
    ai_paned.set_end_child(Some(&ai_widget));
    ai_paned.set_resize_start_child(true);
    ai_paned.set_shrink_start_child(false);
    ai_paned.set_resize_end_child(true);
    ai_paned.set_shrink_end_child(false);

    center_vbox.append(&ai_paned);

    // Main Horizontal Layout
    let main_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    main_hbox.set_hexpand(true);
    main_hbox.set_vexpand(true);
    sidebar_box.set_visible(false);
    let sidebar_sep = gtk4::Separator::new(gtk4::Orientation::Vertical);
    sidebar_sep.set_visible(false);
    main_hbox.append(&sidebar_box);
    main_hbox.append(&sidebar_sep);
    main_hbox.append(&center_vbox);

    root_vbox.append(&main_hbox);
    root_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    root_vbox.append(&footer_box);

    // ── 6. Phase 10: Build Main Hamburger Menu Popover (`ui/menus.py`) ────────
    {
        let menu_popover = gtk4::Popover::new();
        menu_popover.set_has_arrow(true);
        let menu_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        menu_vbox.set_width_request(280);
        menu_vbox.set_margin_start(10);
        menu_vbox.set_margin_end(10);
        menu_vbox.set_margin_top(8);
        menu_vbox.set_margin_bottom(8);

        // 6.1 ThemeSelectorWidget (4 circular buttons: Follow System, Light, Dark, Terminal Match)
        let theme_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 22);
        theme_hbox.add_css_class("themeselector");
        theme_hbox.set_halign(gtk4::Align::Center);

        let rb_system = gtk4::CheckButton::builder()
            .tooltip_text(&tr("Seguir Estilo do Sistema"))
            .build();
        rb_system.add_css_class("follow");
        rb_system.add_css_class("theme-selector");

        let rb_light = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text(&tr("Estilo Claro (Light)"))
            .build();
        rb_light.add_css_class("light");
        rb_light.add_css_class("theme-selector");

        let rb_dark = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text(&tr("Estilo Escuro (Dark)"))
            .build();
        rb_dark.add_css_class("dark");
        rb_dark.add_css_class("theme-selector");

        let rb_terminal = gtk4::CheckButton::builder()
            .group(&rb_system)
            .tooltip_text(&tr("Combinar com Cores do Terminal"))
            .build();
        rb_terminal.add_css_class("terminal");
        rb_terminal.add_css_class("theme-selector");

        match settings_rc.borrow().gtk_theme.as_str() {
            "light" => rb_light.set_active(true),
            "dark" => rb_dark.set_active(true),
            "terminal" => rb_terminal.set_active(true),
            _ => rb_system.set_active(true),
        }

        {
            let st = Rc::clone(&settings_rc);
            rb_system.connect_toggled(move |btn| {
                if btn.is_active() {
                    st.borrow_mut().gtk_theme = "default".to_string();
                    let pal = get_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::System, is_light_hex_color(pal.background));
                    let cfg_path = platform::config_dir().join("settings.json");
                    let _ = st.borrow().save_to_path(&cfg_path);
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_light.connect_toggled(move |btn| {
                if btn.is_active() {
                    st.borrow_mut().gtk_theme = "light".to_string();
                    let pal = get_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::Light, is_light_hex_color(pal.background));
                    let cfg_path = platform::config_dir().join("settings.json");
                    let _ = st.borrow().save_to_path(&cfg_path);
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_dark.connect_toggled(move |btn| {
                if btn.is_active() {
                    st.borrow_mut().gtk_theme = "dark".to_string();
                    let pal = get_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(GtkThemeMode::Dark, is_light_hex_color(pal.background));
                    let cfg_path = platform::config_dir().join("settings.json");
                    let _ = st.borrow().save_to_path(&cfg_path);
                }
            });
        }
        {
            let st = Rc::clone(&settings_rc);
            rb_terminal.connect_toggled(move |btn| {
                if btn.is_active() {
                    st.borrow_mut().gtk_theme = "terminal".to_string();
                    let pal = get_palette(&st.borrow().color_scheme);
                    apply_gtk_theme_mode(
                        GtkThemeMode::TerminalMatch,
                        is_light_hex_color(pal.background),
                    );
                    let cfg_path = platform::config_dir().join("settings.json");
                    let _ = st.borrow().save_to_path(&cfg_path);
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
        btn_zoom_out.set_tooltip_text(Some(&tr("Diminuir Fonte (Ctrl+-)")));

        let (_, initial_pt) = parse_font_string(&settings_rc.borrow().font);
        let font_size_lbl = gtk4::Label::new(Some(&format!("{initial_pt} pt")));
        font_size_lbl.set_width_chars(6);
        let btn_zoom_reset = gtk4::Button::new();
        btn_zoom_reset.add_css_class("flat");
        btn_zoom_reset.set_tooltip_text(Some(&tr("Resetar Tamanho da Fonte (12 pt)")));
        btn_zoom_reset.set_child(Some(&font_size_lbl));

        let btn_zoom_in = gtk4::Button::from_icon_name("zoom-in-symbolic");
        btn_zoom_in.add_css_class("flat");
        btn_zoom_in.set_tooltip_text(Some(&tr("Aumentar Fonte (Ctrl++)")));

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

        // Helper to create flat menu action buttons with optional shortcut accelerator
        let make_menu_btn = |label: &str, icon: Option<&str>, shortcut: Option<&str>| -> gtk4::Button {
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
            if let Some(sc) = shortcut {
                let sc_lbl = gtk4::Label::new(Some(sc));
                sc_lbl.set_halign(gtk4::Align::End);
                sc_lbl.add_css_class("dim-label");
                hbox.append(&sc_lbl);
            }
            btn.set_child(Some(&hbox));
            btn
        };

        // 1. Nova Janela
        let mi_new_win = make_menu_btn(&tr("Nova Janela"), Some("window-new-symbolic"), Some("Shift+Ctrl+N"));
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

        // 2. Exportar Saída do Terminal & Caderno de Bordo (Runbook)
        let mi_export = make_menu_btn(
            &tr("Export Terminal Output..."),
            Some("document-save-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let tv = tab_view.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_export.connect_clicked(move |_| {
                pop.popdown();
                if let Some(term) = get_active_vte_terminal(&tv) {
                    let tab_title = tv.selected_page().map(|p| p.title().to_string());
                    crate::export_dialog::show_export_terminal_dialog(
                        &win_clone,
                        &term,
                        tab_title.as_deref(),
                        false,
                        &status,
                    );
                }
            });
        }
        menu_vbox.append(&mi_export);

        let mi_runbook = make_menu_btn(
            &tr("Exportar Sessão como Caderno de Bordo (Runbook)..."),
            Some("document-edit-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let tv = tab_view.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_runbook.connect_clicked(move |_| {
                pop.popdown();
                if let Some(term) = get_active_vte_terminal(&tv) {
                    let tab_title = tv.selected_page().map(|p| p.title().to_string());
                    crate::export_dialog::show_export_terminal_dialog(
                        &win_clone,
                        &term,
                        tab_title.as_deref(),
                        true,
                        &status,
                    );
                }
            });
        }
        menu_vbox.append(&mi_runbook);

        // 3. Preferências (4 Abas Completas — `adw::PreferencesWindow`)
        let mi_prefs = make_menu_btn(&tr("Preferences"), Some("preferences-system-symbolic"), Some("Shift+Ctrl+,"));
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let css_prov = css_provider.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_prefs.connect_clicked(move |_| {
                pop.popdown();
                crate::preferences_ui::show_preferences_window(
                    &win_clone,
                    Rc::clone(&st_rc),
                    Rc::clone(&terms_rc),
                    css_prov.clone(),
                    &status,
                );
            });
        }
        menu_vbox.append(&mi_prefs);

        // 4. Cores (Abre HighlightDialog / coresfix.png)
        let mi_colors = make_menu_btn(
            &tr("Colors"),
            Some("format-text-highlight-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let css_prov = css_provider.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_colors.connect_clicked(move |_| {
                pop.popdown();
                crate::highlight_dialog_ui::show_highlight_dialog(
                    &win_clone,
                    Rc::clone(&st_rc),
                    Rc::clone(&terms_rc),
                    css_prov.clone(),
                    &status,
                );
            });
        }
        menu_vbox.append(&mi_colors);

        // 5. Assistente de IA
        let mi_ai = make_menu_btn(&tr("Assistente de IA"), Some("avatar-default-symbolic"), None);
        {
            let bai = btn_ai.clone();
            let pop = menu_popover.clone();
            mi_ai.connect_clicked(move |_| {
                pop.popdown();
                bai.set_active(!bai.is_active());
            });
        }
        menu_vbox.append(&mi_ai);

        // 5b. Configurar Assistente de IA
        let mi_ai_cfg = make_menu_btn(
            &tr("Configurar Assistente de IA..."),
            Some("preferences-system-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_ai_cfg.connect_clicked(move |_| {
                pop.popdown();
                crate::ai_config_ui::show_ai_config_window(&win_clone, Rc::clone(&st_rc), &status);
            });
        }
        menu_vbox.append(&mi_ai_cfg);

        // 6. Atalhos de Teclado (Editável — 34 Ações)
        let mi_shortcuts = make_menu_btn(
            &tr("Atalhos de Teclado"),
            Some("preferences-desktop-keyboard-shortcuts-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let st_rc = Rc::clone(&settings_rc);
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_shortcuts.connect_clicked(move |_| {
                pop.popdown();
                let cfg_path = platform::config_dir().join("settings.json");
                crate::global_search_and_shortcuts::show_editable_shortcuts_dialog(
                    &win_clone,
                    &st_rc,
                    &cfg_path,
                    &status,
                );
            });
        }
        menu_vbox.append(&mi_shortcuts);

        // 7. Importar Sessões (~/.ssh/config)
        let mi_import = make_menu_btn(
            &tr("Importar Sessões (~/.ssh/config)"),
            Some("document-open-symbolic"),
            None,
        );
        {
            let pop_sess = Rc::clone(&populate_sessions_sidebar);
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_import.connect_clicked(move |_| {
                pop.popdown();
                pop_sess();
                status.set_text(&tr(
                    "Sessões sincronizadas a partir de ~/.config/onyxsh/sessions.json e ~/.ssh/config.",
                ));
            });
        }
        menu_vbox.append(&mi_import);

        // 8. Gerenciador de Túneis SSH
        let mi_tunnels = make_menu_btn(
            &tr("SSH Tunnel Manager"),
            Some("network-vpn-symbolic"),
            None,
        );
        {
            let win_clone = window.clone();
            let status = status_lbl.clone();
            let pop = menu_popover.clone();
            mi_tunnels.connect_clicked(move |_| {
                pop.popdown();
                show_tunnel_manager_dialog(&win_clone, &status);
            });
        }
        menu_vbox.append(&mi_tunnels);

        // 9. Iniciar / Parar Servidor TFTP
        let tftp_lbl = gtk4::Label::new(Some(&tr("Iniciar Servidor TFTP")));
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
                    lbl_clone.set_text(&tr("Iniciar Servidor TFTP"));
                    status.set_text(&tr("Servidor TFTP encerrado."));
                } else {
                    let root = platform::home_dir();
                    if let Ok(srv) = UdpTftpServer::bind("127.0.0.1:0", &root) {
                        let addr = srv
                            .local_addr()
                            .map(|a| a.to_string())
                            .unwrap_or_else(|_| "127.0.0.1:6969".to_string());
                        lbl_clone.set_text(&tr_fmt(
                            "Parar Servidor TFTP ({addr})",
                            &[("addr", &addr)],
                        ));
                        status.set_text(&tr_fmt(
                            "Servidor TFTP Read-Only ativo em {addr} (raiz: {root})",
                            &[
                                ("addr", &addr),
                                ("root", &root.display().to_string()),
                            ],
                        ));
                        *guard = Some(srv);
                    }
                }
            });
        }
        menu_vbox.append(&mi_tftp);

        // 10. Sobre
        let mi_about = make_menu_btn(&tr("Sobre o OnyxSH"), Some("help-about-symbolic"), None);
        {
            let win_clone = window.clone();
            let pop = menu_popover.clone();
            mi_about.connect_clicked(move |_| {
                pop.popdown();

                let gtk_ver = format!(
                    "{}.{}.{}",
                    gtk4::major_version(),
                    gtk4::minor_version(),
                    gtk4::micro_version()
                );
                let adw_ver = format!(
                    "{}.{}.{}",
                    adw::major_version(),
                    adw::minor_version(),
                    adw::micro_version()
                );
                let debug_info = format!(
                    "App ID: {}\nVersion: {}\nPlatform: Linux\nArchitecture: {}\nShell: {}\nGTK: {}\nlibadwaita: {}\nFlatpak: {}\nConfig: {}",
                    APP_ID,
                    APP_VERSION,
                    std::env::consts::ARCH,
                    std::env::var("SHELL").unwrap_or_else(|_| "N/A".to_string()),
                    gtk_ver,
                    adw_ver,
                    platform::is_flatpak_sandbox(),
                    platform::config_dir().display(),
                );

                let about = adw::AboutWindow::builder()
                    .transient_for(&win_clone)
                    .modal(true)
                    .application_name(APP_NAME)
                    .application_icon(APP_ID)
                    .version(APP_VERSION)
                    .developer_name(DEVELOPER_NAME)
                    .developers(DEVELOPERS)
                    .copyright(COPYRIGHT)
                    .website(WEBSITE)
                    .issue_url(ISSUE_URL)
                    .license_type(gtk4::License::Gpl30)
                    .comments(&tr(
                        "Emulador de terminal moderno para fluxos de trabalho locais e remotos no Linux, com ProductionGuard, Dual-Pane SFTP/TFTP e Assistente de IA Zero-Trust.",
                    ))
                    .debug_info(&debug_info)
                    .build();

                about.add_link(&tr("Código-Fonte"), WEBSITE);
                about.add_link(&tr("Reportar Problema"), ISSUE_URL);

                about.present();
            });
        }
        menu_vbox.append(&mi_about);

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        // 11. Sair
        let mi_quit = make_menu_btn(&tr("Quit"), Some("application-exit-symbolic"), Some("Shift+Ctrl+Q"));
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
        Rc::clone(&clicked_url_rc),
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
        let c_url = Rc::clone(&clicked_url_rc);
        let win_c = window.clone();
        let ai_w = ai_widget.clone();
        let ai_b = btn_ai.clone();
        let sub = Rc::clone(&submit_ai_prompt_cb);
        let st = status_lbl.clone();
        *attach_ux_holder.borrow_mut() = Some(Rc::new(move |t: &vte4::Terminal| {
            crate::terminal_ux::attach_vte_ux_controllers(
                t,
                Rc::clone(&c_url),
                &win_c,
                &ai_w,
                &ai_b,
                Rc::clone(&sub),
                &st,
            );
        }));
    }

    {
        let sb = sidebar_box.clone();
        let sep = sidebar_sep.clone();
        btn_sidebar.connect_toggled(move |btn| {
            let active = btn.is_active();
            sb.set_visible(active);
            sep.set_visible(active);
            btn.set_icon_name(if active {
                "sidebar-hide-symbolic"
            } else {
                "sidebar-show-symbolic"
            });
            btn.set_tooltip_text(Some(&if active {
                tr("Hide Sessions Panel")
            } else {
                tr("Exibir Painel de Sessões")
            }));
        });
    }
    let is_programmatic_pos = Rc::new(Cell::new(false));
    {
        let fm = fm_widget.clone();
        let paned = fm_paned.clone();
        let st_rc = Rc::clone(&settings_rc);
        let win_ref = window.clone();
        let is_prog = Rc::clone(&is_programmatic_pos);

        btn_fm.connect_toggled(move |btn| {
            let active = btn.is_active();
            if active {
                is_prog.set(true);

                // Make file manager visible first so GTK Paned has both children realized/visible
                fm.set_visible(true);

                let paned_h = paned.height();
                let win_h = win_ref.height();
                let alloc_h = if paned_h > 200 {
                    paned_h
                } else {
                    (win_h - 100).max(400)
                };

                let raw_saved = st_rc.borrow().file_manager_height as i32;
                let (_target_fm_h, target_pos) =
                    crate::filemanager_ui::calculate_file_manager_split(alloc_h, raw_saved);

                paned.set_position(target_pos);

                let p_clone = paned.clone();
                let st_clone = Rc::clone(&st_rc);
                let w_clone = win_ref.clone();
                let is_prog_clone = Rc::clone(&is_prog);

                glib::idle_add_local_once(move || {
                    let total_h = p_clone.height();
                    let alloc_h = if total_h > 200 {
                        total_h
                    } else {
                        (w_clone.height() - 100).max(400)
                    };
                    let raw_saved = st_clone.borrow().file_manager_height as i32;
                    let (_target_fm_h, target_pos) =
                        crate::filemanager_ui::calculate_file_manager_split(alloc_h, raw_saved);
                    p_clone.set_position(target_pos);

                    glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                        is_prog_clone.set(false);
                    });
                });
            } else {
                is_prog.set(true);
                // Save last position before hiding, matching python tabs.py closing logic
                let total_h = paned.height();
                let pos = paned.position();
                if total_h > 200 && pos > 50 {
                    let min_fm = 240;
                    let min_term = 120;
                    let max_fm = (total_h - min_term).max(min_fm);
                    let last_h = (total_h - pos).clamp(min_fm, max_fm) as u32;
                    if last_h >= 240 {
                        st_rc.borrow_mut().file_manager_height = last_h;
                        let st_to_save = st_rc.borrow().clone();
                        let cfg_p = platform::config_dir().join("settings.json");
                        let _ = st_to_save.save_to_path(&cfg_p);
                    }
                }
                fm.set_visible(false);
                let is_prog_clone = Rc::clone(&is_prog);
                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    is_prog_clone.set(false);
                });
            }
        });
    }

    // Connect notify::position on fm_paned to dynamically update and persist file_manager_height
    {
        let paned = fm_paned.clone();
        let fm = fm_widget.clone();
        let st_rc = Rc::clone(&settings_rc);
        let debounce_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let is_prog_track = Rc::clone(&is_programmatic_pos);

        paned.connect_position_notify(move |p| {
            if !fm.is_visible() || is_prog_track.get() {
                return;
            }
            let total_h = p.height();
            let pos = p.position();
            if total_h <= 200 || pos <= 50 {
                return;
            }
            let min_fm = 240;
            let min_term = 120;
            let max_fm = (total_h - min_term).max(min_fm);
            let calculated_fm_h = (total_h - pos).clamp(min_fm, max_fm) as u32;

            if st_rc.borrow().file_manager_height != calculated_fm_h {
                st_rc.borrow_mut().file_manager_height = calculated_fm_h;

                if let Some(source_id) = debounce_timer.borrow_mut().take() {
                    source_id.remove();
                }

                let st_persist = Rc::clone(&st_rc);
                let timer_holder = Rc::clone(&debounce_timer);
                let new_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(300),
                    move || {
                        timer_holder.borrow_mut().take();
                        let cfg_path = platform::config_dir().join("settings.json");
                        let _ = st_persist.borrow().save_to_path(&cfg_path);
                    },
                );
                *debounce_timer.borrow_mut() = Some(new_id);
            }
        });
    }
    // Programmatic position flag to prevent feedback loops between set_position and position_notify on ai_paned
    let is_programmatic_ai_pos = Rc::new(std::cell::Cell::new(false));

    // Toggle AI Assistant panel visibility & set ~50% horizontal split (Python window_ui.py specification)
    {
        let paned = ai_paned.clone();
        let ai = ai_widget.clone();
        let st_rc = Rc::clone(&settings_rc);
        let win = window.clone();
        let is_prog = Rc::clone(&is_programmatic_ai_pos);

        btn_ai.connect_toggled(move |btn| {
            let active = btn.is_active();
            if active {
                is_prog.set(true);
                ai.set_visible(true);
                let total_h = paned.height();
                let alloc_h = if total_h > 200 {
                    total_h
                } else {
                    (win.height() - 100).max(400)
                };
                let raw_saved = st_rc.borrow().ai_panel_height as i32;
                let (_target_ai_h, target_pos) =
                    crate::ai_assistant_ui::calculate_ai_panel_split(alloc_h, raw_saved);
                paned.set_position(target_pos);

                let p_clone = paned.clone();
                let w_clone = win.clone();
                let st_clone = Rc::clone(&st_rc);
                let is_prog_clone = Rc::clone(&is_prog);
                glib::idle_add_local_once(move || {
                    let total_h = p_clone.height();
                    let alloc_h = if total_h > 200 {
                        total_h
                    } else {
                        (w_clone.height() - 100).max(400)
                    };
                    let raw_saved = st_clone.borrow().ai_panel_height as i32;
                    let (_target_ai_h, target_pos) =
                        crate::ai_assistant_ui::calculate_ai_panel_split(alloc_h, raw_saved);
                    p_clone.set_position(target_pos);

                    glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                        is_prog_clone.set(false);
                    });
                });
            } else {
                is_prog.set(true);
                let total_h = paned.height();
                let pos = paned.position();
                if total_h > 200 && pos > 50 {
                    let min_ai = 200;
                    let min_term = 120;
                    let max_ai = (total_h - min_term).max(min_ai);
                    let last_h = (total_h - pos).clamp(min_ai, max_ai) as u32;
                    if last_h >= 200 {
                        st_rc.borrow_mut().ai_panel_height = last_h;
                        let st_to_save = st_rc.borrow().clone();
                        let cfg_p = platform::config_dir().join("settings.json");
                        let _ = st_to_save.save_to_path(&cfg_p);
                    }
                }
                ai.set_visible(false);
                let is_prog_clone = Rc::clone(&is_prog);
                glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
                    is_prog_clone.set(false);
                });
            }
        });
    }

    // Connect notify::position on ai_paned to dynamically update and persist ai_panel_height
    {
        let paned = ai_paned.clone();
        let ai = ai_widget.clone();
        let st_rc = Rc::clone(&settings_rc);
        let debounce_timer: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
        let is_prog_track = Rc::clone(&is_programmatic_ai_pos);

        paned.connect_position_notify(move |p| {
            if !ai.is_visible() || is_prog_track.get() {
                return;
            }
            let total_h = p.height();
            let pos = p.position();
            if total_h <= 200 || pos <= 50 {
                return;
            }
            let min_ai = 200;
            let min_term = 120;
            let max_ai = (total_h - min_term).max(min_ai);
            let calculated_ai_h = (total_h - pos).clamp(min_ai, max_ai) as u32;

            if st_rc.borrow().ai_panel_height != calculated_ai_h {
                st_rc.borrow_mut().ai_panel_height = calculated_ai_h;

                if let Some(source_id) = debounce_timer.borrow_mut().take() {
                    source_id.remove();
                }

                let st_persist = Rc::clone(&st_rc);
                let timer_holder = Rc::clone(&debounce_timer);
                let new_id = glib::timeout_add_local_once(
                    std::time::Duration::from_millis(300),
                    move || {
                        timer_holder.borrow_mut().take();
                        let cfg_path = platform::config_dir().join("settings.json");
                        let _ = st_persist.borrow().save_to_path(&cfg_path);
                    },
                );
                *debounce_timer.borrow_mut() = Some(new_id);
            }
        });
    }
    {
        let tv = tab_view.clone();
        let st = Rc::clone(&settings_rc);
        let terms = Rc::clone(&terminals_rc);
        let win_ref = window.clone();
        let c_url = Rc::clone(&clicked_url_rc);
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
                Rc::clone(&c_url),
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
        let c_url = Rc::clone(&clicked_url_rc);
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
                Rc::clone(&c_url),
                &win_ref,
                &ai_w,
                &ai_b,
                Rc::clone(&sub_ai),
                &status,
            );
            crate::terminal_ux::attach_vte_ux_controllers(
                &t2,
                Rc::clone(&c_url),
                &win_ref,
                &ai_w,
                &ai_b,
                Rc::clone(&sub_ai),
                &status,
            );
            paned.set_start_child(Some(&t1));
            paned.set_end_child(Some(&t2));
            let page = tv.append(&paned);
            page.set_title(&tr("Split Terminal"));
            tv.set_selected_page(&page);
        });
    }

    setup_terminal_window_actions(
        app,
        &window,
        &tab_view,
        &active_term_cb,
        &ai_widget,
        &btn_ai,
        &submit_ai_prompt_cb,
        &btn_split,
        &settings_rc,
        &status_lbl,
        &clicked_url_rc,
    );

    // Global Search (`Ctrl+Shift+G`) & Command History (`Ctrl+H`) HeaderBar Buttons
    {
        let win_ref = window.clone();
        let tv = tab_view.clone();
        let cmds_rc = Rc::clone(&cmd_buttons_rc);
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();

        btn_global_search.connect_clicked(move |_| {
            let at_sess = Rc::clone(&act_term);
            let st_sess = status.clone();
            let on_sess: Rc<dyn Fn(&SessionItem)> = Rc::new(move |sess: &SessionItem| {
                if let Some(term) = at_sess() {
                    if sess.session_type == "ssh" && !sess.host.is_empty() {
                        let cmd = format!("ssh -p {} {}\n", sess.port, sess.connection_string());
                        term.feed_child(cmd.as_bytes());
                    }
                    st_sess.set_text(&tr_fmt(
                        "Conectando via Busca Global: {name}",
                        &[("name", &sess.name)],
                    ));
                }
            });
            crate::global_search_and_shortcuts::show_global_search_dialog(
                &win_ref,
                &tv,
                &cmds_rc.borrow(),
                Rc::clone(&act_term),
                on_sess,
                &status,
            );
        });
    }
    {
        let win_ref = window.clone();
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();
        btn_cmd_history.connect_clicked(move |_| {
            crate::global_search_and_shortcuts::show_command_history_dialog(
                &win_ref,
                Rc::clone(&act_term),
                &status,
            );
        });
    }

    // Phase 16: Unified Command Palette (`Ctrl+Shift+P`)
    {
        let win_ref = window.clone();
        let tv = tab_view.clone();
        let st_rc_pal = Rc::clone(&settings_rc);
        let terms_rc_pal = Rc::clone(&terminals_rc);
        let css_prov_pal = css_provider.clone();
        let cmds_rc = Rc::clone(&cmd_buttons_rc);
        let b_nt = btn_new_tab.clone();
        let b_sp = btn_split.clone();
        let b_sb = btn_sidebar.clone();
        let b_fm = btn_fm.clone();
        let b_ai = btn_ai.clone();
        let b_sr = btn_search.clone();
        let b_gs = btn_global_search.clone();
        let b_ch = btn_cmd_history.clone();
        let b_bc = btn_broadcast_toggle.clone();
        let b_cm = btn_cmd_manager.clone();
        let b_rm = btn_resource_monitor.clone();
        let b_ac = btn_autocomplete.clone();
        let act_term = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();

        btn_palette.connect_clicked(move |_| {
            let win_c = win_ref.clone();
            let tv_c = tv.clone();
            let st_c = Rc::clone(&st_rc_pal);
            let terms_c = Rc::clone(&terms_rc_pal);
            let css_c = css_prov_pal.clone();
            let b_nt_c = b_nt.clone();
            let b_sp_c = b_sp.clone();
            let b_sb_c = b_sb.clone();
            let b_fm_c = b_fm.clone();
            let b_ai_c = b_ai.clone();
            let b_sr_c = b_sr.clone();
            let b_gs_c = b_gs.clone();
            let b_ch_c = b_ch.clone();
            let b_bc_c = b_bc.clone();
            let b_cm_c = b_cm.clone();
            let b_rm_c = b_rm.clone();
            let b_ac_c = b_ac.clone();
            let st_act = status.clone();
            let at_act = Rc::clone(&act_term);

            let on_act: Rc<dyn Fn(&str)> = Rc::new(move |act: &str| match act {
                "global-search" => b_gs_c.emit_clicked(),
                "toggle-search" => b_sr_c.set_active(!b_sr_c.is_active()),
                "show-command-history" => b_ch_c.emit_clicked(),
                "command-manager" => b_cm_c.emit_clicked(),
                "new-tab" => b_nt_c.emit_clicked(),
                "close-tab" => {
                    if let Some(page) = tv_c.selected_page() {
                        if tv_c.n_pages() > 1 {
                            tv_c.close_page(&page);
                        }
                    }
                }
                "next-tab" => {
                    tv_c.select_next_page();
                }
                "previous-tab" => {
                    tv_c.select_previous_page();
                }
                "split" | "split-vertical" => b_sp_c.emit_clicked(),
                "export-terminal-buffer" => {
                    if let Some(term) = at_act() {
                        let tab_title = tv_c.selected_page().map(|p| p.title().to_string());
                        crate::export_dialog::show_export_terminal_dialog(
                            &win_c,
                            &term,
                            tab_title.as_deref(),
                            false,
                            &st_act,
                        );
                    }
                }
                "export-terminal-runbook" => {
                    if let Some(term) = at_act() {
                        let tab_title = tv_c.selected_page().map(|p| p.title().to_string());
                        crate::export_dialog::show_export_terminal_dialog(
                            &win_c,
                            &term,
                            tab_title.as_deref(),
                            true,
                            &st_act,
                        );
                    }
                }
                "clear-session" => {
                    if let Some(term) = at_act() {
                        term.reset(true, true);
                        term.feed_child(b"\x0c");
                        st_act.set_text(&tr("Terminal session cleared."));
                    }
                }
                "toggle-broadcast" => b_bc_c.set_active(!b_bc_c.is_active()),
                "zoom-in" | "zoom-out" | "zoom-reset" => {
                    let mut st = st_c.borrow_mut();
                    let (new_font, _) = match act {
                        "zoom-in" => change_font_size(&st.font, 1),
                        "zoom-out" => change_font_size(&st.font, -1),
                        _ => ("Monospace 11".to_string(), 11),
                    };
                    st.font = new_font;
                    for term in terms_c.borrow().iter() {
                        apply_terminal_appearance(term, &st);
                    }
                }
                "toggle-sidebar" => b_sb_c.set_active(!b_sb_c.is_active()),
                "toggle-fm" => b_fm_c.set_active(!b_fm_c.is_active()),
                "toggle-ai" => b_ai_c.set_active(!b_ai_c.is_active()),
                "resource-monitor" => b_rm_c.emit_clicked(),
                "autocomplete" => b_ac_c.emit_clicked(),
                "system-diagnostics" => {
                    crate::global_search_and_shortcuts::show_system_diagnostics_dialog(
                        &win_c, &st_act,
                    );
                }
                "shortcuts" => {
                    let cfg_path = platform::config_dir().join("settings.json");
                    crate::global_search_and_shortcuts::show_editable_shortcuts_dialog(
                        &win_c, &st_c, &cfg_path, &st_act,
                    );
                }
                "preferences" => {
                    crate::preferences_ui::show_preferences_window(
                        &win_c,
                        Rc::clone(&st_c),
                        Rc::clone(&terms_c),
                        css_c.clone(),
                        &st_act,
                    );
                }
                "colors" | "highlight-settings" => {
                    crate::highlight_dialog_ui::show_highlight_dialog(
                        &win_c,
                        Rc::clone(&st_c),
                        Rc::clone(&terms_c),
                        css_c.clone(),
                        &st_act,
                    );
                }
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
                        st_act.set_text(&tr(
                            "Git Assistant: diretório atual não é um repositório Git.",
                        ));
                    }
                }
                "backup-config" => {
                    let cfg_dir = platform::config_dir();
                    let out_path = platform::home_dir().join("onyxsh_config_backup.json");
                    match onyxsh_core::backup::create_config_backup(&cfg_dir, &out_path) {
                        Ok(bundle) => st_act.set_text(&tr_fmt(
                            "Backup SHA-256 criado ({count} arquivos): {path}",
                            &[
                                ("count", &bundle.files.len().to_string()),
                                ("path", &out_path.display().to_string()),
                            ],
                        )),
                        Err(e) => st_act.set_text(&tr_fmt(
                            "Erro ao criar backup: {err}",
                            &[("err", &e.to_string())],
                        )),
                    }
                }
                "show-tunnel-manager" => {
                    crate::tunnel_ui::show_tunnel_manager_window(&win_c, &st_act);
                }
                "configure-ai" => {
                    crate::ai_config_ui::show_ai_config_window(&win_c, Rc::clone(&st_c), &st_act);
                }
                "show-audit-log" | "audit-log" => {
                    crate::audit_log_dialog::show_audit_log_dialog(&win_c, &st_act);
                }
                "show-scope-dialog" | "agent-scope" => {
                    crate::agent_scope_dialog::show_agent_scope_dialog(&win_c, &st_c);
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
                    st_1.set_text(&tr_fmt(
                        "Conectando via Command Palette: {name}",
                        &[("name", &sess.name)],
                    ));
                }
            });

            let at_2 = Rc::clone(&act_term);
            let st_2 = status.clone();
            let on_cmd: Rc<dyn Fn(&str)> = Rc::new(move |cmd_tpl: &str| {
                if let Some(term) = at_2() {
                    term.feed_child(format!("{cmd_tpl}\n").as_bytes());
                    st_2.set_text(&tr_fmt(
                        "Comando executado via Palette: {cmd}",
                        &[("cmd", cmd_tpl)],
                    ));
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

    // Complete 34-Action Window-Level Keyboard Controller (`PropagationPhase::Capture`)
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);

        let app_k = app.clone();
        let win_k = window.clone();
        let tv_k = tab_view.clone();
        let st_k = Rc::clone(&settings_rc);
        let terms_k = Rc::clone(&terminals_rc);
        let css_k = css_provider.clone();
        let bs = btn_search.clone();
        let bgs = btn_global_search.clone();
        let bch = btn_cmd_history.clone();
        let bnt = btn_new_tab.clone();
        let bsp = btn_split.clone();
        let bsb = btn_sidebar.clone();
        let bfm = btn_fm.clone();
        let bai = btn_ai.clone();
        let bbc = btn_broadcast_toggle.clone();
        let bcm = btn_cmd_manager.clone();
        let bp = btn_palette.clone();
        let brm = btn_resource_monitor.clone();
        let act_term_k = Rc::clone(&active_term_cb);
        let status_k = status_lbl.clone();

        key_ctrl.connect_key_pressed(move |_, keyval, _, state| {
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
                    | gdk::Key::ISO_Level3_Shift
            );
            if is_mod_only {
                return glib::Propagation::Proceed;
            }

            let mods = state & gtk4::accelerator_get_default_mod_mask();

            let matches_action = |action_key: &str| -> bool {
                let st = st_k.borrow();
                let accel = match st.shortcuts.get(action_key) {
                    Some(s) if !s.is_empty() => s.as_str(),
                    _ => return false,
                };
                if let Some((target_keyval, target_mods)) = gtk4::accelerator_parse(accel) {
                    return target_keyval.to_lower() == keyval.to_lower() && target_mods == mods;
                }
                false
            };

            // 1. Tab & Window Management
            if matches_action("new-local-tab") {
                bnt.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("close-tab") {
                if let Some(page) = tv_k.selected_page() {
                    if tv_k.n_pages() > 1 {
                        tv_k.close_page(&page);
                        status_k.set_text(&tr("Aba encerrada."));
                    }
                }
                return glib::Propagation::Stop;
            }
            if matches_action("next-tab") {
                tv_k.select_next_page();
                return glib::Propagation::Stop;
            }
            if matches_action("previous-tab") {
                tv_k.select_previous_page();
                return glib::Propagation::Stop;
            }
            if matches_action("new-window") {
                let st_clone = st_k.borrow().clone();
                build_main_window(&app_k, st_clone, None);
                return glib::Propagation::Stop;
            }
            if matches_action("quit") {
                app_k.quit();
                return glib::Propagation::Stop;
            }

            // 2. Splitting & Panes
            if matches_action("split-horizontal") || matches_action("split-vertical") {
                bsp.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("close-pane") {
                if let Some(page) = tv_k.selected_page() {
                    if tv_k.n_pages() > 1 {
                        tv_k.close_page(&page);
                        status_k.set_text(&tr("Painel/Aba encerrado."));
                    }
                }
                return glib::Propagation::Stop;
            }
            if matches_action("focus-pane-up") || matches_action("focus-pane-left") {
                if let Some(page) = tv_k.selected_page() {
                    if let Ok(paned) = page.child().downcast::<gtk4::Paned>() {
                        if let Some(w) = paned.start_child() {
                            w.grab_focus();
                            status_k.set_text(&tr("Foco alterado para o primeiro painel."));
                            return glib::Propagation::Stop;
                        }
                    }
                }
            }
            if matches_action("focus-pane-down") || matches_action("focus-pane-right") {
                if let Some(page) = tv_k.selected_page() {
                    if let Ok(paned) = page.child().downcast::<gtk4::Paned>() {
                        if let Some(w) = paned.end_child() {
                            w.grab_focus();
                            status_k.set_text(&tr("Foco alterado para o segundo painel."));
                            return glib::Propagation::Stop;
                        }
                    }
                }
            }

            // 3. Search, Global Search, Command Palette & Command History
            if matches_action("toggle-search") {
                bs.set_active(!bs.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("global-search") {
                bgs.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("command-palette") {
                bp.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("show-command-history") {
                bch.emit_clicked();
                return glib::Propagation::Stop;
            }

            // 4. Panels & Overlays (Sidebar, File Manager, AI Assistant, Broadcast)
            if matches_action("toggle-sidebar") {
                bsb.set_active(!bsb.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("toggle-file-manager") {
                bfm.set_active(!bfm.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("ai-assistant") {
                bai.set_active(!bai.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("toggle-broadcast") {
                bbc.set_active(!bbc.is_active());
                return glib::Propagation::Stop;
            }
            if matches_action("show-command-manager") {
                bcm.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("opencode-launcher") {
                if let Some(term) = act_term_k() {
                    term.feed_child(b"opencode\n");
                    status_k.set_text(&tr("OpenCode iniciado no terminal."));
                }
                return glib::Propagation::Stop;
            }
            if matches_action("toggle-selection-mode") {
                if let Some(term) = act_term_k() {
                    let mode = !term.is_mouse_autohide();
                    term.set_mouse_autohide(mode);
                    status_k.set_text(&if mode {
                        tr("Modo Seleção de Texto ativado.")
                    } else {
                        tr("Modo Seleção de Texto desativado.")
                    });
                }
                return glib::Propagation::Stop;
            }
            if matches_action("jump-previous-prompt") {
                if let Some(term) = act_term_k() {
                    if let Some(adj) = term.vadjustment() {
                        let new_val = (adj.value() - adj.step_increment() * 5.0).max(adj.lower());
                        adj.set_value(new_val);
                        status_k.set_text(&tr("Pular para o prompt anterior."));
                    }
                }
                return glib::Propagation::Stop;
            }
            if matches_action("jump-next-prompt") {
                if let Some(term) = act_term_k() {
                    if let Some(adj) = term.vadjustment() {
                        let new_val = (adj.value() + adj.step_increment() * 5.0).min(adj.upper() - adj.page_size());
                        adj.set_value(new_val);
                        status_k.set_text(&tr("Pular para o próximo prompt."));
                    }
                }
                return glib::Propagation::Stop;
            }
            if matches_action("show-tunnel-manager") {
                show_tunnel_manager_dialog(&win_k, &status_k);
                return glib::Propagation::Stop;
            }
            if matches_action("show-resource-monitor") {
                brm.emit_clicked();
                return glib::Propagation::Stop;
            }
            if matches_action("toggle-tftp-server") {
                status_k.set_text(&tr("Servidor TFTP alternado."));
                return glib::Propagation::Stop;
            }

            // 5. Terminal Interaction (Copy, Paste, Select All, Clear)
            if matches_action("copy") {
                if let Some(term) = act_term_k() {
                    term.copy_clipboard_format(vte4::Format::Text);
                    status_k.set_text(&tr("Seleção copiada."));
                }
                return glib::Propagation::Stop;
            }
            if matches_action("paste") {
                if let Some(term) = act_term_k() {
                    term.paste_clipboard();
                }
                return glib::Propagation::Stop;
            }
            if matches_action("select-all") {
                if let Some(term) = act_term_k() {
                    term.select_all();
                }
                return glib::Propagation::Stop;
            }
            if matches_action("clear-session") {
                if let Some(term) = act_term_k() {
                    term.reset(true, true);
                    term.feed_child(b"\x0c");
                    status_k.set_text(&tr("Sessão limpa."));
                }
                return glib::Propagation::Stop;
            }

            // 6. Zoom In / Zoom Out / Reset Zoom (`zoom-in`, `zoom-out`, `zoom-reset`)
            if matches_action("zoom-in") {
                let mut st = st_k.borrow_mut();
                let (new_font, new_pt) = change_font_size(&st.font, 1);
                st.font = new_font;
                for term in terms_k.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                status_k.set_text(&format!("Zoom: {new_pt} pt"));
                return glib::Propagation::Stop;
            }
            if matches_action("zoom-out") {
                let mut st = st_k.borrow_mut();
                let (new_font, new_pt) = change_font_size(&st.font, -1);
                st.font = new_font;
                for term in terms_k.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                status_k.set_text(&format!("Zoom: {new_pt} pt"));
                return glib::Propagation::Stop;
            }
            if matches_action("zoom-reset") {
                let mut st = st_k.borrow_mut();
                st.font = "Monospace 11".to_string();
                for term in terms_k.borrow().iter() {
                    apply_terminal_appearance(term, &st);
                }
                status_k.set_text(&tr("Zoom restaurado (11 pt)."));
                return glib::Propagation::Stop;
            }

            // 7. Preferences (`preferences`)
            if matches_action("preferences") {
                crate::preferences_ui::show_preferences_window(
                    &win_k,
                    Rc::clone(&st_k),
                    Rc::clone(&terms_k),
                    css_k.clone(),
                    &status_k,
                );
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

    {
        let st_close = Rc::clone(&settings_rc);
        window.connect_close_request(move |_| {
            let cfg_path = platform::config_dir().join("settings.json");
            let _ = st_close.borrow().save_to_path(&cfg_path);
            glib::Propagation::Proceed
        });
    }

    window.set_content(Some(&root_vbox));
    window.present();
}
