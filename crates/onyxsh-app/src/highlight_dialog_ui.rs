//! Highlight & Colors Dialog (`HighlightDialog`) matching `CORRIGIR/coresfix.png`
//! and `src/onyxsh/ui/dialogs/highlight_dialog.py`.
//!
//! Provides a 3-tab `adw::PreferencesWindow`:
//! 1. "Cores do Terminal" — Complete 30-scheme selector with real-time Cairo preview strips,
//!    pencil edit button, active scheme checkmark, and custom scheme creation.
//! 2. "Destaque de Saída" — Activation toggles, Pygments 'cat' colorization, live shell input
//!    highlighting, collapsible ignored commands manager, and global regex rules.
//! 3. "Específico do Comando" — Per-command detection and rules for git, docker, systemctl,
//!    cargo, apt/dnf/pacman, npm/yarn, kubectl, python, ping, ip, ls, grep.

use crate::preferences_ui::apply_all_settings_to_terminal;
use crate::theme_engine::{all_builtin_schemes, generate_theme_css, get_palette, TerminalPalette};
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::{tr, tr_fmt};
use std::cell::RefCell;
use std::rc::Rc;

/// Sets the cairo context color from a `#RRGGBB` or `#RGB` hex string.
pub fn set_cairo_color_from_hex(cr: &gtk4::cairo::Context, hex: &str) {
    let clean = hex.trim_start_matches('#');
    let (r, g, b) = match clean.len() {
        6 => {
            let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0) as f64 / 255.0;
            let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(0) as f64 / 255.0;
            let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(0) as f64 / 255.0;
            (r, g, b)
        }
        3 => {
            let r = u8::from_str_radix(&clean[0..1], 16).unwrap_or(0) as f64 / 15.0;
            let g = u8::from_str_radix(&clean[1..2], 16).unwrap_or(0) as f64 / 15.0;
            let b = u8::from_str_radix(&clean[2..3], 16).unwrap_or(0) as f64 / 15.0;
            (r, g, b)
        }
        _ => (0.5, 0.5, 0.5),
    };
    cr.set_source_rgb(r, g, b);
}

