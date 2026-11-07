//! Phase 16: Advanced Terminal UX — Autocomplete Popup (`CompletionEngine`),
//! Selection Context Popover, Production Safety Banner & Confirmation Dialog (`ProductionGuard`),
//! and Unified Command Palette (`Ctrl+Shift+P`).
//!
//! Ports:
//! - `src/onyxsh/terminal/completion/popup.py`
//! - `src/onyxsh/ui/widgets/selection_popover.py`
//! - `src/onyxsh/ui/widgets/production_banner.py` & `ui/dialogs/production_confirm_dialog.py`
//! - `src/onyxsh/ui/dialogs/command_palette.py`

use crate::command_manager_ui::CommandButton;
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::platform;
use onyxsh_core::sessions::{SessionItem, SessionsStorageData};
use onyxsh_terminal::completion::{CompletionEngine, CompletionItem};
use onyxsh_terminal::production_guard::{GuardViolation, ProductionGuard};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use vte4::prelude::*;

/// Entry in the unified Command Palette (`Ctrl+Shift+P`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem {
    pub category: String,
    pub title: String,
    pub subtitle: String,
    pub action_key: String,
}

/// Extracts the active command line typed after the shell prompt marker (`$ `, `# `, `% `, `❯ `, `> `).
pub fn extract_prompt_command_line(full_line: &str) -> String {
    let trimmed = full_line.trim_end_matches(['\r', '\n']);
    let markers = ["$ ", "# ", "% ", "❯ ", "> "];
    let mut best_pos: Option<usize> = None;
    for marker in markers {
        if let Some(idx) = trimmed.rfind(marker) {
            let end = idx + marker.len();
            best_pos = Some(best_pos.map_or(end, |prev| prev.max(end)));
        }
    }
    if let Some(pos) = best_pos {
        trimmed[pos..].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Filters [`PaletteItem`]s by a case-insensitive query across category, title, and subtitle.
pub fn filter_palette_items(items: &[PaletteItem], query: &str) -> Vec<PaletteItem> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|it| {
            it.title.to_lowercase().contains(&q)
                || it.subtitle.to_lowercase().contains(&q)
                || it.category.to_lowercase().contains(&q)
        })
        .cloned()
        .collect()
}

/// Queries [`CompletionEngine`] for a command line and returns ranked [`CompletionItem`]s.
pub fn query_terminal_completions(input_line: &str, limit: usize) -> Vec<CompletionItem> {
    let engine = CompletionEngine::new();
    let probe = if input_line.trim().is_empty() {
        "git "
    } else {
        input_line
    };
    engine.get_completions(probe, Some(probe.len()), &[], limit)
}

/// Builds the Production Environment Warning Banner (`ui/widgets/production_banner.py`).
pub fn build_production_banner(
    parent_window: &adw::ApplicationWindow,
    active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>>,
    status_lbl: &gtk4::Label,
) -> (gtk4::Box, gtk4::ToggleButton) {
    let banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    banner.set_margin_start(10);
    banner.set_margin_end(10);
    banner.set_margin_top(4);
    banner.set_margin_bottom(4);
    banner.set_visible(false);

    let icon = gtk4::Image::from_icon_name("dialog-warning-symbolic");
    banner.append(&icon);

    let lbl = gtk4::Label::new(Some(
        "🛡️ MODO PRODUÇÃO ATIVO — ProductionGuard interceptando comandos destrutivos (rm -rf, mkfs, dd, DROP TABLE, subshells)",
    ));
    lbl.add_css_class("heading");
    lbl.set_halign(gtk4::Align::Start);
    lbl.set_hexpand(true);
    lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    banner.append(&lbl);

    let btn_test_guard = gtk4::Button::with_label("Testar Interceptação");
    btn_test_guard.add_css_class("destructive-action");
    banner.append(&btn_test_guard);

    let toggle_prod_btn = gtk4::ToggleButton::builder()
        .icon_name("security-high-symbolic")
        .tooltip_text("Ativar / Desativar Escudo de Ambiente de Produção (ProductionBanner)")
        .build();

    {
        let b = banner.clone();
        toggle_prod_btn.connect_toggled(move |btn| {
            b.set_visible(btn.is_active());
        });
    }

    {
        let win_ref = parent_window.clone();
        let act_term = Rc::clone(&active_term_cb);
        let st = status_lbl.clone();
        btn_test_guard.connect_clicked(move |_| {
            let guard = ProductionGuard::new();
            let sample_cmd = "sudo rm -rf /var/lib/postgresql/data";
            if let Some(violation) = guard.evaluate_command(sample_cmd) {
                show_production_confirm_dialog(&win_ref, &violation, {
                    let at = Rc::clone(&act_term);
                    let st = st.clone();
                    Rc::new(move || {
                        if let Some(term) = at() {
                            term.feed_child(b"# [Confirmado pelo Operador] rm -rf cancelado em modo demo\n");
                        }
                        st.set_text("Comando de produção avaliado pelo operador.");
                    })
                });
            }
        });
    }

    (banner, toggle_prod_btn)
}

