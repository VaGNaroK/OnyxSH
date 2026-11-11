//! AI Assistant Configuration Dialog (`adw::PreferencesWindow`) for OnyxSH.
//!
//! Full native port of `src/onyxsh/ui/dialogs/ai_config_dialog.py`:
//! - Provider selection (Groq, Gemini, OpenRouter, Local Ollama/LM Studio).
//! - Privacy & Strict Offline Mode toggle.
//! - Smart Routing profile (Auto, Always Fast, Always Advanced) and provider/model bindings.
//! - Dedicated API keys per provider with test buttons.
//! - Zero-Trust Agent parameters: Risk Level (0 to 3), Auto-run Level 0, Post-verification, PWD/OS Context.
//! - Context window size tokens (4K to 128K) and visual Risk Levels guide.
//! - Persists to `~/.config/onyxsh/settings.json`.

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform;
use std::cell::RefCell;
use std::rc::Rc;

/// Provider descriptor tuple: (id, display_label, default_model).
pub const AI_PROVIDERS: &[(&str, &str, &str)] = &[
    ("groq", "Groq", "llama-3.1-8b-instant"),
    ("gemini", "Google Gemini", "gemini-2.5-flash"),
    ("openrouter", "OpenRouter", "openrouter/polaris-alpha"),
    ("local", "Local (Ollama / LM Studio)", "llama3.2"),
];

/// Context size options: (token_count, display_label).
pub const CONTEXT_SIZES: &[(u32, &str)] = &[
    (4096, "4.096 tokens (4K) — Econômico (CPU / GPU < 4GB)"),
    (8192, "8.192 tokens (8K) — Padrão Equilibrado"),
    (16384, "16.384 tokens (16K) — Estendido (GPU 8GB - 12GB)"),
    (32768, "32.768 tokens (32K) — Amplo (GPU 12GB - 16GB / Cloud)"),
    (65536, "65.536 tokens (64K) — Avançado (GPU 24GB+ / Cloud)"),
    (131072, "131.072 tokens (128K) — Máximo (Provedores Cloud)"),
];

fn provider_to_index(id: &str) -> u32 {
    match id {
        "gemini" => 1,
        "openrouter" => 2,
        "local" => 3,
        _ => 0, // groq default
    }
}

fn index_to_provider(idx: u32) -> &'static str {
    match idx {
        1 => "gemini",
        2 => "openrouter",
        3 => "local",
        _ => "groq",
    }
}

fn context_size_to_index(tokens: u32) -> u32 {
    match tokens {
        4096 => 0,
        16384 => 2,
        32768 => 3,
        65536 => 4,
        131072 => 5,
        _ => 1, // 8192 default
    }
}

fn index_to_context_size(idx: u32) -> u32 {
    match idx {
        0 => 4096,
        2 => 16384,
        3 => 32768,
        4 => 65536,
        5 => 131072,
        _ => 8192,
    }
}

/// Helper to persist modified settings to `~/.config/onyxsh/settings.json`.
fn save_settings(settings: &AppSettings) {
    let path = platform::config_dir().join("settings.json");
    let _ = settings.save_to_path(&path);
}