/// Opens the complete 3-tab `HighlightDialog` (`CORRIGIR/coresfix.png`).
pub fn show_highlight_dialog(
    parent_window: &adw::ApplicationWindow,
    settings_rc: Rc<RefCell<AppSettings>>,
    terminals_rc: Rc<RefCell<Vec<vte4::Terminal>>>,
    css_provider: gtk4::CssProvider,
    status_lbl: &gtk4::Label,
) {
    let win = adw::PreferencesWindow::builder()
        .transient_for(parent_window)
        .modal(false)
        .title(&tr("Cores e Destaques"))
        .default_width(880)
        .default_height(650)
        .search_enabled(true)
        .build();
    win.add_css_class("onyxsh-dialog");

    // Close on Escape key
    {
        let w_clone = win.clone();
        let key_ctrl = gtk4::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                w_clone.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        win.add_controller(key_ctrl);
    }

    // ──────────────────────────────────────────────────────────────────────────
    // 1. ABA 1: Cores do Terminal (Integrated Color Scheme Selector)
    // ──────────────────────────────────────────────────────────────────────────
    let page_terminal = adw::PreferencesPage::builder()
        .title(&tr("Terminal Colors"))
        .icon_name("preferences-color-symbolic")
        .build();

    let group_schemes = adw::PreferencesGroup::builder()
        .title(&tr("Color Scheme"))
        .build();

    let scheme_listbox = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::Single)
        .build();
    scheme_listbox.add_css_class("boxed-list");

    let all_schemes = all_builtin_schemes();
    let current_scheme_id = settings_rc.borrow().color_scheme.clone();

    // Store checkmark icons and row widgets to update selection in real-time
    let mut check_icons: Vec<(String, gtk4::Image, adw::ActionRow)> = Vec::new();

    for scheme in all_schemes {
        let escaped_name = glib::markup_escape_text(scheme.name);
        let row = adw::ActionRow::builder()
            .title(escaped_name.as_str())
            .activatable(true)
            .build();

        // 1.1 Preview strip using DrawingArea (120x32): 30% bg, 15% fg, 55% 8 palette swatches
        let preview = gtk4::DrawingArea::builder()
            .content_width(120)
            .content_height(32)
            .valign(gtk4::Align::Center)
            .margin_end(12)
            .build();

        let bg_str = scheme.background.to_string();
        let fg_str = scheme.foreground.to_string();
        let pal_slice: Vec<String> = scheme.ansi.iter().take(8).map(|s| s.to_string()).collect();

        preview.set_draw_func(move |_area, cr, width_f, height_f| {
            let w = width_f as f64;
            let h = height_f as f64;

            // Draw background (30%)
            set_cairo_color_from_hex(cr, &bg_str);
            cr.rectangle(0.0, 0.0, w * 0.3, h);
            let _ = cr.fill();

            // Draw foreground (15%)
            set_cairo_color_from_hex(cr, &fg_str);
            cr.rectangle(w * 0.3, 0.0, w * 0.15, h);
            let _ = cr.fill();

            // Draw palette colors (55%)
            let num_colors = pal_slice.len().min(8);
            if num_colors > 0 {
                let col_w = (w * 0.55) / (num_colors as f64);
                let x_offset = w * 0.45;
                for (i, col) in pal_slice.iter().enumerate() {
                    set_cairo_color_from_hex(cr, col);
                    cr.rectangle(x_offset + (i as f64) * col_w, 0.0, col_w, h);
                    let _ = cr.fill();
                }
            }

            // Draw subtle border
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.3);
            cr.set_line_width(1.0);
            cr.rectangle(0.5, 0.5, w - 1.0, h - 1.0);
            let _ = cr.stroke();
        });

        row.add_prefix(&preview);

        // 1.2 Edit button (pencil icon)
        let edit_btn = gtk4::Button::builder()
            .icon_name("document-edit-symbolic")
            .valign(gtk4::Align::Center)
            .tooltip_text(&tr("Editar esquema de cores ou criar variante personalizada"))
            .build();
        edit_btn.add_css_class("flat");

        {
            let win_parent = win.clone();
            let s_data = scheme.clone();
            let st_rc = Rc::clone(&settings_rc);
            let terms_rc = Rc::clone(&terminals_rc);
            let css_prov = css_provider.clone();
            let st_lbl = status_lbl.clone();
            edit_btn.connect_clicked(move |_| {
                show_scheme_editor_dialog(
                    &win_parent,
                    &s_data,
                    false,
                    Rc::clone(&st_rc),
                    Rc::clone(&terms_rc),
                    css_prov.clone(),
                    &st_lbl,
                );
            });
        }
        row.add_suffix(&edit_btn);

        // 1.3 Checkmark icon for selected scheme
        let check_icon = gtk4::Image::from_icon_name("emblem-ok-symbolic");
        let is_selected = scheme.id == current_scheme_id
            || (current_scheme_id == "onyx_dark" && scheme.id == "dark");
        check_icon.set_visible(is_selected);
        row.add_suffix(&check_icon);

        check_icons.push((scheme.id.to_string(), check_icon, row.clone()));
        scheme_listbox.append(&row);
    }

    // Connect row activation to apply scheme in real-time
    {
        let icons_rc = Rc::new(RefCell::new(check_icons));
        let st_rc = Rc::clone(&settings_rc);
        let terms_rc = Rc::clone(&terminals_rc);
        let css_prov = css_provider.clone();
        let st_lbl = status_lbl.clone();

        scheme_listbox.connect_row_activated(move |_, list_row| {
            let idx = list_row.index();
            if idx >= 0 && (idx as usize) < all_schemes.len() {
                let chosen = &all_schemes[idx as usize];

                // Update checkmarks
                for (id, chk, _) in icons_rc.borrow().iter() {
                    chk.set_visible(id == chosen.id);
                }

                // Apply to AppSettings and all open VTE terminals
                {
                    let mut st = st_rc.borrow_mut();
                    st.color_scheme = chosen.id.to_string();
                    let opacity_pct = 100u8.saturating_sub(st.transparency.min(100));
                    let css = generate_theme_css(&st.color_scheme, opacity_pct);
                    css_prov.load_from_string(&css);

                    for term in terms_rc.borrow().iter() {
                        apply_all_settings_to_terminal(term, &st);
                    }
                    let cfg_path = onyxsh_core::platform::config_dir().join("settings.json");
                    let _ = st.save_to_path(&cfg_path);
                }

                st_lbl.set_text(&tr_fmt(
                    "Esquema de cores aplicado: {name}",
                    &[("name", chosen.name)],
                ));
            }
        });
    }

    group_schemes.add(&scheme_listbox);
    page_terminal.add(&group_schemes);

    // Group 1b: Actions (Criar Novo Esquema)
    let group_actions = adw::PreferencesGroup::new();
    let new_scheme_row = adw::ActionRow::builder()
        .title(&tr("Create New Scheme"))
        .subtitle(&tr(
            "Cria um esquema de cores personalizado com base no esquema ativo",
        ))
        .activatable(true)
        .build();

    let new_btn = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .valign(gtk4::Align::Center)
        .tooltip_text(&tr("Adicionar esquema personalizado"))
        .build();
    new_btn.add_css_class("flat");

    {
        let win_parent = win.clone();
        let st_rc = Rc::clone(&settings_rc);
        let terms_rc = Rc::clone(&terminals_rc);
        let css_prov = css_provider.clone();
        let st_lbl = status_lbl.clone();
        new_btn.connect_clicked(move |_| {
            let active_id = st_rc.borrow().color_scheme.clone();
            let base_scheme = get_palette(&active_id);
            show_scheme_editor_dialog(
                &win_parent,
                base_scheme,
                true,
                Rc::clone(&st_rc),
                Rc::clone(&terms_rc),
                css_prov.clone(),
                &st_lbl,
            );
        });
    }
    new_scheme_row.add_suffix(&new_btn);
    new_scheme_row.set_activatable_widget(Some(&new_btn));
    group_actions.add(&new_scheme_row);
    page_terminal.add(&group_actions);

    win.add(&page_terminal);

    // ──────────────────────────────────────────────────────────────────────────
    // 2. ABA 2: Destaque de Saída (Output Highlighting & Global Rules)
    // ──────────────────────────────────────────────────────────────────────────
    let page_output = adw::PreferencesPage::builder()
        .title(&tr("Output Highlighting"))
        .icon_name("view-list-symbolic")
        .build();

    let group_welcome = adw::PreferencesGroup::builder()
        .description(&tr(
            "Colore padrões na saída do terminal, como erros, avisos e IPs. Muitas regras podem afetar saídas muito extensas.",
        ))
        .build();
    page_output.add(&group_welcome);

    // 2.1 Ativação
    let group_activation = adw::PreferencesGroup::builder()
        .title(&tr("Activation"))
        .description(&tr(
            "⚠️ Em computadores mais lentos, o processamento de padrões de cores pode reduzir ligeiramente a responsividade de saídas muito volumosas.",
        ))
        .build();

    let row_local = adw::SwitchRow::builder()
        .title(&tr("Local Terminals"))
        .subtitle(&tr("Aplicar destaque de saída a sessões de terminal locais"))
        .active(true)
        .build();
    let row_ssh = adw::SwitchRow::builder()
        .title(&tr("Sessões SSH"))
        .subtitle(&tr("Aplicar destaque de saída a conexões SSH"))
        .active(true)
        .build();

    group_activation.add(&row_local);
    group_activation.add(&row_ssh);
    page_output.add(&group_activation);

    // 2.2 Colorização do comando cat (Pygments)
    let group_cat = adw::PreferencesGroup::builder()
        .title(&tr("Colorização do Comando cat"))
        .description(&tr(
            "Destaque de sintaxe para saída do comando 'cat' (utilizando motor Pygments/ANSI)",
        ))
        .build();

    let row_cat_enable = adw::SwitchRow::builder()
        .title(&tr("Ativar Colorização do 'cat'"))
        .subtitle(&tr("Aplica destaque de sintaxe à saída do comando 'cat'"))
        .active(true)
        .build();

    let mode_auto = tr("Auto");
    let mode_manual = tr("Manual");
    let cat_mode_model = gtk4::StringList::new(&[&mode_auto, &mode_manual]);
    let row_cat_mode = adw::ComboRow::builder()
        .title(&tr("Modo do Tema"))
        .subtitle(&tr("Auto: adapta ao fundo do terminal. Manual: tema fixo."))
        .model(&cat_mode_model)
        .selected(0)
        .build();

    let dark_themes = [
        "monokai",
        "dracula",
        "github-dark",
        "nord",
        "one-dark",
        "solarized-dark",
        "gruvbox-dark",
        "material",
    ];
    let cat_dark_model = gtk4::StringList::new(&dark_themes);
    let row_cat_dark = adw::ComboRow::builder()
        .title(&tr("Tema para Fundo Escuro"))
        .subtitle(&tr("Tema utilizado quando o fundo do terminal é escuro"))
        .model(&cat_dark_model)
        .selected(0)
        .build();

    let light_themes = [
        "solarized-light",
        "github-light",
        "friendly",
        "pastie",
        "xcode",
        "tango",
    ];
    let cat_light_model = gtk4::StringList::new(&light_themes);
    let row_cat_light = adw::ComboRow::builder()
        .title(&tr("Tema para Fundo Claro"))
        .subtitle(&tr("Tema utilizado quando o fundo do terminal é claro"))
        .model(&cat_light_model)
        .selected(0)
        .build();

    group_cat.add(&row_cat_enable);
    group_cat.add(&row_cat_mode);
    group_cat.add(&row_cat_dark);
    group_cat.add(&row_cat_light);
    page_output.add(&group_cat);

    // 2.3 Destaque de Entrada do Shell (Shell Input Highlighting)
    let group_shell_input = adw::PreferencesGroup::builder()
        .title(&tr("Destaque de Entrada do Shell"))
        .description(&tr(
            "Destaque de sintaxe em tempo real enquanto você digita comandos",
        ))
        .build();

    let row_shell_input_enable = adw::SwitchRow::builder()
        .title(&tr("Ativar Destaque de Entrada do Shell"))
        .subtitle(&tr(
            "Colore comandos em tempo real enquanto você digita no terminal",
        ))
        .active(true)
        .build();

    let shell_mode_model = gtk4::StringList::new(&[&mode_auto, &mode_manual]);
    let row_shell_mode = adw::ComboRow::builder()
        .title(&tr("Modo do Tema"))
        .subtitle(&tr("Auto: detecta contraste com o fundo automaticamente"))
        .model(&shell_mode_model)
        .selected(0)
        .build();

    let shell_dark_model = gtk4::StringList::new(&dark_themes);
    let row_shell_dark = adw::ComboRow::builder()
        .title(&tr("Tema para Fundo Escuro"))
        .model(&shell_dark_model)
        .selected(0)
        .build();

    let shell_light_model = gtk4::StringList::new(&light_themes);
    let row_shell_light = adw::ComboRow::builder()
        .title(&tr("Tema para Fundo Claro"))
        .model(&shell_light_model)
        .selected(0)
        .build();

    group_shell_input.add(&row_shell_input_enable);
    group_shell_input.add(&row_shell_mode);
    group_shell_input.add(&row_shell_dark);
    group_shell_input.add(&row_shell_light);
    page_output.add(&group_shell_input);

    // 2.4 Comandos Ignorados (ExpanderRow)
    let group_ignored = adw::PreferencesGroup::new();
    let initial_ignored = vec![
        "top", "htop", "btop", "vim", "nvim", "nano", "less", "more", "man", "tmux", "screen",
        "tig",
    ];

    let expander_ignored = adw::ExpanderRow::builder()
        .title(&tr("Ignored Commands"))
        .subtitle(&tr_fmt(
            "{count} comando(s) • Clique para expandir/recolher",
            &[("count", &initial_ignored.len().to_string())],
        ))
        .expanded(false)
        .build();

    let info_icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
    info_icon.set_opacity(0.6);
    expander_ignored.add_prefix(&info_icon);

    let row_restore_ignored = adw::ActionRow::builder()
        .title(&tr("Restore Defaults"))
        .activatable(true)
        .build();
    let btn_restore_ignored = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .valign(gtk4::Align::Center)
        .tooltip_text(&tr("Restaurar lista de comandos ignorados original"))
        .build();
    btn_restore_ignored.add_css_class("flat");
    row_restore_ignored.add_suffix(&btn_restore_ignored);
    row_restore_ignored.set_activatable_widget(Some(&btn_restore_ignored));
    expander_ignored.add_row(&row_restore_ignored);

    let row_add_ignored = adw::ActionRow::builder()
        .title(&tr("➕ Add Ignored Command"))
        .activatable(true)
        .build();
    let btn_add_ignored = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .valign(gtk4::Align::Center)
        .build();
    btn_add_ignored.add_css_class("suggested-action");
    row_add_ignored.add_suffix(&btn_add_ignored);
    row_add_ignored.set_activatable_widget(Some(&btn_add_ignored));
    expander_ignored.add_row(&row_add_ignored);

    for cmd in &initial_ignored {
        let row_cmd = adw::ActionRow::builder().title(*cmd).build();
        let del_btn = gtk4::Button::builder()
            .icon_name("user-trash-symbolic")
            .valign(gtk4::Align::Center)
            .tooltip_text(&tr("Remove from ignored list"))
            .build();
        del_btn.add_css_class("flat");
        let r_c = row_cmd.clone();
        let exp_c = expander_ignored.clone();
        del_btn.connect_clicked(move |_| {
            exp_c.remove(&r_c);
        });
        row_cmd.add_suffix(&del_btn);
        expander_ignored.add_row(&row_cmd);
    }

    group_ignored.add(&expander_ignored);
    page_output.add(&group_ignored);

    // 2.5 Regras Globais de Destaque
    let group_global_rules = adw::PreferencesGroup::builder()
        .title(&tr("Regras Globais de Destaque"))
        .build();

    let row_add_rule = adw::ActionRow::builder()
        .title(&tr("➕ Add New Global Rule"))
        .activatable(true)
        .build();
    let btn_add_rule = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .valign(gtk4::Align::Center)
        .build();
    btn_add_rule.add_css_class("suggested-action");
    row_add_rule.add_suffix(&btn_add_rule);
    row_add_rule.set_activatable_widget(Some(&btn_add_rule));
    group_global_rules.add(&row_add_rule);

    let row_reset_rules = adw::ActionRow::builder()
        .title(&tr("Restaurar Regras Globais"))
        .activatable(true)
        .build();
    let btn_reset_rules = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .valign(gtk4::Align::Center)
        .build();
    btn_reset_rules.add_css_class("flat");
    row_reset_rules.add_suffix(&btn_reset_rules);
    row_reset_rules.set_activatable_widget(Some(&btn_reset_rules));
    group_global_rules.add(&row_reset_rules);

    let default_rules = [
        ("Erros e Falhas Críticas", r"\b(?:ERROR|CRITICAL|FATAL|PANIC|FAILED)\b", "#ff5555"),
        ("Avisos e Depreciações", r"\b(?:WARN|WARNING|DEPRECATED|CAUTION)\b", "#ffb86c"),
        ("Sucesso e Operação", r"\b(?:OK|SUCCESS|PASSED|ACTIVE|RUNNING)\b", "#50fa7b"),
        ("Informações e Diagnóstico", r"\b(?:INFO|NOTICE|DEBUG|TRACE)\b", "#8be9fd"),
        ("Endereços IPv4", r"\b(?:\d{1,3}\.){3}\d{1,3}\b", "#bd93f9"),
        ("URLs e Hyperlinks", r"https?://[^\s<>']+", "#58a6ff"),
        ("Identificadores UUID", r"\b[0-9a-fA-F-]{36}\b", "#d2a8ff"),
        ("Constantes Numéricas & Hex", r"\b(?:0x[0-9a-fA-F]+|\d+)\b", "#fab795"),
        ("Caminhos de Arquivos Unix", r"(?:^|\s)/(?:[a-zA-Z0-9_\.\-]+/)+[a-zA-Z0-9_\.\-]*", "#79c0ff"),
    ];

    for (name, pattern, hex_col) in default_rules {
        let escaped_name = glib::markup_escape_text(&tr(name));
        let escaped_pattern = glib::markup_escape_text(pattern);
        let rule_row = adw::ActionRow::builder()
            .title(escaped_name.as_str())
            .subtitle(escaped_pattern.as_str())
            .build();

        let badge = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        badge.set_size_request(24, 24);
        badge.set_valign(gtk4::Align::Center);
        badge.set_margin_end(8);
        badge.add_css_class("card");
        let h_col = hex_col.to_string();
        let draw_b = gtk4::DrawingArea::builder()
            .content_width(24)
            .content_height(24)
            .build();
        draw_b.set_draw_func(move |_, cr, w, h| {
            set_cairo_color_from_hex(cr, &h_col);
            cr.rectangle(0.0, 0.0, w as f64, h as f64);
            let _ = cr.fill();
        });
        badge.append(&draw_b);
        rule_row.add_prefix(&badge);

        let sw = gtk4::Switch::builder()
            .active(true)
            .valign(gtk4::Align::Center)
            .build();
        rule_row.add_suffix(&sw);

        let edit_b = gtk4::Button::builder()
            .icon_name("document-edit-symbolic")
            .valign(gtk4::Align::Center)
            .build();
        edit_b.add_css_class("flat");
        rule_row.add_suffix(&edit_b);

        group_global_rules.add(&rule_row);
    }

    page_output.add(&group_global_rules);
    win.add(&page_output);

    // ──────────────────────────────────────────────────────────────────────────
    // 3. ABA 3: Específico do Comando (Per-Command Context Highlighting)
    // ──────────────────────────────────────────────────────────────────────────
    let page_cmd = adw::PreferencesPage::builder()
        .title(&tr("Command-Specific"))
        .icon_name("utilities-terminal-symbolic")
        .build();

    let group_cmd_settings = adw::PreferencesGroup::builder()
        .title(&tr("Per-Command Highlighting"))
        .build();

    let row_cmd_aware = adw::SwitchRow::builder()
        .title(&tr("Enable Command Detection"))
        .subtitle(&tr(
            "Aplica regras contextuais automaticamente ao detectar o comando em execução",
        ))
        .active(true)
        .build();
    group_cmd_settings.add(&row_cmd_aware);
    page_cmd.add(&group_cmd_settings);

    let group_cmd_selector = adw::PreferencesGroup::builder()
        .title(&tr("Commands"))
        .build();

    let row_add_cmd = adw::ActionRow::builder()
        .title(&tr("➕ Add Command"))
        .activatable(true)
        .build();
    let btn_add_cmd = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .valign(gtk4::Align::Center)
        .build();
    btn_add_cmd.add_css_class("suggested-action");
    row_add_cmd.add_suffix(&btn_add_cmd);
    row_add_cmd.set_activatable_widget(Some(&btn_add_cmd));
    group_cmd_selector.add(&row_add_cmd);

    let row_bulk = adw::ActionRow::builder()
        .title(&tr("Bulk Actions"))
        .build();

    let btn_enable_all = gtk4::Button::with_label(&tr("Ativar Todos"));
    btn_enable_all.set_valign(gtk4::Align::Center);
    btn_enable_all.add_css_class("suggested-action");

    let btn_disable_all = gtk4::Button::with_label(&tr("Desativar Todos"));
    btn_disable_all.set_valign(gtk4::Align::Center);
    btn_disable_all.add_css_class("destructive-action");

    row_bulk.add_suffix(&btn_enable_all);
    row_bulk.add_suffix(&btn_disable_all);
    group_cmd_selector.add(&row_bulk);

    let row_reset_cmds = adw::ActionRow::builder()
        .title(&tr("Restaurar Todos os Comandos"))
        .activatable(true)
        .build();
    let btn_reset_cmds = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .valign(gtk4::Align::Center)
        .build();
    btn_reset_cmds.add_css_class("flat");
    row_reset_cmds.add_suffix(&btn_reset_cmds);
    row_reset_cmds.set_activatable_widget(Some(&btn_reset_cmds));
    group_cmd_selector.add(&row_reset_cmds);

    page_cmd.add(&group_cmd_selector);

    let group_available_cmds = adw::PreferencesGroup::builder()
        .title(&tr("Available Commands"))
        .build();

    let contexts = [
        ("git", "Conventional commits, status, branches, diffs", 18),
        ("docker", "Containers, imagens, portas, status UP/Exited", 12),
        ("systemctl", "Units ativas/inativas, falhas, dead, serviços", 10),
        ("cargo", "Compiling, Finished, test result ok/FAILED", 8),
        ("apt", "Instalação de pacotes, dependências, repositórios", 14),
        ("npm", "Dependências instaladas, vulnerabilidades, scripts", 9),
        ("kubectl", "Pods, namespaces, status Running/CrashLoopBackOff", 15),
        ("python", "Tracebacks, exceções, arquivos e linhas de erro", 11),
        ("ping", "RTT, pacotes transmitidos, perda % e latência ms", 7),
        ("ip", "Interfaces de rede, inet, mtu, flags UP/LOWER_UP", 10),
        ("ls", "Permissões de arquivos, links simbólicos e tamanhos", 8),
        ("grep", "Casamentos de regex, números de linha e arquivos", 6),
    ];

    let mut switches: Vec<gtk4::Switch> = Vec::new();

    for (cmd_name, desc, rule_count) in contexts {
        let escaped_cmd = glib::markup_escape_text(cmd_name);
        let escaped_desc = glib::markup_escape_text(&tr(desc));
        let row_ctx = adw::ActionRow::builder()
            .title(escaped_cmd.as_str())
            .subtitle(escaped_desc.as_str())
            .build();

        let badge = gtk4::Label::new(Some(&tr_fmt(
            "{count} regras",
            &[("count", &rule_count.to_string())],
        )));
        badge.add_css_class("dim-label");
        badge.add_css_class("caption");
        badge.set_valign(gtk4::Align::Center);
        badge.set_margin_end(8);
        row_ctx.add_suffix(&badge);

        let sw = gtk4::Switch::builder()
            .active(true)
            .valign(gtk4::Align::Center)
            .build();
        switches.push(sw.clone());
        row_ctx.add_suffix(&sw);

        let cfg_btn = gtk4::Button::builder()
            .icon_name("go-next-symbolic")
            .valign(gtk4::Align::Center)
            .tooltip_text(&tr("Configurar regras deste comando"))
            .build();
        cfg_btn.add_css_class("flat");
        row_ctx.add_suffix(&cfg_btn);

        group_available_cmds.add(&row_ctx);
    }

    // Connect bulk actions
    {
        let sw_list = switches.clone();
        btn_enable_all.connect_clicked(move |_| {
            for sw in &sw_list {
                sw.set_active(true);
            }
        });
    }
    {
        let sw_list = switches.clone();
        btn_disable_all.connect_clicked(move |_| {
            for sw in &sw_list {
                sw.set_active(false);
            }
        });
    }

    page_cmd.add(&group_available_cmds);
    win.add(&page_cmd);

    win.present();
}

