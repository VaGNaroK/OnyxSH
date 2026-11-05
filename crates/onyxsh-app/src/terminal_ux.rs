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
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::platform;
use onyxsh_core::sessions::{SessionItem, SessionsStorageData};
use onyxsh_terminal::completion::{CompletionEngine, CompletionItem};
use onyxsh_terminal::production_guard::{GuardViolation, ProductionGuard};
use std::rc::Rc;
use vte4::prelude::*;

/// Entry in the unified Command Palette (`Ctrl+Shift+P`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem {
    pub category: String,
    pub title: String,
    pub subtitle: String,
    pub action_key: String,
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
        "Digite um comando (ex: 'git ', 'docker run ', 'sudo systemctl ', 'cargo ', 'flatpak '):",
    ));
    hint.add_css_class("caption");
    hint.set_halign(gtk4::Align::Start);
    content.append(&hint);

    let entry = gtk4::Entry::builder()
        .text("sudo systemctl ")
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
            while let Some(c) = lb.first_child() {
                lb.remove(&c);
            }
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

/// Attaches a right-click `SelectionPopover` (`ui/widgets/selection_popover.py`) and
/// `ProductionGuard` Enter-key interceptor to a `vte4::Terminal`.
pub fn attach_vte_ux_controllers(
    term: &vte4::Terminal,
    parent_window: &adw::ApplicationWindow,
    ai_widget: &gtk4::Box,
    btn_ai_toggle: &gtk4::ToggleButton,
    submit_ai_prompt_cb: Rc<dyn Fn(&str)>,
    status_lbl: &gtk4::Label,
) {
    // 1. Right-click Contextual `SelectionPopover`
    let popover = gtk4::Popover::new();
    popover.set_parent(term);
    popover.set_has_arrow(true);

    let pop_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    pop_vbox.set_margin_start(6);
    pop_vbox.set_margin_end(6);
    pop_vbox.set_margin_top(6);
    pop_vbox.set_margin_bottom(6);

    let btn_copy = gtk4::Button::with_label("📋 Copiar Seleção");
    btn_copy.add_css_class("flat");
    let btn_paste = gtk4::Button::with_label("📥 Colar");
    btn_paste.add_css_class("flat");
    let btn_explain_ai = gtk4::Button::with_label("🤖 Explicar / Diagnosticar com IA");
    btn_explain_ai.add_css_class("flat");
    let btn_autocomplete = gtk4::Button::with_label("⌨️ Autocompletar (CompletionEngine)");
    btn_autocomplete.add_css_class("flat");

    pop_vbox.append(&btn_copy);
    pop_vbox.append(&btn_paste);
    pop_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
    pop_vbox.append(&btn_explain_ai);
    pop_vbox.append(&btn_autocomplete);
    popover.set_child(Some(&pop_vbox));

    {
        let t = term.clone();
        let p = popover.clone();
        let st = status_lbl.clone();
        btn_copy.connect_clicked(move |_| {
            t.copy_clipboard_format(vte4::Format::Text);
            st.set_text("Texto selecionado copiado do terminal.");
            p.popdown();
        });
    }
    {
        let t = term.clone();
        let p = popover.clone();
        btn_paste.connect_clicked(move |_| {
            t.paste_clipboard();
            p.popdown();
        });
    }
    {
        let t = term.clone();
        let p = popover.clone();
        let ai_w = ai_widget.clone();
        let ai_btn = btn_ai_toggle.clone();
        let sub_ai = Rc::clone(&submit_ai_prompt_cb);
        let st = status_lbl.clone();

        btn_explain_ai.connect_clicked(move |_| {
            p.popdown();
            let selected = t
                .text_selected(vte4::Format::Text)
                .map(|g| g.to_string())
                .unwrap_or_default();
            let query = if selected.trim().is_empty() {
                "Explique o último comando ou erro do terminal".to_string()
            } else {
                format!("Analise e explique esta saída do terminal:\n{}", selected.trim())
            };
            ai_w.set_visible(true);
            ai_btn.set_active(true);
            sub_ai(&query);
            st.set_text("Seleção enviada para análise no Assistente de IA Zero-Trust.");
        });
    }
    {
        let win_ref = parent_window.clone();
        let t = term.clone();
        let p = popover.clone();
        let st = status_lbl.clone();
        btn_autocomplete.connect_clicked(move |_| {
            p.popdown();
            let t_clone = t.clone();
            show_autocomplete_assistant_dialog(
                &win_ref,
                Rc::new(move || Some(t_clone.clone())),
                &st,
            );
        });
    }

    let click_ctrl = gtk4::GestureClick::new();
    click_ctrl.set_button(3); // Right mouse button
    {
        let p = popover;
        click_ctrl.connect_pressed(move |_, _, x, y| {
            p.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
            p.popup();
        });
    }
    term.add_controller(click_ctrl);
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

    // Build unified catalog of PaletteItems
    let mut catalog = vec![
        PaletteItem {
            category: "Ação".to_string(),
            title: "Nova Aba de Terminal".to_string(),
            subtitle: "Abre um novo terminal local (Ctrl+Shift+T)".to_string(),
            action_key: "act:new-tab".to_string(),
        },
        PaletteItem {
            category: "Ação".to_string(),
            title: "Dividir Terminal (Split Pane)".to_string(),
            subtitle: "Divide a aba atual em 2 terminais lado a lado".to_string(),
            action_key: "act:split".to_string(),
        },
        PaletteItem {
            category: "Ação".to_string(),
            title: "Alternar Gerenciador de Arquivos Dual-Pane".to_string(),
            subtitle: "Lista, Grade de Miniaturas, Árvore & Quick Look".to_string(),
            action_key: "act:toggle-fm".to_string(),
        },
        PaletteItem {
            category: "Ação".to_string(),
            title: "Alternar Assistente de IA Zero-Trust".to_string(),
            subtitle: "SmartRouter, Planos de Ação e Rollback Atômico".to_string(),
            action_key: "act:toggle-ai".to_string(),
        },
        PaletteItem {
            category: "Ação".to_string(),
            title: "Abrir Monitor de Recursos (Sparklines Cairo)".to_string(),
            subtitle: "Gráficos em tempo real de CPU, RAM, Disco I/O e Rede/GPU".to_string(),
            action_key: "act:resource-monitor".to_string(),
        },
        PaletteItem {
            category: "Ação".to_string(),
            title: "Autocompletar Inteligente (CompletionEngine)".to_string(),
            subtitle: "Sugestões contextuais para git, docker, systemctl, cargo, apt".to_string(),
            action_key: "act:autocomplete".to_string(),
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
            while let Some(c) = lb.first_child() {
                lb.remove(&c);
            }
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
    }
}
