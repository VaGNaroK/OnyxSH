//! Phase 15: Agent Scope & Security Policies Dialog.
//! Ports `src/onyxsh/ui/dialogs/agent_scope_dialog.py`.
//!
//! Displays allowed directories for AI actions and active default-deny protections matching `politicadesegurançaagente.png`.

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::tr;
use std::cell::RefCell;
use std::rc::Rc;

/// Active security protection rule displayed in the Default-Deny section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeProtectionRule {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub badge: &'static str,
}

/// Returns the builtin security policies matching `src/onyxsh/ui/dialogs/agent_scope_dialog.py` and `politicadesegurançaagente.png`.
pub fn builtin_protection_rules() -> &'static [ScopeProtectionRule] {
    &[
        ScopeProtectionRule {
            title: "Bloqueio de Chaves e Credenciais",
            subtitle: "~/.ssh, ~/.gnupg, ~/.aws, ~/.docker, arquivos .env e históricos",
            badge: "🛡️ Ativo",
        },
        ScopeProtectionRule {
            title: "Proteção de Inicialização de Shell",
            subtitle: "~/.bashrc, ~/.zshrc, ~/.profile e diretórios de sistema",
            badge: "🛡️ Ativo",
        },
        ScopeProtectionRule {
            title: "Ofuscação Automática de Segredos",
            subtitle: "Filtra chaves de API, senhas e tokens antes de qualquer envio ao LLM",
            badge: "🛡️ Ativo",
        },
    ]
}

/// Displays the Agent Scope & Security Policies Dialog (`politicadesegurançaagente.png`).
pub fn show_agent_scope_dialog(
    parent: &adw::ApplicationWindow,
    settings: &Rc<RefCell<AppSettings>>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Configuração de Escopo e Segurança do Agente"))
        .default_width(680)
        .default_height(520)
        .build();

    let root_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Header bar with "Cancelar" on the left and Close on the right
    let header_bar = adw::HeaderBar::new();
    header_bar.set_show_end_title_buttons(true);
    header_bar.set_show_start_title_buttons(false);

    let title_lbl = gtk4::Label::builder()
        .label(&tr("Configuração de Escopo e Segurança do Agente"))
        .css_classes(["title", "heading"])
        .build();
    header_bar.set_title_widget(Some(&title_lbl));

    let btn_cancel = gtk4::Button::builder()
        .label(&tr("Cancelar"))
        .css_classes(["flat"])
        .build();
    let d_close = dlg.clone();
    btn_cancel.connect_clicked(move |_| {
        d_close.close();
    });
    header_bar.pack_start(&btn_cancel);
    root_vbox.append(&header_bar);

    let content_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    content_vbox.set_margin_top(16);
    content_vbox.set_margin_bottom(16);
    content_vbox.set_margin_start(16);
    content_vbox.set_margin_end(16);

    // 1. Allowed Roots Group (matching python agent_scope_dialog.py)
    let roots_group = adw::PreferencesGroup::new();
    roots_group.set_title(&tr("Diretórios Permitidos para Ações do Agente"));
    roots_group.set_description(Some(&tr(
        "O agente tem permissão de leitura/escrita restrita apenas aos diretórios listados abaixo.",
    )));

    let st = settings.borrow();
    let allowed_roots = if st.ai_agent_allowed_roots.is_empty() {
        onyxsh_core::config::default_ai_agent_allowed_roots()
    } else {
        st.ai_agent_allowed_roots.clone()
    };
    drop(st);

    for path_str in allowed_roots {
        let row = adw::ActionRow::builder().title(&path_str).build();
        roots_group.add(&row);
    }
    content_vbox.append(&roots_group);

    // 2. Protection Policies Group (Default-Deny)
    let policy_group = adw::PreferencesGroup::new();
    policy_group.set_title(&tr("Proteções Ativas (Default-Deny)"));

    for rule in builtin_protection_rules() {
        let row = adw::ActionRow::builder()
            .title(&tr(rule.title))
            .subtitle(&tr(rule.subtitle))
            .build();

        let badge_txt = format!("🛡️ {}", tr("Active"));
        let badge = gtk4::Label::builder()
            .label(&badge_txt)
            .css_classes(["accent"])
            .build();
        row.add_suffix(&badge);
        policy_group.add(&row);
    }
    content_vbox.append(&policy_group);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_child(Some(&content_vbox));
    root_vbox.append(&scroll);

    dlg.set_content(Some(&root_vbox));
    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_protection_rules() {
        let rules = builtin_protection_rules();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].title, "Bloqueio de Chaves e Credenciais");
        assert!(rules[0].subtitle.contains("~/.ssh"));
        assert_eq!(rules[0].badge, "🛡️ Ativo");

        assert_eq!(rules[1].title, "Proteção de Inicialização de Shell");
        assert!(rules[1].subtitle.contains("~/.bashrc"));

        assert_eq!(rules[2].title, "Ofuscação Automática de Segredos");
        assert!(rules[2].subtitle.contains("chaves de API"));
    }

    #[test]
    fn test_default_allowed_roots_resolution() {
        let roots = onyxsh_core::config::default_ai_agent_allowed_roots();
        assert_eq!(roots.len(), 4);
        assert!(roots[0].ends_with("Documents"));
        assert!(roots[1].ends_with("Projects"));
        assert!(roots[2].ends_with("Downloads"));
        assert!(roots[3].ends_with("scripts"));
    }
}