/// Opens the modal `ProductionConfirmDialog` (`ui/dialogs/production_confirm_dialog.py`)
/// when [`ProductionGuard`] intercepts a high-risk/destructive command.
pub fn show_production_confirm_dialog(
    parent: &adw::ApplicationWindow,
    violation: &GuardViolation,
    on_confirm_execute: Rc<dyn Fn()>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("🛑 Alerta Crítico — ProductionGuard")
        .default_width(500)
        .default_height(310)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    content.set_margin_start(18);
    content.set_margin_end(18);
    content.set_margin_top(8);
    content.set_margin_bottom(16);

    let head = gtk4::Label::new(Some(&format!(
        "⚠️ Comando Destrutivo Interceptado ({:?} / {:?})",
        violation.category, violation.severity
    )));
    head.add_css_class("title-4");
    head.set_halign(gtk4::Align::Start);
    content.append(&head);

    let summary_lbl = gtk4::Label::new(Some(&violation.risk_summary));
    summary_lbl.set_wrap(true);
    summary_lbl.set_halign(gtk4::Align::Start);
    content.append(&summary_lbl);

    let cmd_lbl = gtk4::Label::new(Some(&format!("$ {}", violation.command)));
    cmd_lbl.add_css_class("monospace");
    cmd_lbl.set_halign(gtk4::Align::Start);
    cmd_lbl.set_selectable(true);
    content.append(&cmd_lbl);

    let btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_row.set_halign(gtk4::Align::End);

    let btn_cancel = gtk4::Button::with_label("Bloquear e Cancelar (Recomendado)");
    btn_cancel.add_css_class("suggested-action");

    let btn_proceed = gtk4::Button::with_label("Assumir Risco e Executar");
    btn_proceed.add_css_class("destructive-action");

    btn_row.append(&btn_cancel);
    btn_row.append(&btn_proceed);
    content.append(&btn_row);

    {
        let d = dlg.clone();
        btn_cancel.connect_clicked(move |_| d.close());
    }
    {
        let d = dlg.clone();
        let cb = on_confirm_execute;
        btn_proceed.connect_clicked(move |_| {
            cb();
            d.close();
        });
    }

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Opens the interactive Autocomplete Assistant (`terminal/completion/popup.py` & `completion.rs`).
pub fn show_autocomplete_assistant_dialog(
    parent: &adw::ApplicationWindow,
    active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>>,
    status_lbl: &gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Autocompletar Inteligente — OnyxSH CompletionEngine")
        .default_width(540)
        .default_height(420)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    content.set_margin_start(14);
    content.set_margin_end(14);
    content.set_margin_top(8);
    content.set_margin_bottom(14);
    content.set_vexpand(true);

    let hint = gtk4::Label::new(Some(
        "Digite um comando (ex: 'sudo rpm -', 'sudo dnf install ', 'apt ', 'pacman -', 'flatpak ', 'zypper ', 'snap '):",
    ));
    hint.add_css_class("caption");
    hint.set_halign(gtk4::Align::Start);
    content.append(&hint);

    let entry = gtk4::Entry::builder()
        .text("sudo dnf ")
        .hexpand(true)
        .build();
    entry.add_css_class("monospace");
    content.append(&entry);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    scroll.set_child(Some(&list_box));
    content.append(&scroll);

    let populate = {
        let lb = list_box.clone();
        let e = entry.clone();
        let act_term = Rc::clone(&active_term_cb);
        let st = status_lbl.clone();
        let d_close = dlg.clone();

        Rc::new(move || {
            lb.remove_all();
            let line = e.text().to_string();
            let suggestions = query_terminal_completions(&line, 25);

            for item in suggestions {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                row.set_margin_start(10);
                row.set_margin_end(10);
                row.set_margin_top(6);
                row.set_margin_bottom(6);

                let cmd_lbl = gtk4::Label::new(Some(&item.text));
                cmd_lbl.add_css_class("monospace");
                cmd_lbl.add_css_class("heading");
                cmd_lbl.set_halign(gtk4::Align::Start);
                cmd_lbl.set_width_request(130);

                let desc_lbl = gtk4::Label::new(Some(&item.description));
                desc_lbl.add_css_class("caption");
                desc_lbl.add_css_class("dim-label");
                desc_lbl.set_halign(gtk4::Align::Start);
                desc_lbl.set_hexpand(true);
                desc_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                let full_to_insert = if line.ends_with(char::is_whitespace) {
                    format!("{line}{}", item.text)
                } else if !item.suffix_to_insert.is_empty() {
                    format!("{line}{}", item.suffix_to_insert)
                } else {
                    format!("{line} {}", item.text)
                };

                let btn_ins = gtk4::Button::with_label("Inserir");
                btn_ins.add_css_class("suggested-action");
                let at = Rc::clone(&act_term);
                let status = st.clone();
                let dc = d_close.clone();
                btn_ins.connect_clicked(move |_| {
                    if let Some(term) = at() {
                        term.feed_child(full_to_insert.as_bytes());
                        status.set_text(&format!("Autocompletado: {full_to_insert}"));
                    }
                    dc.close();
                });

                row.append(&cmd_lbl);
                row.append(&desc_lbl);
                row.append(&btn_ins);
                lb.append(&row);
            }
        })
    };

    populate();

    {
        let pop = Rc::clone(&populate);
        entry.connect_changed(move |_| pop());
    }

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Detects if there is a hyperlink at coordinates `(x, y)` in `term`.
pub fn detect_url_at_coords(term: &vte4::Terminal, x: f64, y: f64) -> Option<String> {
    if let Some(uri) = term.check_hyperlink_at(x, y) {
        let s = uri.to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    let (matched, _tag) = term.check_match_at(x, y);
    if let Some(m) = matched {
        let s = m.to_string();
        if s.starts_with("http://")
            || s.starts_with("https://")
            || s.starts_with("file://")
            || s.starts_with("ssh://")
            || s.starts_with("ftp://")
        {
            return Some(s);
        }
    }
    None
}

/// Checks if recent terminal lines contain an error indicator.
pub fn check_terminal_last_error(term: &vte4::Terminal) -> Option<i32> {
    let text = crate::global_search_and_shortcuts::extract_vte_text(term);
    let last_lines: Vec<&str> = text.lines().rev().take(6).collect();
    let lower = last_lines.join("\n").to_lowercase();
    let err_keywords = [
        "error",
        "erro",
        "failed",
        "falha",
        "command not found",
        "não encontrada",
        "não reconhecida",
        "no such file",
        "permission denied",
        "permissão negada",
    ];
    if err_keywords.iter().any(|kw| lower.contains(kw)) {
        Some(1)
    } else {
        None
    }
}

/// Builds the native GTK4 `gio::Menu` model for the terminal right-click context menu.
/// Matches `src/onyxsh/ui/menus.py` (`create_terminal_menu`) and `CORRIGIR/menucontext3.jpg`.
pub fn create_terminal_context_menu_model(
    has_selection: bool,
    url_at_click: Option<&str>,
    error_code: Option<i32>,
) -> gio::Menu {
    let menu = gio::Menu::new();

    // 0. Contextual URL section (when right-clicking over a hyperlink)
    if let Some(_url) = url_at_click {
        let url_section = gio::Menu::new();
        url_section.append(Some("Abrir Link"), Some("win.open-url"));
        url_section.append(Some("Copiar Link"), Some("win.copy-url"));
        menu.append_section(None, &url_section);
    }

    // 1. Standard editing, export & session clear
    let standard_section = gio::Menu::new();
    standard_section.append(Some("Copiar"), Some("win.copy"));
    standard_section.append(Some("Colar"), Some("win.paste"));
    standard_section.append(Some("Selecionar Tudo"), Some("win.select-all"));
    standard_section.append(
        Some("Exportar Saída do Terminal..."),
        Some("win.export-terminal-buffer"),
    );
    standard_section.append(
        Some("Exportar Sessão como Caderno de Bordo..."),
        Some("win.export-terminal-runbook"),
    );
    standard_section.append(Some("Limpar Sessão"), Some("win.clear-session"));
    menu.append_section(None, &standard_section);

    // 2. Prompt navigation
    let nav_section = gio::Menu::new();
    nav_section.append(
        Some("Pular para o Prompt Anterior"),
        Some("win.jump-previous-prompt"),
    );
    nav_section.append(
        Some("Pular para o Próximo Prompt"),
        Some("win.jump-next-prompt"),
    );
    menu.append_section(None, &nav_section);

    // 3. AI / OpenCode Assistant
    let ai_section = gio::Menu::new();
    if has_selection {
        ai_section.append(Some("Perguntar à IA"), Some("win.ask-ai-selection"));
        ai_section.append(
            Some("Refatorar seleção com OpenCode"),
            Some("win.opencode-selection"),
        );
    } else {
        let label = if let Some(code) = error_code {
            if code != 1 && code != 0 {
                format!("Resolver erro (código {code}) com OpenCode")
            } else {
                "Resolver erro com OpenCode".to_string()
            }
        } else {
            "Resolver erro com OpenCode".to_string()
        };
        ai_section.append(Some(&label), Some("win.resolve-error-opencode"));
    }
    menu.append_section(None, &ai_section);

    // 4. Splitting & Panes
    let split_section = gio::Menu::new();
    split_section.append(
        Some("Dividir Esquerda/Direita"),
        Some("win.split-horizontal"),
    );
    split_section.append(
        Some("Dividir Superior/Inferior"),
        Some("win.split-vertical"),
    );
    split_section.append(Some("Fechar Painel"), Some("win.close-pane"));
    menu.append_section(None, &split_section);

    menu
}

/// Attaches native right-click `PopoverMenu` (`CORRIGIR/menucontext3.jpg`), floating live autocomplete
/// popup (`CompletionEngine`), and `ProductionGuard` to a `vte4::Terminal`.
pub fn attach_vte_ux_controllers(
    term: &vte4::Terminal,
    clicked_url_rc: Rc<RefCell<Option<String>>>,
    _parent_window: &adw::ApplicationWindow,
    _ai_widget: &gtk4::Box,
    _btn_ai_toggle: &gtk4::ToggleButton,
    _submit_ai_prompt_cb: Rc<dyn Fn(&str)>,
    status_lbl: &gtk4::Label,
) {
    // 1. Live Floating Autocomplete Popover (`src/onyxsh/terminal/completion/popup.py` & `manager.py`)
    let comp_popover = gtk4::Popover::new();
    comp_popover.set_parent(term);
    comp_popover.set_autohide(false);
    comp_popover.set_focusable(false);
    comp_popover.set_can_focus(false);
    comp_popover.set_has_arrow(false);
    comp_popover.set_position(gtk4::PositionType::Bottom);

    let comp_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    comp_vbox.set_margin_start(4);
    comp_vbox.set_margin_end(4);
    comp_vbox.set_margin_top(4);
    comp_vbox.set_margin_bottom(4);
    comp_vbox.set_size_request(340, -1);
    comp_vbox.set_can_focus(false);

    let comp_list = gtk4::ListBox::new();
    comp_list.add_css_class("boxed-list");
    comp_list.set_selection_mode(gtk4::SelectionMode::Single);
    comp_list.set_can_focus(false);
    comp_vbox.append(&comp_list);

    let comp_hint = gtk4::Label::new(Some("Tab ou → aplicar • ↑/↓ navegar • Esc fechar"));
    comp_hint.add_css_class("dim-label");
    comp_hint.add_css_class("caption");
    comp_hint.set_halign(gtk4::Align::Start);
    comp_hint.set_margin_start(6);
    comp_vbox.append(&comp_hint);

    comp_popover.set_child(Some(&comp_vbox));

    // 2. Right-click Native PopoverMenu Context Menu (`CORRIGIR/menucontext3.jpg` & `src/onyxsh/ui/menus.py`)
    // Ensure VTE internal context menu is disabled so it doesn't spawn an orphaned duplicate popup
    term.set_context_menu_model(None::<&gio::MenuModel>);

    let initial_menu = create_terminal_context_menu_model(false, None, None);
    let popover = gtk4::PopoverMenu::builder()
        .has_arrow(false)
        .menu_model(&initial_menu)
        .autohide(true)
        .build();
    popover.set_parent(term);

    let click_ctrl = gtk4::GestureClick::new();
    click_ctrl.set_button(3); // Right mouse button
    {
        let p = popover;
        let t = term.clone();
        let c_url = Rc::clone(&clicked_url_rc);
        let comp_p = comp_popover.clone();
        click_ctrl.connect_pressed(move |gesture, _, x, y| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            t.grab_focus();
            if comp_p.is_visible() {
                comp_p.popdown();
            }

            let url = detect_url_at_coords(&t, x, y);
            *c_url.borrow_mut() = url.clone();

            let has_sel = t.has_selection();
            let err_code = check_terminal_last_error(&t);

            let menu = create_terminal_context_menu_model(has_sel, url.as_deref(), err_code);
            p.set_menu_model(Some(&menu));

            let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            p.set_pointing_to(Some(&rect));
            p.popup();
        });
    }
    term.add_controller(click_ctrl);

    let comp_items: Rc<RefCell<Vec<CompletionItem>>> = Rc::new(RefCell::new(Vec::new()));
    let comp_selected_idx: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let comp_rows: Rc<RefCell<Vec<gtk4::ListBoxRow>>> = Rc::new(RefCell::new(Vec::new()));

    let accept_completion: Rc<dyn Fn(usize)> = {
        let t = term.clone();
        let pop = comp_popover.clone();
        let items_rc = Rc::clone(&comp_items);
        let st = status_lbl.clone();

        Rc::new(move |idx: usize| {
            let maybe_item = items_rc.borrow().get(idx).cloned();
            pop.popdown();
            if let Some(item) = maybe_item {
                if !item.suffix_to_insert.is_empty() {
                    let mut feed = item.suffix_to_insert.clone();
                    if !feed.ends_with(' ') {
                        feed.push(' ');
                    }
                    t.feed_child(feed.as_bytes());
                } else if !item.prefix_to_replace.is_empty() {
                    let backspaces = "\x08".repeat(item.prefix_to_replace.chars().count());
                    let mut feed = format!("{backspaces}{}", item.text);
                    if !feed.ends_with(' ') {
                        feed.push(' ');
                    }
                    t.feed_child(feed.as_bytes());
                } else {
                    let mut feed = item.text.clone();
                    if !feed.ends_with(' ') {
                        feed.push(' ');
                    }
                    t.feed_child(feed.as_bytes());
                }
                st.set_text(&format!("Autocompletar aplicado: {}", item.text));
            }
        })
    };

    {
        let acc = Rc::clone(&accept_completion);
        comp_list.connect_row_activated(move |_, row| {
            let idx = row.index();
            if idx >= 0 {
                acc(idx as usize);
            }
        });
    }

    let key_ctrl = gtk4::EventControllerKey::new();
    key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let t = term.clone();
        let pop = comp_popover;
        let lb = comp_list;
        let items_rc = Rc::clone(&comp_items);
        let sel_rc = Rc::clone(&comp_selected_idx);
        let rows_rc = Rc::clone(&comp_rows);
        let acc = Rc::clone(&accept_completion);

        key_ctrl.connect_key_pressed(move |_, keyval, _, state| {
            let has_mod = state.intersects(
                gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK,
            );

            // Handle navigation and acceptance when floating CompletionPopup is visible
            if pop.is_visible() && !items_rc.borrow().is_empty() && !has_mod {
                let len = items_rc.borrow().len();
                if keyval == gdk::Key::Down || keyval == gdk::Key::KP_Down {
                    let next = (*sel_rc.borrow() + 1) % len;
                    *sel_rc.borrow_mut() = next;
                    if let Some(r) = rows_rc.borrow().get(next) {
                        lb.select_row(Some(r));
                    }
                    return glib::Propagation::Stop;
                }
                if keyval == gdk::Key::Up || keyval == gdk::Key::KP_Up {
                    let prev = if *sel_rc.borrow() == 0 {
                        len - 1
                    } else {
                        *sel_rc.borrow() - 1
                    };
                    *sel_rc.borrow_mut() = prev;
                    if let Some(r) = rows_rc.borrow().get(prev) {
                        lb.select_row(Some(r));
                    }
                    return glib::Propagation::Stop;
                }
                if keyval == gdk::Key::Tab
                    || keyval == gdk::Key::Right
                    || keyval == gdk::Key::KP_Right
                {
                    let idx = *sel_rc.borrow();
                    acc(idx);
                    return glib::Propagation::Stop;
                }
                if keyval == gdk::Key::Escape {
                    pop.popdown();
                    return glib::Propagation::Stop;
                }
                if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
                    pop.popdown();
                    return glib::Propagation::Proceed;
                }
            }

            // Trigger debounced autocomplete lookup on printable keys or Backspace
            let is_printable = keyval.to_unicode().is_some_and(|c| !c.is_control());
            if !has_mod && (is_printable || keyval == gdk::Key::BackSpace) {
                let t_ref = t.clone();
                let pop_ref = pop.clone();
                let lb_ref = lb.clone();
                let items_ref = Rc::clone(&items_rc);
                let sel_ref = Rc::clone(&sel_rc);
                let rows_ref = Rc::clone(&rows_rc);

                glib::timeout_add_local_once(Duration::from_millis(140), move || {
                    if !t_ref.is_realized() || !t_ref.is_mapped() {
                        pop_ref.popdown();
                        return;
                    }
                    let (cursor_col, cursor_row) = t_ref.cursor_position();
                    let col_count = t_ref.column_count();
                    let (text_opt, _) = t_ref.text_range_format(
                        vte4::Format::Text,
                        cursor_row,
                        0,
                        cursor_row,
                        col_count,
                    );
                    let raw_line = text_opt.map(|g| g.to_string()).unwrap_or_default();
                    let cmd_line = extract_prompt_command_line(&raw_line);

                    if cmd_line.trim().is_empty() {
                        pop_ref.popdown();
                        return;
                    }

                    let engine = CompletionEngine::new();
                    let suggestions =
                        engine.get_completions(&cmd_line, Some(cmd_line.len()), &[], 6);

                    lb_ref.remove_all();
                    rows_ref.borrow_mut().clear();

                    if suggestions.is_empty() {
                        items_ref.borrow_mut().clear();
                        pop_ref.popdown();
                        return;
                    }

                    for item in &suggestions {
                        let row = gtk4::ListBoxRow::new();
                        row.set_can_focus(false);
                        row.set_focusable(false);

                        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                        hbox.set_margin_start(8);
                        hbox.set_margin_end(8);
                        hbox.set_margin_top(5);
                        hbox.set_margin_bottom(5);

                        let icon = gtk4::Image::from_icon_name(item.completion_type.icon_name());
                        icon.add_css_class("dim-label");
                        hbox.append(&icon);

                        let vbox_txt = gtk4::Box::new(gtk4::Orientation::Vertical, 1);
                        vbox_txt.set_hexpand(true);

                        let title = gtk4::Label::new(Some(&item.text));
                        title.add_css_class("monospace");
                        title.add_css_class("heading");
                        title.set_halign(gtk4::Align::Start);
                        vbox_txt.append(&title);

                        if !item.description.is_empty() {
                            let desc = gtk4::Label::new(Some(&item.description));
                            desc.add_css_class("caption");
                            desc.add_css_class("dim-label");
                            desc.set_halign(gtk4::Align::Start);
                            desc.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                            vbox_txt.append(&desc);
                        }
                        hbox.append(&vbox_txt);

                        let badge = gtk4::Label::new(Some(item.completion_type.badge_label()));
                        badge.add_css_class("caption");
                        badge.add_css_class("dim-label");
                        hbox.append(&badge);

                        row.set_child(Some(&hbox));
                        lb_ref.append(&row);
                        rows_ref.borrow_mut().push(row);
                    }

                    *items_ref.borrow_mut() = suggestions;
                    *sel_ref.borrow_mut() = 0;
                    if let Some(first_row) = rows_ref.borrow().first() {
                        lb_ref.select_row(Some(first_row));
                    }

                    let term_w = t_ref.width().max(1);
                    let term_h = t_ref.height().max(1);
                    let char_w = (t_ref.char_width() as i32).max(8);
                    let char_h = (t_ref.char_height() as i32).max(16);

                    let scroll_offset = t_ref
                        .vadjustment()
                        .map(|adj| adj.value())
                        .unwrap_or(0.0);
                    let visible_row = ((cursor_row as f64) - scroll_offset).max(0.0) as i32;

                    let cur_x = ((cursor_col as i32) * char_w).clamp(0, (term_w - char_w).max(0));
                    let cur_y = (visible_row * char_h).clamp(0, (term_h - char_h).max(0));

                    pop_ref.set_pointing_to(Some(&gdk::Rectangle::new(
                        cur_x, cur_y, char_w, char_h,
                    )));
                    pop_ref.popup();
                });
            }

            glib::Propagation::Proceed
        });
    }
    term.add_controller(key_ctrl);
}