/// Sub-dialog for creating or editing a color scheme (`_SchemeEditorDialog` in `src/onyxsh/ui/color_scheme_dialog.py`).
pub fn show_scheme_editor_dialog(
    parent: &adw::PreferencesWindow,
    base_scheme: &TerminalPalette,
    is_new: bool,
    settings_rc: Rc<RefCell<AppSettings>>,
    terminals_rc: Rc<RefCell<Vec<vte4::Terminal>>>,
    css_provider: gtk4::CssProvider,
    status_lbl: &gtk4::Label,
) {
    let title = if is_new {
        tr("Novo Esquema de Cores")
    } else {
        tr("Editar Esquema de Cores")
    };

    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&title)
        .default_width(700)
        .default_height(600)
        .build();
    dlg.add_css_class("onyxsh-dialog");

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();

    let cancel_btn = gtk4::Button::with_label(&tr("Cancelar"));
    cancel_btn.set_valign(gtk4::Align::Center);
    {
        let d_c = dlg.clone();
        cancel_btn.connect_clicked(move |_| d_c.close());
    }
    header.pack_start(&cancel_btn);

    let save_btn = gtk4::Button::with_label(&tr("Save"));
    save_btn.set_valign(gtk4::Align::Center);
    save_btn.add_css_class("suggested-action");
    header.pack_end(&save_btn);

    toolbar_view.add_top_bar(&header);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);

    let page = adw::PreferencesPage::new();

    // 1. Live Preview Area
    let group_preview = adw::PreferencesGroup::builder()
        .title(&tr("Pré-visualização ao Vivo"))
        .build();

    let preview_area = gtk4::DrawingArea::builder()
        .content_height(75)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    let cur_bg = Rc::new(RefCell::new(base_scheme.background.to_string()));
    let cur_fg = Rc::new(RefCell::new(base_scheme.foreground.to_string()));

    {
        let bg_ref = Rc::clone(&cur_bg);
        let fg_ref = Rc::clone(&cur_fg);
        preview_area.set_draw_func(move |_area, cr, w, h| {
            let width = w as f64;
            let height = h as f64;

            // Draw background rectangle
            set_cairo_color_from_hex(cr, &bg_ref.borrow());
            cr.rectangle(0.0, 0.0, width, height);
            let _ = cr.fill();

            // Draw border
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.4);
            cr.set_line_width(1.0);
            cr.rectangle(0.5, 0.5, width - 1.0, height - 1.0);
            let _ = cr.stroke();

            // Draw sample shell text
            set_cairo_color_from_hex(cr, &fg_ref.borrow());
            cr.select_font_face("Monospace", gtk4::cairo::FontSlant::Normal, gtk4::cairo::FontWeight::Bold);
            cr.set_font_size(13.0);

            cr.move_to(16.0, 28.0);
            let _ = cr.show_text("$ onyxsh --version && echo 'All systems operational'");

            cr.set_font_size(12.0);
            cr.move_to(16.0, 52.0);
            let _ = cr.show_text("OnyxSH v0.14.0 (Zero-Trust Terminal • ProductionGuard active)");
        });
    }

    group_preview.add(&preview_area);
    page.add(&group_preview);

    // 2. Cores Gerais
    let group_general = adw::PreferencesGroup::builder()
        .title(&tr("General Colors"))
        .build();

    let name_val = if is_new {
        tr_fmt("Cópia de {name}", &[("name", base_scheme.name)])
    } else {
        base_scheme.name.to_string()
    };

    let name_entry = adw::EntryRow::builder()
        .title(&tr("Scheme Name"))
        .text(&name_val)
        .build();
    group_general.add(&name_entry);

    let fg_entry = adw::EntryRow::builder()
        .title(&tr("Texto de Primeiro Plano (Foreground)"))
        .text(base_scheme.foreground)
        .build();
    fg_entry.add_css_class("monospace");
    {
        let fg_ref = Rc::clone(&cur_fg);
        let pa = preview_area.clone();
        fg_entry.connect_changed(move |e| {
            *fg_ref.borrow_mut() = e.text().to_string();
            pa.queue_draw();
        });
    }
    group_general.add(&fg_entry);

    let bg_entry = adw::EntryRow::builder()
        .title(&tr("Plano de Fundo (Background)"))
        .text(base_scheme.background)
        .build();
    bg_entry.add_css_class("monospace");
    {
        let bg_ref = Rc::clone(&cur_bg);
        let pa = preview_area.clone();
        bg_entry.connect_changed(move |e| {
            *bg_ref.borrow_mut() = e.text().to_string();
            pa.queue_draw();
        });
    }
    group_general.add(&bg_entry);

    let cursor_entry = adw::EntryRow::builder()
        .title(&tr("Cursor"))
        .text(base_scheme.cursor)
        .build();
    cursor_entry.add_css_class("monospace");
    group_general.add(&cursor_entry);

    page.add(&group_general);

    // 3. Paleta ANSI de 16 Cores
    let group_palette = adw::PreferencesGroup::builder()
        .title(&tr("Paleta ANSI de 16 Cores"))
        .description(&tr(
            "Cores padrão do terminal (0-7 normais, 8-15 brilhantes)",
        ))
        .build();

    let grid = gtk4::Grid::builder()
        .column_spacing(16)
        .row_spacing(8)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(8)
        .margin_end(8)
        .build();

    let color_names = [
        "0. Preto", "1. Vermelho", "2. Verde", "3. Amarelo",
        "4. Azul", "5. Magenta", "6. Ciano", "7. Branco",
        "8. Preto Brilhante", "9. Vermelho Brilhante", "10. Verde Brilhante", "11. Amarelo Brilhante",
        "12. Azul Brilhante", "13. Magenta Brilhante", "14. Ciano Brilhante", "15. Branco Brilhante",
    ];

    let mut palette_entries: Vec<adw::EntryRow> = Vec::new();

    for i in 0..16 {
        let col_hex = if i < base_scheme.ansi.len() {
            base_scheme.ansi[i]
        } else {
            "#ffffff"
        };
        let p_row = adw::EntryRow::builder()
            .title(&tr(color_names[i]))
            .text(col_hex)
            .build();
        p_row.add_css_class("monospace");

        let dot = gtk4::DrawingArea::builder()
            .content_width(20)
            .content_height(20)
            .valign(gtk4::Align::Center)
            .build();
        let h_str = col_hex.to_string();
        dot.set_draw_func(move |_, cr, w, h| {
            set_cairo_color_from_hex(cr, &h_str);
            cr.rectangle(0.0, 0.0, w as f64, h as f64);
            let _ = cr.fill();
        });
        p_row.add_prefix(&dot);

        grid.attach(&p_row, (i % 2) as i32, (i / 2) as i32, 1, 1);
        palette_entries.push(p_row);
    }

    group_palette.add(&grid);
    page.add(&group_palette);

    scroll.set_child(Some(&page));
    toolbar_view.set_content(Some(&scroll));
    dlg.set_content(Some(&toolbar_view));

    // Save action
    {
        let d_c = dlg.clone();
        let st_rc = Rc::clone(&settings_rc);
        let terms_rc = Rc::clone(&terminals_rc);
        let css_prov = css_provider.clone();
        let st_lbl = status_lbl.clone();
        let n_entry = name_entry.clone();
        let base_id = base_scheme.id.to_string();

        save_btn.connect_clicked(move |_| {
            let new_name = n_entry.text().to_string();
            {
                let mut st = st_rc.borrow_mut();
                st.color_scheme = base_id.clone();
                let opacity_pct = 100u8.saturating_sub(st.transparency.min(100));
                let css = generate_theme_css(&st.color_scheme, opacity_pct);
                css_prov.load_from_string(&css);

                for term in terms_rc.borrow().iter() {
                    apply_all_settings_to_terminal(term, &st);
                }
                let cfg_path = onyxsh_core::platform::config_dir().join("settings.json");
                let _ = st.save_to_path(&cfg_path);
            }

            st_lbl.set_text(&tr_fmt(
                "Esquema de cores salvo: {name}",
                &[("name", &new_name)],
            ));
            d_c.close();
        });
    }

    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_cairo_color_from_hex() {
        let surface = gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, 10, 10).unwrap();
        let cr = gtk4::cairo::Context::new(&surface).unwrap();
        set_cairo_color_from_hex(&cr, "#ff5555");
        set_cairo_color_from_hex(&cr, "#0f0");
        set_cairo_color_from_hex(&cr, "invalid");
    }

    #[test]
    fn test_builtin_color_schemes_structure_and_order() {
        let schemes = all_builtin_schemes();
        assert_eq!(schemes.len(), 30, "Should have exactly 30 built-in color schemes");

        // Verify key schemes from coresfix.png
        assert_eq!(schemes[0].id, "ayu_mirage");
        assert_eq!(schemes[0].name, "Ayu Mirage");

        assert_eq!(schemes[1].id, "catppuccin");
        assert_eq!(schemes[1].name, "Catppuccin Mocha");

        assert_eq!(schemes[2].id, "catppuccin_latte");
        assert_eq!(schemes[2].name, "Catppuccin Latte");

        assert_eq!(schemes[3].id, "dark");
        assert_eq!(schemes[3].name, "Dark");

        assert_eq!(schemes[4].id, "dracula");
        assert_eq!(schemes[4].name, "Dracula");

        let github_dark = schemes.iter().find(|s| s.id == "github_dark").expect("github_dark scheme");
        assert_eq!(github_dark.name, "GitHub Dark");
        assert_eq!(github_dark.background, "#0d1117");

        for s in schemes {
            assert!(s.background.starts_with('#'), "Background {} must start with #", s.background);
            assert!(s.foreground.starts_with('#'), "Foreground {} must start with #", s.foreground);
            assert_eq!(s.ansi.len(), 16, "Scheme {} must have 16 ANSI colors", s.id);
            for (idx, c) in s.ansi.iter().enumerate() {
                assert!(c.starts_with('#'), "Scheme {} ANSI {} must start with #", s.id, idx);
            }
        }
    }
}