/// Opens the full AI Assistant Configuration Window (`win.configure-ai`).
pub fn show_ai_config_window(
    parent: &adw::ApplicationWindow,
    settings_rc: Rc<RefCell<AppSettings>>,
    status_label: &gtk4::Label,
) {
    let win = adw::PreferencesWindow::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Configure AI Assistant — OnyxSH"))
        .default_width(750)
        .default_height(640)
        .build();

    let page = adw::PreferencesPage::builder()
        .title(&tr("Assistente de IA"))
        .icon_name("avatar-default-symbolic")
        .build();

    // ── Group 1: Ativação Geral ───────────────────────────────────────────────
    let enable_group = adw::PreferencesGroup::new();
    let row_enable = adw::SwitchRow::builder()
        .title(&tr("Enable AI Assistant"))
        .subtitle(&tr("Show the AI Assistant button and side panel in the top bar."))
        .active(settings_rc.borrow().ai_enabled)
        .build();
    enable_group.add(&row_enable);
    page.add(&enable_group);

    // ── Group 2: Privacidade e Modo Offline ──────────────────────────────────
    let privacy_group = adw::PreferencesGroup::builder()
        .title(&tr("Privacidade e Modo Offline"))
        .description(&tr("External connection control and strict data isolation."))
        .build();

    let row_offline = adw::SwitchRow::builder()
        .title(&tr("Modo Estritamente Offline (Local-Only)"))
        .subtitle(&tr("Bloqueia qualquer requisição externa (Gemini, Groq, OpenRouter) e restringe a IA a modelos locais no Ollama ou LM Studio."))
        .active(settings_rc.borrow().ai_offline_mode)
        .build();
    privacy_group.add(&row_offline);
    page.add(&privacy_group);

    // ── Group 3: Provedor Principal e Modelos ─────────────────────────────────
    let provider_group = adw::PreferencesGroup::builder()
        .title(&tr("Primary Provider and Connectivity"))
        .description(&tr("Choose between high-speed cloud providers or local servers."))
        .build();

    let row_provider = adw::ComboRow::builder()
        .title(&tr("Default Provider"))
        .subtitle(&tr("Provider used when Smart Routing is disabled"))
        .build();
    let p_groq = tr("Groq • Ultra-Fast LPU");
    let p_gemini = tr("Google Gemini • High Context Capacity");
    let p_openrouter = tr("OpenRouter • Unified Multi-Model Catalog");
    let p_local = tr("Local (Ollama / LM Studio) • Full GPU/CPU Privacy");
    let prov_list = gtk4::StringList::new(&[&p_groq, &p_gemini, &p_openrouter, &p_local]);
    row_provider.set_model(Some(&prov_list));
    row_provider.set_selected(provider_to_index(&settings_rc.borrow().ai_provider));
    provider_group.add(&row_provider);

    let row_base_url = adw::EntryRow::builder()
        .title(&tr("Local Server Base URL"))
        .text(&settings_rc.borrow().ai_local_base_url)
        .build();
    provider_group.add(&row_base_url);

    let row_preload = adw::SwitchRow::builder()
        .title(&tr("Pré-carregar Modelo na VRAM ao Iniciar"))
        .subtitle(&tr("Carrega o modelo na GPU em segundo plano ao abrir o terminal para respostas imediatas."))
        .active(settings_rc.borrow().ai_preload_local_model)
        .build();
    provider_group.add(&row_preload);

    let row_unload = adw::SwitchRow::builder()
        .title(&tr("Liberar VRAM ao Fechar o Terminal"))
        .subtitle(&tr("Descarrega o modelo da memória GPU imediatamente ao fechar a aplicação."))
        .active(settings_rc.borrow().ai_unload_on_exit)
        .build();
    provider_group.add(&row_unload);

    let row_api_key = adw::PasswordEntryRow::builder()
        .title(&tr("Default API Key"))
        .text(&settings_rc.borrow().ai_api_key)
        .build();
    provider_group.add(&row_api_key);

    let row_model = adw::EntryRow::builder()
        .title(&tr("Default Model Identifier"))
        .text(&settings_rc.borrow().ai_model)
        .build();
    provider_group.add(&row_model);

    page.add(&provider_group);

    // ── Group 4: Roteamento Inteligente (Smart Routing) ──────────────────────
    let routing_group = adw::PreferencesGroup::builder()
        .title(&tr("Roteamento Inteligente de Modelos (Smart Routing)"))
        .description(&tr("Aloca automaticamente entre modelos rápidos e modelos avançados com base na complexidade da tarefa."))
        .build();

    let row_smart_routing = adw::SwitchRow::builder()
        .title(&tr("Ativar Roteamento Inteligente"))
        .subtitle(&tr("Direciona consultas simples para o perfil rápido e planos/scripts complexos para o perfil avançado."))
        .active(settings_rc.borrow().ai_smart_routing)
        .build();
    routing_group.add(&row_smart_routing);

    let row_routing_mode = adw::ComboRow::builder()
        .title(&tr("Modo de Roteamento Padrão"))
        .subtitle(&tr("Escolha se a IA deve alternar automaticamente ou fixar um perfil."))
        .build();
    let rm_auto = tr("Automático (Classifica complexidade do prompt)");
    let rm_fast = tr("Sempre Rápido (Baixa latência / Groq / Local)");
    let rm_adv = tr("Sempre Avançado (Raciocínio profundo / Gemini / Claude)");
    let route_mode_list = gtk4::StringList::new(&[&rm_auto, &rm_fast, &rm_adv]);
    row_routing_mode.set_model(Some(&route_mode_list));
    row_routing_mode.set_selected(match settings_rc.borrow().ai_routing_profile.as_str() {
        "fast" => 1,
        "advanced" => 2,
        _ => 0,
    });
    routing_group.add(&row_routing_mode);

    // Expander Perfil Rápido
    let exp_fast = adw::ExpanderRow::builder()
        .title(&tr("⚡ Perfil Rápido (Consultas Pontuais / Sintaxe)"))
        .subtitle(&tr("Low-latency model and provider for daily shell questions"))
        .build();
    let row_fast_prov = adw::ComboRow::builder()
        .title(&tr("Provedor Rápido"))
        .build();
    row_fast_prov.set_model(Some(&prov_list));
    row_fast_prov.set_selected(provider_to_index(&settings_rc.borrow().ai_fast_provider));
    exp_fast.add_row(&row_fast_prov);

    let row_fast_model = adw::EntryRow::builder()
        .title(&tr("Modelo Rápido"))
        .text(&settings_rc.borrow().ai_fast_model)
        .build();
    exp_fast.add_row(&row_fast_model);
    routing_group.add(&exp_fast);

    // Expander Perfil Avançado
    let exp_adv = adw::ExpanderRow::builder()
        .title(&tr("🧠 Perfil Avançado (Planos Complexos / Scripts / Diagnósticos)"))
        .subtitle(&tr("High-reasoning model and provider for multi-step plans"))
        .build();
    let row_adv_prov = adw::ComboRow::builder()
        .title(&tr("Provedor Avançado"))
        .build();
    row_adv_prov.set_model(Some(&prov_list));
    row_adv_prov.set_selected(provider_to_index(&settings_rc.borrow().ai_advanced_provider));
    exp_adv.add_row(&row_adv_prov);

    let row_adv_model = adw::EntryRow::builder()
        .title(&tr("Modelo Avançado"))
        .text(&settings_rc.borrow().ai_advanced_model)
        .build();
    exp_adv.add_row(&row_adv_model);
    routing_group.add(&exp_adv);

    // Expander Chaves de API Dedicadas
    let exp_keys = adw::ExpanderRow::builder()
        .title(&tr("🔑 Chaves de API por Provedor"))
        .subtitle(&tr("Configure individual keys to switch providers without reconfiguring"))
        .build();

    let row_key_gemini = adw::PasswordEntryRow::builder()
        .title(&tr("Google Gemini API Key"))
        .text(&settings_rc.borrow().ai_api_key_gemini)
        .build();
    let btn_test_gemini = gtk4::Button::builder()
        .label(&tr("Testar"))
        .valign(gtk4::Align::Center)
        .build();
    btn_test_gemini.add_css_class("flat");
    row_key_gemini.add_suffix(&btn_test_gemini);
    exp_keys.add_row(&row_key_gemini);

    let row_key_groq = adw::PasswordEntryRow::builder()
        .title(&tr("Groq API Key"))
        .text(&settings_rc.borrow().ai_api_key_groq)
        .build();
    let btn_test_groq = gtk4::Button::builder()
        .label(&tr("Testar"))
        .valign(gtk4::Align::Center)
        .build();
    btn_test_groq.add_css_class("flat");
    row_key_groq.add_suffix(&btn_test_groq);
    exp_keys.add_row(&row_key_groq);

    let row_key_openrouter = adw::PasswordEntryRow::builder()
        .title(&tr("OpenRouter API Key"))
        .text(&settings_rc.borrow().ai_api_key_openrouter)
        .build();
    let btn_test_openrouter = gtk4::Button::builder()
        .label(&tr("Testar"))
        .valign(gtk4::Align::Center)
        .build();
    btn_test_openrouter.add_css_class("flat");
    row_key_openrouter.add_suffix(&btn_test_openrouter);
    exp_keys.add_row(&row_key_openrouter);

    routing_group.add(&exp_keys);
    page.add(&routing_group);

    // ── Group 5: Modo Agente Seguro e Contexto ───────────────────────────────
    let agent_group = adw::PreferencesGroup::builder()
        .title(&tr("Modo Agente Seguro e Contexto"))
        .description(&tr("Parâmetros de proteção, níveis de autonomia e contexto do sistema."))
        .build();

    let row_risk = adw::ComboRow::builder()
        .title(&tr("Nível Máximo de Autonomia Permitido"))
        .subtitle(&tr("Define o limite máximo de risco para comandos propostos pela IA."))
        .build();
    let rl0 = tr("🟢 Nível 0: Somente Leitura (Diagnóstico e Inspeção)");
    let rl1 = tr("🔵 Nível 1: Escrita no Usuário (Criar/Editar Scripts e Arquivos)");
    let rl2 = tr("🟠 Nível 2: Administração Polkit (Serviços e Manutenção)");
    let rl3 = tr("🔴 Nível 3: Operações Críticas (Pacotes e Sistema Global)");
    let risk_list = gtk4::StringList::new(&[&rl0, &rl1, &rl2, &rl3]);
    row_risk.set_model(Some(&risk_list));
    row_risk.set_selected(settings_rc.borrow().ai_agent_max_risk_level.min(3) as u32);
    agent_group.add(&row_risk);

    let row_auto_l0 = adw::SwitchRow::builder()
        .title(&tr("Auto-executar Comandos Seguros (Nível 0)"))
        .subtitle(&tr("Automatically runs read-only diagnostics without requiring manual confirmation."))
        .active(settings_rc.borrow().ai_agent_auto_run_level0)
        .build();
    agent_group.add(&row_auto_l0);

    let row_post_verify = adw::SwitchRow::builder()
        .title(&tr("Sugerir Verificação Pós-Execução (Sanity Loop)"))
        .subtitle(&tr("Infers and proposes automatic validations (service status, config syntax) after mutating commands."))
        .active(settings_rc.borrow().ai_agent_post_verification)
        .build();
    agent_group.add(&row_post_verify);

    let row_auto_verify = adw::SwitchRow::builder()
        .title(&tr("Executar Verificações Automaticamente"))
        .subtitle(&tr("Dispara os testes de sanidade automaticamente sem exigir clique manual no botão de validar."))
        .active(settings_rc.borrow().ai_agent_auto_verify)
        .build();
    agent_group.add(&row_auto_verify);

    let row_sys_context = adw::SwitchRow::builder()
        .title(&tr("Incluir Contexto do Sistema e Hardware"))
        .subtitle(&tr("Informa à IA a distribuição Linux, arquitetura e dados do sistema para comandos exatos."))
        .active(settings_rc.borrow().ai_agent_include_system_context)
        .build();
    agent_group.add(&row_sys_context);

    let row_pwd_context = adw::SwitchRow::builder()
        .title(&tr("Incluir Diretório Atual (PWD) no Contexto"))
        .subtitle(&tr("Permite que a IA use caminhos relativos ao diretório em que você está navegando."))
        .active(settings_rc.borrow().ai_agent_include_pwd_context)
        .build();
    agent_group.add(&row_pwd_context);

    let row_context_size = adw::ComboRow::builder()
        .title(&tr("Context Window Size (Tokens)"))
        .subtitle(&tr("Maximum context memory allocation sent to the model"))
        .build();
    let cs0 = tr(CONTEXT_SIZES[0].1);
    let cs1 = tr(CONTEXT_SIZES[1].1);
    let cs2 = tr(CONTEXT_SIZES[2].1);
    let cs3 = tr(CONTEXT_SIZES[3].1);
    let cs4 = tr(CONTEXT_SIZES[4].1);
    let cs5 = tr(CONTEXT_SIZES[5].1);
    let ctx_list = gtk4::StringList::new(&[&cs0, &cs1, &cs2, &cs3, &cs4, &cs5]);
    row_context_size.set_model(Some(&ctx_list));
    row_context_size.set_selected(context_size_to_index(settings_rc.borrow().ai_context_size));
    agent_group.add(&row_context_size);

    // Expander Guia dos Níveis de Risco
    let exp_guide = adw::ExpanderRow::builder()
        .title(&tr("ℹ️ Guia dos Níveis de Risco do Contexto"))
        .subtitle(&tr("Clique para entender o que cada nível de permissão autoriza ou restringe"))
        .build();
    let row_g0 = adw::ActionRow::builder()
        .title(&tr("🟢 Nível 0: Somente Leitura (Diagnóstico)"))
        .subtitle(&tr("Comandos puramente informativos (lshw, df, free, ps, ls, cat). Risco zero."))
        .build();
    let row_g1 = adw::ActionRow::builder()
        .title(&tr("🔵 Nível 1: Escrita no Espaço do Usuário"))
        .subtitle(&tr("Criação de scripts, edição de arquivos e git na pasta home. Requer 1 clique de aprovação."))
        .build();
    let row_g2 = adw::ActionRow::builder()
        .title(&tr("🟠 Nível 2: Administração Polkit"))
        .subtitle(&tr("Serviços e configurações do sistema (systemctl, journalctl). Requer autenticação segura."))
        .build();
    let row_g3 = adw::ActionRow::builder()
        .title(&tr("🔴 Nível 3: Operações Críticas Globais"))
        .subtitle(&tr("Instalação/remoção de pacotes e arquivos em /etc (apt, dpkg). Exige confirmação explícita."))
        .build();
    let row_g4 = adw::ActionRow::builder()
        .title(&tr("⛔ Nível 4: Ações Destrutivas (Bloqueado)"))
        .subtitle(&tr("Comandos perigosos (rm -rf /, mkfs, dd). Permanentemente bloqueados pelo motor de segurança."))
        .build();
    exp_guide.add_row(&row_g0);
    exp_guide.add_row(&row_g1);
    exp_guide.add_row(&row_g2);
    exp_guide.add_row(&row_g3);
    exp_guide.add_row(&row_g4);
    agent_group.add(&exp_guide);

    page.add(&agent_group);
    win.add(&page);

    // ── Signal Connections and Real-time Persistence ─────────────────────────
    {
        let st = Rc::clone(&settings_rc);
        let status = status_label.clone();
        row_enable.connect_active_notify(move |r| {
            st.borrow_mut().ai_enabled = r.is_active();
            save_settings(&st.borrow());
            let state_str = if r.is_active() { tr("Enabled") } else { tr("Disabled") };
            status.set_text(&tr_fmt("AI Assistant: {state}", &[("state", &state_str)]));
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_offline.connect_active_notify(move |r| {
            st.borrow_mut().ai_offline_mode = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_provider.connect_selected_notify(move |r| {
            let prov = index_to_provider(r.selected());
            st.borrow_mut().ai_provider = prov.to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_base_url.connect_apply(move |r| {
            st.borrow_mut().ai_local_base_url = r.text().to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_preload.connect_active_notify(move |r| {
            st.borrow_mut().ai_preload_local_model = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_unload.connect_active_notify(move |r| {
            st.borrow_mut().ai_unload_on_exit = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_api_key.connect_apply(move |r| {
            st.borrow_mut().ai_api_key = r.text().to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_model.connect_apply(move |r| {
            st.borrow_mut().ai_model = r.text().to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_smart_routing.connect_active_notify(move |r| {
            st.borrow_mut().ai_smart_routing = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_routing_mode.connect_selected_notify(move |r| {
            let prof = match r.selected() {
                1 => "fast",
                2 => "advanced",
                _ => "auto",
            };
            st.borrow_mut().ai_routing_profile = prof.to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_fast_prov.connect_selected_notify(move |r| {
            st.borrow_mut().ai_fast_provider = index_to_provider(r.selected()).to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_fast_model.connect_apply(move |r| {
            st.borrow_mut().ai_fast_model = r.text().to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_adv_prov.connect_selected_notify(move |r| {
            st.borrow_mut().ai_advanced_provider = index_to_provider(r.selected()).to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_adv_model.connect_apply(move |r| {
            st.borrow_mut().ai_advanced_model = r.text().to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_key_gemini.connect_apply(move |r| {
            st.borrow_mut().ai_api_key_gemini = r.text().to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_key_groq.connect_apply(move |r| {
            st.borrow_mut().ai_api_key_groq = r.text().to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_key_openrouter.connect_apply(move |r| {
            st.borrow_mut().ai_api_key_openrouter = r.text().to_string();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_risk.connect_selected_notify(move |r| {
            st.borrow_mut().ai_agent_max_risk_level = r.selected().min(3) as u8;
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_auto_l0.connect_active_notify(move |r| {
            st.borrow_mut().ai_agent_auto_run_level0 = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_post_verify.connect_active_notify(move |r| {
            st.borrow_mut().ai_agent_post_verification = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_auto_verify.connect_active_notify(move |r| {
            st.borrow_mut().ai_agent_auto_verify = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_sys_context.connect_active_notify(move |r| {
            st.borrow_mut().ai_agent_include_system_context = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_pwd_context.connect_active_notify(move |r| {
            st.borrow_mut().ai_agent_include_pwd_context = r.is_active();
            save_settings(&st.borrow());
        });
    }

    {
        let st = Rc::clone(&settings_rc);
        row_context_size.connect_selected_notify(move |r| {
            st.borrow_mut().ai_context_size = index_to_context_size(r.selected());
            save_settings(&st.borrow());
        });
    }

    // Connect test buttons
    let wire_key_test = |btn: &gtk4::Button, entry: &adw::PasswordEntryRow, provider: &'static str| {
        let win_c = win.clone();
        let rk = entry.clone();
        btn.connect_clicked(move |_| {
            let key = rk.text();
            let msg = if key.is_empty() {
                tr_fmt("Please enter a {provider} API key before testing.", &[("provider", provider)])
            } else {
                tr_fmt("{provider} API key configured successfully.", &[("provider", provider)])
            };
            win_c.add_toast(adw::Toast::new(&msg));
        });
    };
    wire_key_test(&btn_test_gemini, &row_key_gemini, "Google Gemini");
    wire_key_test(&btn_test_groq, &row_key_groq, "Groq");
    wire_key_test(&btn_test_openrouter, &row_key_openrouter, "OpenRouter");

    win.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_provider_and_context_size_indexes() {
        assert_eq!(provider_to_index("groq"), 0);
        assert_eq!(provider_to_index("gemini"), 1);
        assert_eq!(provider_to_index("openrouter"), 2);
        assert_eq!(provider_to_index("local"), 3);

        assert_eq!(index_to_provider(0), "groq");
        assert_eq!(index_to_provider(1), "gemini");
        assert_eq!(index_to_provider(2), "openrouter");
        assert_eq!(index_to_provider(3), "local");

        assert_eq!(context_size_to_index(4096), 0);
        assert_eq!(context_size_to_index(8192), 1);
        assert_eq!(context_size_to_index(16384), 2);
        assert_eq!(context_size_to_index(131072), 5);

        assert_eq!(index_to_context_size(0), 4096);
        assert_eq!(index_to_context_size(1), 8192);
        assert_eq!(index_to_context_size(5), 131072);
    }
}