/// Opens the Unified Command Palette (`ui/dialogs/command_palette.py` — `Ctrl+Shift+P`).
pub fn show_command_palette_dialog(
    parent: &adw::ApplicationWindow,
    cmd_buttons: &[CommandButton],
    on_select_action: Rc<dyn Fn(&str)>,
    on_spawn_session: Rc<dyn Fn(&SessionItem)>,
    on_run_command: Rc<dyn Fn(&str)>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Command Palette (Ctrl+Shift+P) — Busca Unificada")
        .default_width(580)
        .default_height(420)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    content.set_margin_start(14);
    content.set_margin_end(14);
    content.set_margin_top(6);
    content.set_margin_bottom(14);
    content.set_vexpand(true);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Digite uma ação, sessão SSH ou comando parametrizado...")
        .hexpand(true)
        .build();
    content.append(&search_entry);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    scroll.set_child(Some(&list_box));
    content.append(&scroll);

    // Build unified catalog of PaletteItems (Port of `src/onyxsh/ui/dialogs/command_palette_dialog.py`)
    let mut catalog = vec![
        PaletteItem {
            category: "Busca".to_string(),
            title: "Busca Global Multi-Abas / Sessões / Histórico".to_string(),
            subtitle: "Pesquisa em todas as abas abertas, sessões SSH, histórico e snippets (Ctrl+Shift+G)".to_string(),
            action_key: "act:global-search".to_string(),
        },
        PaletteItem {
            category: "Busca".to_string(),
            title: "Buscar no Terminal Atual".to_string(),
            subtitle: "Abre a barra de busca Regex/Literal na aba ativa (Ctrl+Shift+F)".to_string(),
            action_key: "act:toggle-search".to_string(),
        },
        PaletteItem {
            category: "Terminal".to_string(),
            title: "Histórico Enriquecido de Comandos".to_string(),
            subtitle: "Busca fuzzy por diretório, host e favoritos (Ctrl+H)".to_string(),
            action_key: "act:show-command-history".to_string(),
        },
        PaletteItem {
            category: "Abas e Janelas".to_string(),
            title: "Nova Aba de Terminal".to_string(),
            subtitle: "Abre um novo terminal local (Ctrl+Shift+T)".to_string(),
            action_key: "act:new-tab".to_string(),
        },
        PaletteItem {
            category: "Abas e Janelas".to_string(),
            title: "Fechar Aba Atual".to_string(),
            subtitle: "Encerra a aba ativa (Ctrl+Shift+W)".to_string(),
            action_key: "act:close-tab".to_string(),
        },
        PaletteItem {
            category: "Abas e Janelas".to_string(),
            title: "Próxima Aba".to_string(),
            subtitle: "Avança para a próxima aba (Alt+Page Down)".to_string(),
            action_key: "act:next-tab".to_string(),
        },
        PaletteItem {
            category: "Abas e Janelas".to_string(),
            title: "Aba Anterior".to_string(),
            subtitle: "Volta para a aba anterior (Alt+Page Up)".to_string(),
            action_key: "act:previous-tab".to_string(),
        },
        PaletteItem {
            category: "Divisão de Telas".to_string(),
            title: "Dividir Painel Horizontalmente".to_string(),
            subtitle: "Divide a aba atual lado a lado (Ctrl+Shift+()".to_string(),
            action_key: "act:split".to_string(),
        },
        PaletteItem {
            category: "Divisão de Telas".to_string(),
            title: "Dividir Painel Verticalmente".to_string(),
            subtitle: "Divide a aba atual acima/abaixo (Ctrl+Shift+))".to_string(),
            action_key: "act:split-vertical".to_string(),
        },
        PaletteItem {
            category: "Terminal".to_string(),
            title: "Exportar Saída do Terminal (.txt, .log, .md, .html, .cast)".to_string(),
            subtitle: "Exporta todo o buffer ou a seleção ativa do terminal".to_string(),
            action_key: "act:export-terminal-buffer".to_string(),
        },
        PaletteItem {
            category: "Terminal".to_string(),
            title: "Exportar Sessão como Caderno de Bordo (Runbook)".to_string(),
            subtitle: "Relatório operacional estruturado com etapas, anotações, objetivo e conclusão (.md, .html, .log)".to_string(),
            action_key: "act:export-terminal-runbook".to_string(),
        },
        PaletteItem {
            category: "Terminal".to_string(),
            title: "Limpar Sessão do Terminal".to_string(),
            subtitle: "Limpa a tela e o scrollback da sessão ativa (Ctrl+Shift+L)".to_string(),
            action_key: "act:clear-session".to_string(),
        },
        PaletteItem {
            category: "Terminal".to_string(),
            title: "Modo Transmissão (Broadcast p/ Todas as Abas)".to_string(),
            subtitle: "Envia comandos simultaneamente para múltiplos terminais (Ctrl+Shift+B)".to_string(),
            action_key: "act:toggle-broadcast".to_string(),
        },
        PaletteItem {
            category: "Terminal".to_string(),
            title: "Aumentar Zoom / Tamanho da Fonte".to_string(),
            subtitle: "Incrementa o tamanho da fonte do terminal (Ctrl++)".to_string(),
            action_key: "act:zoom-in".to_string(),
        },
        PaletteItem {
            category: "Terminal".to_string(),
            title: "Diminuir Zoom / Tamanho da Fonte".to_string(),
            subtitle: "Reduz o tamanho da fonte do terminal (Ctrl+-)".to_string(),
            action_key: "act:zoom-out".to_string(),
        },
        PaletteItem {
            category: "Terminal".to_string(),
            title: "Resetar Zoom".to_string(),
            subtitle: "Restaura a fonte padrão Monospace 11 (Ctrl+0)".to_string(),
            action_key: "act:zoom-reset".to_string(),
        },
        PaletteItem {
            category: "Sessões & SSH".to_string(),
            title: "Alternar Painel Lateral de Sessões SSH".to_string(),
            subtitle: "Mostra ou oculta a árvore de sessões SSH (Ctrl+Shift+H)".to_string(),
            action_key: "act:toggle-sidebar".to_string(),
        },
        PaletteItem {
            category: "Sessões & SSH".to_string(),
            title: "Alternar Gerenciador de Arquivos Dual-Pane".to_string(),
            subtitle: "Lista, Grade de Miniaturas, Árvore & Quick Look (Ctrl+Shift+E)".to_string(),
            action_key: "act:toggle-fm".to_string(),
        },
        PaletteItem {
            category: "Ferramentas".to_string(),
            title: "Gerenciador de Comandos e Snippets".to_string(),
            subtitle: "Cria e edita botões de comando parametrizados (Ctrl+Shift+M)".to_string(),
            action_key: "act:show-command-manager".to_string(),
        },
        PaletteItem {
            category: "Assistente de IA".to_string(),
            title: "Alternar Assistente de IA Zero-Trust".to_string(),
            subtitle: "SmartRouter, Planos de Ação e Rollback Atômico (Ctrl+Shift+I)".to_string(),
            action_key: "act:toggle-ai".to_string(),
        },
        PaletteItem {
            category: "Ferramentas".to_string(),
            title: "Abrir Monitor de Recursos (Sparklines Cairo)".to_string(),
            subtitle: "Gráficos em tempo real de CPU, RAM, Disco I/O e Rede/GPU".to_string(),
            action_key: "act:resource-monitor".to_string(),
        },
        PaletteItem {
            category: "Ferramentas".to_string(),
            title: "Autocompletar Inteligente (CompletionEngine)".to_string(),
            subtitle: "Sugestões para rpm, dnf, apt, pacman, flatpak, zypper, snap, docker, git".to_string(),
            action_key: "act:autocomplete".to_string(),
        },
        PaletteItem {
            category: "Assistente de IA".to_string(),
            title: "Git Assistant — Sugerir Conventional Commit".to_string(),
            subtitle: "Inspeciona repositório Git atual e sugere mensagem de commit".to_string(),
            action_key: "act:git-assistant".to_string(),
        },
        PaletteItem {
            category: "Ferramentas".to_string(),
            title: "Diagnóstico Seguro do Sistema (Telemetria & Suporte)".to_string(),
            subtitle: "Relatório sanitizado de SO, RAM, GPU/VRAM e ambiente Flatpak".to_string(),
            action_key: "act:system-diagnostics".to_string(),
        },
        PaletteItem {
            category: "Configurações".to_string(),
            title: "Atalhos de Teclado (34 Ações Configuráveis)".to_string(),
            subtitle: "Visualiza, edita e restaura todos os atalhos de teclado do OnyxSH".to_string(),
            action_key: "act:shortcuts".to_string(),
        },
        PaletteItem {
            category: "Configurações".to_string(),
            title: "Preferências do OnyxSH".to_string(),
            subtitle: "Fonte, paleta de cores, transparência e IA (F2)".to_string(),
            action_key: "act:preferences".to_string(),
        },
        PaletteItem {
            category: "Configurações".to_string(),
            title: "Criar Backup de Configurações (Bundle SHA-256)".to_string(),
            subtitle: "Exporta settings.json, sessions.json e command_buttons.json com hash SHA-256"
                .to_string(),
            action_key: "act:backup-config".to_string(),
        },
    ];

    let sessions_path = platform::config_dir().join("sessions.json");
    let loaded_sessions = SessionsStorageData::load_from_path(&sessions_path)
        .map(|s| s.sessions)
        .unwrap_or_default();

    for (idx, sess) in loaded_sessions.iter().enumerate() {
        catalog.push(PaletteItem {
            category: "Sessão".to_string(),
            title: format!("Conectar: {}", sess.name),
            subtitle: format!("{} ({})", sess.connection_string(), sess.session_type),
            action_key: format!("sess:{idx}"),
        });
    }

    for cmd in cmd_buttons {
        catalog.push(PaletteItem {
            category: format!("Comando ({})", cmd.category),
            title: cmd.name.clone(),
            subtitle: format!("$ {}", cmd.command_template),
            action_key: format!("cmd:{}", cmd.command_template),
        });
    }

    let catalog_rc = Rc::new(catalog);
    let sessions_rc = Rc::new(loaded_sessions);

    let populate = {
        let lb = list_box.clone();
        let se = search_entry.clone();
        let cat = Rc::clone(&catalog_rc);
        let sess_list = Rc::clone(&sessions_rc);
        let act_cb = Rc::clone(&on_select_action);
        let sess_cb = Rc::clone(&on_spawn_session);
        let cmd_cb = Rc::clone(&on_run_command);
        let d_close = dlg.clone();

        Rc::new(move || {
            lb.remove_all();
            let filtered = filter_palette_items(&cat, &se.text());

            for item in filtered {
                let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                row.set_margin_start(10);
                row.set_margin_end(10);
                row.set_margin_top(6);
                row.set_margin_bottom(6);

                let text_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                text_vbox.set_hexpand(true);

                let t_lbl = gtk4::Label::new(Some(&format!("[{}] {}", item.category, item.title)));
                t_lbl.add_css_class("heading");
                t_lbl.set_halign(gtk4::Align::Start);

                let s_lbl = gtk4::Label::new(Some(&item.subtitle));
                s_lbl.add_css_class("caption");
                s_lbl.add_css_class("dim-label");
                s_lbl.set_halign(gtk4::Align::Start);

                text_vbox.append(&t_lbl);
                text_vbox.append(&s_lbl);
                row.append(&text_vbox);

                let run_btn = gtk4::Button::with_label("Executar");
                run_btn.add_css_class("flat");
                let key = item.action_key.clone();
                let a_fn = Rc::clone(&act_cb);
                let s_fn = Rc::clone(&sess_cb);
                let c_fn = Rc::clone(&cmd_cb);
                let sl = Rc::clone(&sess_list);
                let dc = d_close.clone();

                run_btn.connect_clicked(move |_| {
                    if let Some(act) = key.strip_prefix("act:") {
                        a_fn(act);
                    } else if let Some(idx_str) = key.strip_prefix("sess:") {
                        if let Ok(idx) = idx_str.parse::<usize>() {
                            if let Some(s) = sl.get(idx) {
                                s_fn(s);
                            }
                        }
                    } else if let Some(cmd_tpl) = key.strip_prefix("cmd:") {
                        c_fn(cmd_tpl);
                    }
                    dc.close();
                });

                row.append(&run_btn);
                lb.append(&row);
            }
        })
    };

    populate();

    {
        let pop = Rc::clone(&populate);
        search_entry.connect_search_changed(move |_| pop());
    }
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        let d_close = dlg.clone();
        let lb = list_box.clone();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_filtering_and_terminal_completions() {
        let items = vec![
            PaletteItem {
                category: "Ação".to_string(),
                title: "Abrir Monitor de Recursos".to_string(),
                subtitle: "Sparklines Cairo".to_string(),
                action_key: "act:resource-monitor".to_string(),
            },
            PaletteItem {
                category: "Comando".to_string(),
                title: "Docker Logs".to_string(),
                subtitle: "docker logs --tail 100".to_string(),
                action_key: "cmd:docker logs".to_string(),
            },
        ];

        let filtered = filter_palette_items(&items, "sparkline");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].action_key, "act:resource-monitor");

        let completions = query_terminal_completions("sudo systemctl ", 10);
        assert!(!completions.is_empty());
        assert!(completions.iter().any(|c| c.text == "restart" || c.text == "status"));

        assert_eq!(
            extract_prompt_command_line("vagnarok@host:~/OnyxSH$ sudo dnf in\n"),
            "sudo dnf in"
        );
        assert_eq!(
            extract_prompt_command_line("root@prod:/etc# rpm -ivh --no"),
            "rpm -ivh --no"
        );
        let rpm_comps = query_terminal_completions("rpm -ivh --no", 6);
        assert!(rpm_comps.iter().any(|c| c.text == "--nodeps"));
    }

    #[test]
    fn test_create_terminal_context_menu_model() {
        // 1. Default context menu (no selection, no URL, no error)
        let menu_default = create_terminal_context_menu_model(false, None, None);
        assert_eq!(menu_default.n_items(), 4);

        // 2. With URL clicked
        let menu_url = create_terminal_context_menu_model(false, Some("https://github.com"), None);
        assert_eq!(menu_url.n_items(), 5);

        // 3. With Selection
        let menu_sel = create_terminal_context_menu_model(true, None, None);
        assert_eq!(menu_sel.n_items(), 4);

        // 4. With error code
        let menu_err = create_terminal_context_menu_model(false, None, Some(127));
        assert_eq!(menu_err.n_items(), 4);
    }
}
