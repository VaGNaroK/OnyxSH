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
use crate::theme_engine::{
    all_available_schemes, delete_custom_scheme, generate_theme_css, resolve_palette,
    save_custom_scheme, slugify_scheme_id, OwnedTerminalPalette,
};
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
fn sgr_name_to_preview_hex(spec: Option<&str>) -> String {
    let raw = spec.unwrap_or("yellow").trim();
    if raw.starts_with('#') && raw.len() == 7 {
        return raw.to_string();
    }
    let lower = raw.to_lowercase();
    if lower.contains("red") {
        "#ff5555".to_string()
    } else if lower.contains("green") {
        "#50fa7b".to_string()
    } else if lower.contains("yellow") {
        "#f1fa8c".to_string()
    } else if lower.contains("blue") {
        "#58a6ff".to_string()
    } else if lower.contains("magenta") {
        "#bd93f9".to_string()
    } else if lower.contains("cyan") {
        "#8be9fd".to_string()
    } else if lower.contains("white") {
        "#f8f8f2".to_string()
    } else {
        "#ffb86c".to_string()
    }
}

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

    let schemes_rc: Rc<RefCell<Vec<OwnedTerminalPalette>>> = Rc::new(RefCell::new(Vec::new()));
    let check_icons_rc: Rc<RefCell<Vec<(String, gtk4::Image, adw::ActionRow)>>> =
        Rc::new(RefCell::new(Vec::new()));

    let rebuild_scheme_list: Rc<dyn Fn()> = {
        let listbox = scheme_listbox.clone();
        let schemes_ref = Rc::clone(&schemes_rc);
        let icons_ref = Rc::clone(&check_icons_rc);
        let win_parent = win.clone();
        let st_rc = Rc::clone(&settings_rc);
        let terms_rc = Rc::clone(&terminals_rc);
        let css_prov = css_provider.clone();
        let st_lbl = status_lbl.clone();

        Rc::new(move || {
            let slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
            fn build_list(
                listbox: &gtk4::ListBox,
                schemes_ref: &Rc<RefCell<Vec<OwnedTerminalPalette>>>,
                icons_ref: &Rc<RefCell<Vec<(String, gtk4::Image, adw::ActionRow)>>>,
                win_parent: &adw::PreferencesWindow,
                st_rc: &Rc<RefCell<AppSettings>>,
                terminals_rc: &Rc<RefCell<Vec<vte4::Terminal>>>,
                css_prov: &gtk4::CssProvider,
                st_lbl: &gtk4::Label,
            ) {
                while let Some(child) = listbox.first_child() {
                    listbox.remove(&child);
                }
                icons_ref.borrow_mut().clear();

                let all_schemes = all_available_schemes();
                *schemes_ref.borrow_mut() = all_schemes.clone();
                let current_scheme_id = st_rc.borrow().color_scheme.clone();

                for scheme in all_schemes {
                    let escaped_name = glib::markup_escape_text(&scheme.name);
                    let row = adw::ActionRow::builder()
                        .title(escaped_name.as_str())
                        .activatable(true)
                        .build();

                    let preview = gtk4::DrawingArea::builder()
                        .content_width(120)
                        .content_height(32)
                        .valign(gtk4::Align::Center)
                        .margin_end(12)
                        .build();

                    let bg_str = scheme.background.clone();
                    let fg_str = scheme.foreground.clone();
                    let pal_slice: Vec<String> = scheme.ansi.iter().take(8).cloned().collect();

                    preview.set_draw_func(move |_area, cr, width_f, height_f| {
                        let w = width_f as f64;
                        let h = height_f as f64;

                        set_cairo_color_from_hex(cr, &bg_str);
                        cr.rectangle(0.0, 0.0, w * 0.3, h);
                        let _ = cr.fill();

                        set_cairo_color_from_hex(cr, &fg_str);
                        cr.rectangle(w * 0.3, 0.0, w * 0.15, h);
                        let _ = cr.fill();

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

                        cr.set_source_rgba(0.5, 0.5, 0.5, 0.3);
                        cr.set_line_width(1.0);
                        cr.rectangle(0.5, 0.5, w - 1.0, h - 1.0);
                        let _ = cr.stroke();
                    });

                    row.add_prefix(&preview);

                    if scheme.is_custom {
                        let del_btn = gtk4::Button::builder()
                            .icon_name("user-trash-symbolic")
                            .valign(gtk4::Align::Center)
                            .tooltip_text(&tr("Delete"))
                            .build();
                        del_btn.add_css_class("flat");
                        let del_id = scheme.id.clone();
                        let lb_d = listbox.clone();
                        let sr_d = Rc::clone(schemes_ref);
                        let ir_d = Rc::clone(icons_ref);
                        let wp_d = win_parent.clone();
                        let st_d = Rc::clone(st_rc);
                        let tm_d = Rc::clone(terminals_rc);
                        let cp_d = css_prov.clone();
                        let sl_d = st_lbl.clone();
                        del_btn.connect_clicked(move |_| {
                            if delete_custom_scheme(&del_id).unwrap_or(false) {
                                {
                                    let mut st = st_d.borrow_mut();
                                    if st.color_scheme == del_id {
                                        st.color_scheme = "onyx_dark".to_string();
                                        let opacity_pct = 100u8.saturating_sub(st.transparency.min(100));
                                        let css = generate_theme_css(&st.color_scheme, opacity_pct);
                                        cp_d.load_from_string(&css);
                                        for term in tm_d.borrow().iter() {
                                            apply_all_settings_to_terminal(term, &st);
                                        }
                                        let cfg_path = onyxsh_core::platform::config_dir().join("settings.json");
                                        let _ = st.save_to_path(&cfg_path);
                                    }
                                }
                                build_list(&lb_d, &sr_d, &ir_d, &wp_d, &st_d, &tm_d, &cp_d, &sl_d);
                            }
                        });
                        row.add_suffix(&del_btn);
                    }

                    let edit_btn = gtk4::Button::builder()
                        .icon_name("document-edit-symbolic")
                        .valign(gtk4::Align::Center)
                        .tooltip_text(&tr("Editar esquema de cores ou criar variante personalizada"))
                        .build();
                    edit_btn.add_css_class("flat");

                    {
                        let wp = win_parent.clone();
                        let s_data = scheme.clone();
                        let st_c = Rc::clone(st_rc);
                        let tm_c = Rc::clone(terminals_rc);
                        let cp_c = css_prov.clone();
                        let sl_c = st_lbl.clone();
                        let lb_e = listbox.clone();
                        let sr_e = Rc::clone(schemes_ref);
                        let ir_e = Rc::clone(icons_ref);
                        edit_btn.connect_clicked(move |_| {
                            let wp2 = wp.clone();
                            let lb2 = lb_e.clone();
                            let sr2 = Rc::clone(&sr_e);
                            let ir2 = Rc::clone(&ir_e);
                            let st2 = Rc::clone(&st_c);
                            let tm2 = Rc::clone(&tm_c);
                            let cp2 = cp_c.clone();
                            let sl2 = sl_c.clone();
                            show_scheme_editor_dialog(
                                &wp,
                                &s_data,
                                !s_data.is_custom,
                                Rc::clone(&st_c),
                                Rc::clone(&tm_c),
                                cp_c.clone(),
                                &sl_c,
                                Some(Rc::new(move || {
                                    build_list(&lb2, &sr2, &ir2, &wp2, &st2, &tm2, &cp2, &sl2);
                                })),
                            );
                        });
                    }
                    row.add_suffix(&edit_btn);

                    let check_icon = gtk4::Image::from_icon_name("emblem-ok-symbolic");
                    let is_selected = scheme.id == current_scheme_id
                        || (current_scheme_id == "onyx_dark" && scheme.id == "dark");
                    check_icon.set_visible(is_selected);
                    row.add_suffix(&check_icon);

                    icons_ref.borrow_mut().push((scheme.id.clone(), check_icon, row.clone()));
                    listbox.append(&row);
                }
            }
            let _ = slot;
            build_list(
                &listbox,
                &schemes_ref,
                &icons_ref,
                &win_parent,
                &st_rc,
                &terms_rc,
                &css_prov,
                &st_lbl,
            );
        })
    };

    rebuild_scheme_list();

    // Connect row activation to apply scheme in real-time
    {
        let icons_rc = Rc::clone(&check_icons_rc);
        let schemes_ref = Rc::clone(&schemes_rc);
        let st_rc = Rc::clone(&settings_rc);
        let terms_rc = Rc::clone(&terminals_rc);
        let css_prov = css_provider.clone();
        let st_lbl = status_lbl.clone();

        scheme_listbox.connect_row_activated(move |_, list_row| {
            let idx = list_row.index();
            let schemes = schemes_ref.borrow();
            if idx >= 0 && (idx as usize) < schemes.len() {
                let chosen = &schemes[idx as usize];

                for (id, chk, _) in icons_rc.borrow().iter() {
                    chk.set_visible(id == &chosen.id);
                }

                {
                    let mut st = st_rc.borrow_mut();
                    st.color_scheme = chosen.id.clone();
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
                    &[("name", &chosen.name)],
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
        let refresh_cb = Rc::clone(&rebuild_scheme_list);
        new_btn.connect_clicked(move |_| {
            let active_id = st_rc.borrow().color_scheme.clone();
            let base_scheme = resolve_palette(&active_id);
            show_scheme_editor_dialog(
                &win_parent,
                &base_scheme,
                true,
                Rc::clone(&st_rc),
                Rc::clone(&terms_rc),
                css_prov.clone(),
                &st_lbl,
                Some(Rc::clone(&refresh_cb)),
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
    let hl_cfg_rc = Rc::new(RefCell::new(
        onyxsh_terminal::highlighter::HighlightConfig::load_default(),
    ));

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
        .active(hl_cfg_rc.borrow().output_highlighting_enabled)
        .build();
    let row_ssh = adw::SwitchRow::builder()
        .title(&tr("Sessões SSH"))
        .subtitle(&tr("Aplicar destaque de saída a conexões SSH"))
        .active(hl_cfg_rc.borrow().output_highlighting_enabled)
        .build();
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        let rs = row_ssh.clone();
        row_local.connect_active_notify(move |r| {
            let active = r.is_active();
            cfg.borrow_mut().output_highlighting_enabled = active || rs.is_active();
            let _ = cfg.borrow().save_default();
        });
    }
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        let rl = row_local.clone();
        row_ssh.connect_active_notify(move |r| {
            let active = r.is_active();
            cfg.borrow_mut().output_highlighting_enabled = active || rl.is_active();
            let _ = cfg.borrow().save_default();
        });
    }

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
        .active(hl_cfg_rc.borrow().cat_colorization_enabled)
        .build();
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        row_cat_enable.connect_active_notify(move |r| {
            cfg.borrow_mut().cat_colorization_enabled = r.is_active();
            let _ = cfg.borrow().save_default();
        });
    }

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
        .active(hl_cfg_rc.borrow().shell_input_highlighting_enabled)
        .build();
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        row_shell_input_enable.connect_active_notify(move |r| {
            cfg.borrow_mut().shell_input_highlighting_enabled = r.is_active();
            let _ = cfg.borrow().save_default();
        });
    }

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
    let expander_ignored = adw::ExpanderRow::builder()
        .title(&tr("Ignored Commands"))
        .subtitle(&tr_fmt(
            "{count} comando(s) • Clique para expandir/recolher",
            &[("count", &hl_cfg_rc.borrow().ignored_commands.len().to_string())],
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

    let ignored_rows_rc: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::new(RefCell::new(Vec::new()));
    let rebuild_ignored_rc: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        let cfg_cb = Rc::clone(&cfg);
        let exp = expander_ignored.clone();
        let rows_store = Rc::clone(&ignored_rows_rc);
        let rebuild_weak = Rc::downgrade(&rebuild_ignored_rc);
        let rebuild: Rc<dyn Fn()> = Rc::new(move || {
            for r in rows_store.borrow_mut().drain(..) {
                exp.remove(&r);
            }
            let cmds = cfg_cb.borrow().ignored_commands.clone();
            exp.set_subtitle(&tr_fmt(
                "{count} comando(s) • Clique para expandir/recolher",
                &[("count", &cmds.len().to_string())],
            ));
            for cmd in cmds {
                let row_cmd = adw::ActionRow::builder().title(&cmd).build();
                let del_btn = gtk4::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .valign(gtk4::Align::Center)
                    .tooltip_text(&tr("Remove from ignored list"))
                    .build();
                del_btn.add_css_class("flat");
                let cfg_d = Rc::clone(&cfg_cb);
                let cmd_d = cmd.clone();
                let rw = rebuild_weak.clone();
                del_btn.connect_clicked(move |_| {
                    cfg_d
                        .borrow_mut()
                        .ignored_commands
                        .retain(|c| c != &cmd_d);
                    let _ = cfg_d.borrow().save_default();
                    if let Some(rc) = rw.upgrade() {
                        if let Some(f) = rc.borrow().as_ref() {
                            f();
                        }
                    }
                });
                row_cmd.add_suffix(&del_btn);
                exp.add_row(&row_cmd);
                rows_store.borrow_mut().push(row_cmd);
            }
        });
        *rebuild_ignored_rc.borrow_mut() = Some(Rc::clone(&rebuild));
        rebuild();

        let cfg_r = Rc::clone(&cfg);
        let reb_r = Rc::clone(&rebuild);
        btn_restore_ignored.connect_clicked(move |_| {
            cfg_r.borrow_mut().ignored_commands = onyxsh_terminal::highlighter::DEFAULT_IGNORED_COMMANDS
                .iter()
                .map(|s| (*s).to_string())
                .collect();
            let _ = cfg_r.borrow().save_default();
            reb_r();
        });

        let cfg_a = Rc::clone(&cfg);
        let reb_a = Rc::clone(&rebuild);
        let win_ref = win.clone();
        btn_add_ignored.connect_clicked(move |_| {
            let dlg = adw::Window::builder()
                .transient_for(&win_ref)
                .modal(true)
                .title(&tr("Add Ignored Command"))
                .default_width(360)
                .default_height(180)
                .build();
            let vb = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
            vb.append(&adw::HeaderBar::new());
            let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
            inner.set_margin_start(14);
            inner.set_margin_end(14);
            inner.set_margin_bottom(14);
            let entry = gtk4::Entry::builder().placeholder_text("htop").build();
            inner.append(&entry);
            let btn_save = gtk4::Button::with_label(&tr("Salvar"));
            btn_save.add_css_class("suggested-action");
            let d_c = dlg.clone();
            let c_c = Rc::clone(&cfg_a);
            let r_c = Rc::clone(&reb_a);
            btn_save.connect_clicked(move |_| {
                let txt = entry.text().trim().to_string();
                if !txt.is_empty() && !c_c.borrow().ignored_commands.contains(&txt) {
                    c_c.borrow_mut().ignored_commands.push(txt);
                    let _ = c_c.borrow().save_default();
                    r_c();
                }
                d_c.close();
            });
            inner.append(&btn_save);
            vb.append(&inner);
            dlg.set_content(Some(&vb));
            dlg.present();
        });
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

    let global_rule_rows: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::new(RefCell::new(Vec::new()));
    let rebuild_global_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let show_rule_editor = {
        let win_ref = win.clone();
        let cfg_ref = Rc::clone(&hl_cfg_rc);
        let reb_holder = Rc::clone(&rebuild_global_holder);
        move |edit_idx: Option<usize>| {
            let initial = edit_idx
                .and_then(|idx| cfg_ref.borrow().global_rules.get(idx).cloned())
                .unwrap_or_default();
            let dlg = adw::Window::builder()
                .transient_for(&win_ref)
                .modal(true)
                .title(&tr("Regras Globais de Destaque"))
                .default_width(440)
                .default_height(300)
                .build();
            let vb = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
            vb.append(&adw::HeaderBar::new());
            let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
            inner.set_margin_start(16);
            inner.set_margin_end(16);
            inner.set_margin_bottom(16);

            let name_e = gtk4::Entry::builder()
                .text(&initial.name)
                .placeholder_text(&tr("Nome"))
                .build();
            let pat_e = gtk4::Entry::builder()
                .text(&initial.pattern)
                .placeholder_text("Regex (ex: \\bERROR\\b)")
                .build();
            pat_e.add_css_class("monospace");
            let col_initial = initial
                .colors
                .first()
                .and_then(|c| c.as_deref())
                .unwrap_or("bold yellow");
            let col_e = gtk4::Entry::builder()
                .text(col_initial)
                .placeholder_text("bold red / #ff5555")
                .build();
            inner.append(&name_e);
            inner.append(&pat_e);
            inner.append(&col_e);

            let act_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            act_row.set_halign(gtk4::Align::End);
            let btn_cancel = gtk4::Button::with_label(&tr("Cancel"));
            let btn_save = gtk4::Button::with_label(&tr("Salvar"));
            btn_save.add_css_class("suggested-action");
            act_row.append(&btn_cancel);
            act_row.append(&btn_save);
            inner.append(&act_row);

            let d1 = dlg.clone();
            btn_cancel.connect_clicked(move |_| d1.close());
            let d2 = dlg.clone();
            let c_s = Rc::clone(&cfg_ref);
            let rh = Rc::clone(&reb_holder);
            btn_save.connect_clicked(move |_| {
                let pat = pat_e.text().trim().to_string();
                if pat.is_empty() || regex::Regex::new(&pat).is_err() {
                    return;
                }
                let nm = name_e.text().trim().to_string();
                let col_str = col_e.text().trim().to_string();
                let updated = onyxsh_terminal::highlighter::ProfileRuleConfig {
                    name: if nm.is_empty() { "Custom Rule".to_string() } else { nm },
                    pattern: pat,
                    colors: vec![Some(if col_str.is_empty() {
                        "bold yellow".to_string()
                    } else {
                        col_str
                    })],
                    enabled: initial.enabled,
                    description: initial.description.clone(),
                    comment: initial.comment.clone(),
                };
                {
                    let mut cfg = c_s.borrow_mut();
                    if let Some(i) = edit_idx {
                        if let Some(slot) = cfg.global_rules.get_mut(i) {
                            *slot = updated;
                        }
                    } else {
                        cfg.global_rules.push(updated);
                    }
                    let _ = cfg.save_default();
                }
                if let Some(ref f) = *rh.borrow() {
                    f();
                }
                d2.close();
            });

            vb.append(&inner);
            dlg.set_content(Some(&vb));
            dlg.present();
        }
    };
    let show_rule_editor = Rc::new(show_rule_editor);

    {
        let cfg = Rc::clone(&hl_cfg_rc);
        let cfg_cb = Rc::clone(&cfg);
        let grp = group_global_rules.clone();
        let rows_store = Rc::clone(&global_rule_rows);
        let editor = Rc::clone(&show_rule_editor);
        let reb_weak = Rc::downgrade(&rebuild_global_holder);
        let rebuild: Rc<dyn Fn()> = Rc::new(move || {
            for r in rows_store.borrow_mut().drain(..) {
                grp.remove(&r);
            }
            let rules = cfg_cb.borrow().global_rules.clone();
            for (idx, rule) in rules.iter().enumerate() {
                let escaped_name = glib::markup_escape_text(&tr(&rule.name));
                let escaped_pattern = glib::markup_escape_text(&rule.pattern);
                let rule_row = adw::ActionRow::builder()
                    .title(escaped_name.as_str())
                    .subtitle(escaped_pattern.as_str())
                    .build();

                let badge = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
                badge.set_size_request(24, 24);
                badge.set_valign(gtk4::Align::Center);
                badge.set_margin_end(8);
                badge.add_css_class("card");
                let h_col = sgr_name_to_preview_hex(rule.colors.first().and_then(|c| c.as_deref()));
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
                    .active(rule.enabled)
                    .valign(gtk4::Align::Center)
                    .build();
                let cfg_sw = Rc::clone(&cfg_cb);
                sw.connect_state_set(move |_, active| {
                    if let Some(r) = cfg_sw.borrow_mut().global_rules.get_mut(idx) {
                        r.enabled = active;
                    }
                    let _ = cfg_sw.borrow().save_default();
                    glib::Propagation::Proceed
                });
                rule_row.add_suffix(&sw);

                let edit_b = gtk4::Button::builder()
                    .icon_name("document-edit-symbolic")
                    .valign(gtk4::Align::Center)
                    .build();
                edit_b.add_css_class("flat");
                let ed_c = Rc::clone(&editor);
                edit_b.connect_clicked(move |_| ed_c(Some(idx)));
                rule_row.add_suffix(&edit_b);

                let del_b = gtk4::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .valign(gtk4::Align::Center)
                    .build();
                del_b.add_css_class("flat");
                let cfg_del = Rc::clone(&cfg_cb);
                let rw = reb_weak.clone();
                del_b.connect_clicked(move |_| {
                    if idx < cfg_del.borrow().global_rules.len() {
                        cfg_del.borrow_mut().global_rules.remove(idx);
                        let _ = cfg_del.borrow().save_default();
                        if let Some(rc) = rw.upgrade() {
                            if let Some(f) = rc.borrow().as_ref() {
                                f();
                            }
                        }
                    }
                });
                rule_row.add_suffix(&del_b);

                grp.add(&rule_row);
                rows_store.borrow_mut().push(rule_row);
            }
        });
        *rebuild_global_holder.borrow_mut() = Some(Rc::clone(&rebuild));
        rebuild();

        let ed_add = Rc::clone(&show_rule_editor);
        btn_add_rule.connect_clicked(move |_| ed_add(None));

        let cfg_reset = Rc::clone(&cfg);
        let reb_reset = Rc::clone(&rebuild);
        btn_reset_rules.connect_clicked(move |_| {
            cfg_reset.borrow_mut().global_rules =
                onyxsh_terminal::highlighter::default_global_rules_from_bundle();
            let _ = cfg_reset.borrow().save_default();
            reb_reset();
        });
    }

    page_output.add(&group_global_rules);
    win.add(&page_output);

    // ──────────────────────────────────────────────────────────────────────────
    // 3. ABA 3: Específico do Comando (Per-Command Context Highlighting — 51 Perfis)
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
        .active(hl_cfg_rc.borrow().command_highlighting_enabled)
        .build();
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        row_cmd_aware.connect_active_notify(move |r| {
            cfg.borrow_mut().command_highlighting_enabled = r.is_active();
            let _ = cfg.borrow().save_default();
        });
    }
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

    let cmd_rows_store: Rc<RefCell<Vec<adw::ActionRow>>> = Rc::new(RefCell::new(Vec::new()));
    let cmd_switches_store: Rc<RefCell<Vec<gtk4::Switch>>> = Rc::new(RefCell::new(Vec::new()));

    let rebuild_cmd_profiles: Rc<dyn Fn()> = {
        let cfg = Rc::clone(&hl_cfg_rc);
        let grp = group_available_cmds.clone();
        let rows_st = Rc::clone(&cmd_rows_store);
        let sw_st = Rc::clone(&cmd_switches_store);
        let win_ref = win.clone();

        Rc::new(move || {
            for r in rows_st.borrow_mut().drain(..) {
                grp.remove(&r);
            }
            sw_st.borrow_mut().clear();

            let profiles: Vec<(String, onyxsh_terminal::highlighter::HighlightProfile)> = cfg
                .borrow()
                .profiles
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            for (prof_key, prof) in profiles {
                let escaped_cmd = glib::markup_escape_text(&prof.name);
                let triggers_txt = if prof.triggers.is_empty() {
                    prof.name.clone()
                } else {
                    prof.triggers.join(", ")
                };
                let raw_desc = if prof.description.trim().is_empty() {
                    triggers_txt
                } else {
                    format!("{} ({triggers_txt})", prof.description)
                };
                let escaped_desc = glib::markup_escape_text(&raw_desc);
                let row_ctx = adw::ActionRow::builder()
                    .title(escaped_cmd.as_str())
                    .subtitle(escaped_desc.as_str())
                    .build();

                let badge = gtk4::Label::new(Some(&tr_fmt(
                    "{count} regras",
                    &[("count", &prof.rules.len().to_string())],
                )));
                badge.add_css_class("dim-label");
                badge.add_css_class("caption");
                badge.set_valign(gtk4::Align::Center);
                badge.set_margin_end(8);
                row_ctx.add_suffix(&badge);

                let sw = gtk4::Switch::builder()
                    .active(prof.enabled)
                    .valign(gtk4::Align::Center)
                    .build();
                {
                    let cfg_s = Rc::clone(&cfg);
                    let pk = prof_key.clone();
                    sw.connect_state_set(move |_, active| {
                        if let Some(p) = cfg_s.borrow_mut().profiles.get_mut(&pk) {
                            p.enabled = active;
                        }
                        let _ = cfg_s.borrow().save_default();
                        glib::Propagation::Proceed
                    });
                }
                sw_st.borrow_mut().push(sw.clone());
                row_ctx.add_suffix(&sw);

                let cfg_btn = gtk4::Button::builder()
                    .icon_name("go-next-symbolic")
                    .valign(gtk4::Align::Center)
                    .tooltip_text(&tr("Configurar regras deste comando"))
                    .build();
                cfg_btn.add_css_class("flat");
                {
                    let cfg_p = Rc::clone(&cfg);
                    let pk = prof_key.clone();
                    let w_ref = win_ref.clone();
                    cfg_btn.connect_clicked(move |_| {
                        let maybe_prof = cfg_p.borrow().profiles.get(&pk).cloned();
                        let Some(p_val) = maybe_prof else {
                            return;
                        };
                        let dlg = adw::Window::builder()
                            .transient_for(&w_ref)
                            .modal(true)
                            .title(&p_val.name)
                            .default_width(540)
                            .default_height(460)
                            .build();
                        let vb = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
                        vb.append(&adw::HeaderBar::new());
                        let scroll = gtk4::ScrolledWindow::builder().vexpand(true).build();
                        let list = gtk4::ListBox::new();
                        list.add_css_class("boxed-list");
                        list.set_margin_start(14);
                        list.set_margin_end(14);
                        list.set_margin_top(12);
                        list.set_margin_bottom(12);

                        for (ridx, rule) in p_val.rules.iter().enumerate() {
                            let r_row = adw::ActionRow::builder()
                                .title(glib::markup_escape_text(&rule.name).as_str())
                                .subtitle(glib::markup_escape_text(&rule.pattern).as_str())
                                .build();
                            let r_sw = gtk4::Switch::builder()
                                .active(rule.enabled)
                                .valign(gtk4::Align::Center)
                                .build();
                            let c_inner = Rc::clone(&cfg_p);
                            let pk_inner = pk.clone();
                            r_sw.connect_state_set(move |_, active| {
                                if let Some(prof_mut) = c_inner.borrow_mut().profiles.get_mut(&pk_inner) {
                                    if let Some(r_mut) = prof_mut.rules.get_mut(ridx) {
                                        r_mut.enabled = active;
                                    }
                                }
                                let _ = c_inner.borrow().save_default();
                                glib::Propagation::Proceed
                            });
                            r_row.add_suffix(&r_sw);
                            list.append(&r_row);
                        }

                        scroll.set_child(Some(&list));
                        vb.append(&scroll);
                        dlg.set_content(Some(&vb));
                        dlg.present();
                    });
                }
                row_ctx.add_suffix(&cfg_btn);

                grp.add(&row_ctx);
                rows_st.borrow_mut().push(row_ctx);
            }
        })
    };
    rebuild_cmd_profiles();

    {
        let cfg = Rc::clone(&hl_cfg_rc);
        let reb = Rc::clone(&rebuild_cmd_profiles);
        btn_enable_all.connect_clicked(move |_| {
            for prof in cfg.borrow_mut().profiles.values_mut() {
                prof.enabled = true;
            }
            let _ = cfg.borrow().save_default();
            reb();
        });
    }
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        let reb = Rc::clone(&rebuild_cmd_profiles);
        btn_disable_all.connect_clicked(move |_| {
            for prof in cfg.borrow_mut().profiles.values_mut() {
                prof.enabled = false;
            }
            let _ = cfg.borrow().save_default();
            reb();
        });
    }
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        let reb = Rc::clone(&rebuild_cmd_profiles);
        btn_reset_cmds.connect_clicked(move |_| {
            let defaults = onyxsh_terminal::highlighter::HighlightConfig::default();
            cfg.borrow_mut().profiles = defaults.profiles;
            let _ = cfg.borrow().save_default();
            reb();
        });
    }
    {
        let cfg = Rc::clone(&hl_cfg_rc);
        let reb = Rc::clone(&rebuild_cmd_profiles);
        let w_ref = win.clone();
        btn_add_cmd.connect_clicked(move |_| {
            let dlg = adw::Window::builder()
                .transient_for(&w_ref)
                .modal(true)
                .title(&tr("Add Command"))
                .default_width(380)
                .default_height(240)
                .build();
            let vb = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
            vb.append(&adw::HeaderBar::new());
            let inner = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
            inner.set_margin_start(14);
            inner.set_margin_end(14);
            inner.set_margin_bottom(14);
            let cmd_e = gtk4::Entry::builder().placeholder_text("kubectl / helm").build();
            let pat_e = gtk4::Entry::builder().placeholder_text("Regex (ex: Running|Pending)").build();
            pat_e.add_css_class("monospace");
            let col_e = gtk4::Entry::builder().text("bold cyan").build();
            inner.append(&cmd_e);
            inner.append(&pat_e);
            inner.append(&col_e);

            let btn_save = gtk4::Button::with_label(&tr("Salvar"));
            btn_save.add_css_class("suggested-action");
            let d_c = dlg.clone();
            let c_c = Rc::clone(&cfg);
            let r_c = Rc::clone(&reb);
            btn_save.connect_clicked(move |_| {
                let name = cmd_e.text().trim().to_string();
                let pat = pat_e.text().trim().to_string();
                if name.is_empty() || pat.is_empty() || regex::Regex::new(&pat).is_err() {
                    return;
                }
                let col = col_e.text().trim().to_string();
                let prof = onyxsh_terminal::highlighter::HighlightProfile {
                    name: name.clone(),
                    description: String::new(),
                    triggers: vec![name.clone()],
                    enabled: true,
                    use_global_rules: true,
                    rules: vec![onyxsh_terminal::highlighter::ProfileRuleConfig {
                        name: name.clone(),
                        pattern: pat,
                        colors: vec![Some(if col.is_empty() { "bold cyan".to_string() } else { col })],
                        enabled: true,
                        description: String::new(),
                        comment: String::new(),
                    }],
                };
                c_c.borrow_mut().profiles.insert(name, prof);
                let _ = c_c.borrow().save_default();
                r_c();
                d_c.close();
            });
            inner.append(&btn_save);
            vb.append(&inner);
            dlg.set_content(Some(&vb));
            dlg.present();
        });
    }

    page_cmd.add(&group_available_cmds);
    win.add(&page_cmd);

    win.present();
}

/// Sub-dialog for creating or editing a color scheme (`_SchemeEditorDialog` in `src/onyxsh/ui/color_scheme_dialog.py`).
pub fn show_scheme_editor_dialog(
    parent: &adw::PreferencesWindow,
    base_scheme: &OwnedTerminalPalette,
    is_new: bool,
    settings_rc: Rc<RefCell<AppSettings>>,
    terminals_rc: Rc<RefCell<Vec<vte4::Terminal>>>,
    css_provider: gtk4::CssProvider,
    status_lbl: &gtk4::Label,
    on_saved: Option<Rc<dyn Fn()>>,
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
        tr_fmt("Cópia de {name}", &[("name", &base_scheme.name)])
    } else {
        base_scheme.name.clone()
    };

    let name_entry = adw::EntryRow::builder()
        .title(&tr("Scheme Name"))
        .text(&name_val)
        .build();
    group_general.add(&name_entry);

    let fg_entry = adw::EntryRow::builder()
        .title(&tr("Texto de Primeiro Plano (Foreground)"))
        .text(&base_scheme.foreground)
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
        .text(&base_scheme.background)
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
        .text(&base_scheme.cursor)
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
            base_scheme.ansi[i].as_str()
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
        let fg_e = fg_entry.clone();
        let bg_e = bg_entry.clone();
        let cur_e = cursor_entry.clone();
        let pal_entries = palette_entries.clone();
        let existing_id = base_scheme.id.clone();
        let existing_is_custom = base_scheme.is_custom;

        save_btn.connect_clicked(move |_| {
            let new_name = n_entry.text().to_string();
            let trimmed_name = if new_name.trim().is_empty() {
                "Custom Scheme".to_string()
            } else {
                new_name.trim().to_string()
            };
            let target_id = if !is_new && existing_is_custom {
                existing_id.clone()
            } else {
                slugify_scheme_id(&trimmed_name)
            };

            let custom_palette = OwnedTerminalPalette {
                id: target_id.clone(),
                name: trimmed_name.clone(),
                foreground: fg_e.text().trim().to_string(),
                background: bg_e.text().trim().to_string(),
                cursor: cur_e.text().trim().to_string(),
                ansi: std::array::from_fn(|i| {
                    pal_entries
                        .get(i)
                        .map(|e| e.text().trim().to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "#ffffff".to_string())
                }),
                is_custom: true,
            };

            let _ = save_custom_scheme(&custom_palette);

            {
                let mut st = st_rc.borrow_mut();
                st.color_scheme = target_id.clone();
                let opacity_pct = 100u8.saturating_sub(st.transparency.min(100));
                let css = generate_theme_css(&st.color_scheme, opacity_pct);
                css_prov.load_from_string(&css);

                for term in terms_rc.borrow().iter() {
                    apply_all_settings_to_terminal(term, &st);
                }
                let cfg_path = onyxsh_core::platform::config_dir().join("settings.json");
                let _ = st.save_to_path(&cfg_path);
            }

            if let Some(ref cb) = on_saved {
                cb();
            }

            st_lbl.set_text(&tr_fmt(
                "Esquema de cores salvo: {name}",
                &[("name", &trimmed_name)],
            ));
            d_c.close();
        });
    }

    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme_engine::all_builtin_schemes;

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
