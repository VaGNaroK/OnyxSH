//! Comprehensive Libadwaita Preferences Window (`adw::PreferencesWindow`) for OnyxSH.
//!
//! Full port of `src/onyxsh/ui/dialogs/preferences_dialog.py` matching the 4 screenshots in `CORRIGIR/`:
//! 1. **Aparência (`preferences-desktop-display-symbolic`):**
//!    - `Fonte do Terminal` (`gtk4::FontDialogButton` monospace) + `Espaçamento entre linhas` (`0.80..=2.00`)
//!    - `Transparência do Terminal` (`0..=100`), `Transparência da Barra de Cabeçalho` (`0..=100`)
//!    - `Use Cores Brilhantes para Texto em Negrito` (`bold_is_bright`)
//!    - `Painel de Sessões com Ocultação Automática` (`Ocultar painel ao ativar sessões`)
//!    - `Texto Piscante` (`Quando focado`, `Sempre`)
//!    - `Alinhamento de Guia` (`Esquerda`, `Centro`)
//!    - `Tema de Ícone` (`Ícones do OnyxSH (Embutidos)`, `Ícones do Sistema`)
//!    - `Botões da Janela Quando Maximizada` (`Para integração com painéis do KDE Plasma` -> `Detectar automaticamente`, `Sempre ocultar`, `Nunca ocultar`)
//! 2. **Terminal (`utilities-terminal-symbolic`):**
//!    - `Quando já estiver em execução` (`Abrir uma nova aba`, `Abrir uma nova janela`, `Focar janela existente`)
//!    - `Forma do Cursor` (`Bloco`, `I-Beam`, `Sublinhado`) + `Cursor Piscando` (`Seguir Sistema`, `Ativado`, `Desativado`)
//!    - `Linhas de Rolagem` (`0 para ilimitado`), `Sensibilidade de Rolagem do Mouse`, `Sensibilidade de Rolagem do Touchpad`, `Rolagem ao Colar`
//!    - `Executar Comando como um Shell de Login`, `Sino Audível`
//!    - `Notificações de Comandos` (`Notificar Comandos Longos`, `Tempo Limite Mínimo (Segundos)`, `Condição de Notificação`, `Alerta Sonoro`, `Testar Notificação Agora`)
//!    - `Sugestões Proativas para Erros de Terminal` (`Sugestões Proativas para Erros`, `Modo de Exibição das Sugestões`, `Executar Ações Rápidas Imediatamente`)
//!    - `Autocomplete e Sugestões Inteligentes` (`Ativar Autocomplete Inteligente`, `Catálogo de Comandos Linux (Specs)`, `Sugerir a partir do Histórico`, `Sugerir Snippets Salvos`)
//!    - `Seleção de Texto` (`Barra Flutuante de Ações na Seleção`, `Ponte de Clipboard OSC 52`)
//! 3. **Perfis e Dados (`folder-saved-search-symbolic`):**
//!    - `Restauração de Sessão` (`Na Inicialização`: `Sempre restaurar`, `Perguntar ao iniciar`, `Nunca restaurar`; `Reconectar sessões SSH automaticamente`; `Restaurar Painéis Laterais e Assistente de IA`)
//!    - `Criar Backup` (`Backup criptografado de sessões e configurações` -> `Criar Backup...`) + `Restaurar a partir do Backup` (`Restaurar...`)
//!    - `Usar Diretório Temporário do Sistema` (`Use /tmp para edição remota de arquivos`) + `Limpar Arquivos de Edição Remota ao Sair`
//!    - `SSH e Verificação de Saúde` (`Monitoramento de Saúde e Latência (RTT)`, `Intervalo de Verificação de Latência`, `Reconexão Automática Inteligente em Quedas`, `Tentativas Máximas de Reconexão`, `SSH KeepAlive (ServerAliveInterval)`, `Persistência de Conexão SSH`)
//! 4. **Avançado (`preferences-other-symbolic`):**
//!    - `Suporte a Texto Bidirecional`, `Ativar Formatação de Texto em Árabe`, `Suporte a Gráficos SIXEL`
//!    - `Tecla Backspace`, `Tecla Delete`, `Caracteres de largura ambígua`
//!    - `Caracteres de Palavra` (`-_.:/~`)
//!    - `Salvar Logs em Arquivo`, `Nível de Log do Console` (`DEBUG`, `INFO`, `WARNING`, `ERROR`, `CRITICAL`)
//!    - `Diagnóstico e Suporte Técnico` (`Gerar Diagnóstico Seguro do Sistema` -> `Gerar...`)
//!    - `Redefinir Todas as Configurações` (`Redefinir`)

use crate::main_menu::menu_custom_css;
use crate::theme_engine::{generate_theme_css, get_palette};
use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::platform;
use std::cell::RefCell;
use std::rc::Rc;
use vte4::prelude::*;

/// Helper to parse `#RRGGBB` into `gdk::RGBA`.
fn parse_rgba(hex: &str) -> gdk::RGBA {
    gdk::RGBA::parse(hex).unwrap_or_else(|_| gdk::RGBA::new(0.06, 0.07, 0.09, 1.0))
}

/// Applies all relevant settings from [`AppSettings`] onto a live `vte4::Terminal`.
pub fn apply_all_settings_to_terminal(term: &vte4::Terminal, settings: &AppSettings) {
    let font_desc = pango::FontDescription::from_string(&settings.font);
    term.set_font(Some(&font_desc));
    term.set_font_scale(settings.font_scale);
    term.set_cell_height_scale(settings.line_spacing.clamp(0.8, 2.0));

    let scrollback = if settings.scrollback_lines == 0 {
        -1
    } else {
        settings.scrollback_lines as i64
    };
    term.set_scrollback_lines(scrollback);
    term.set_bold_is_bright(settings.bold_is_bright);
    term.set_audible_bell(settings.bell_sound);
    term.set_scroll_on_keystroke(settings.scroll_on_insert);
    term.set_enable_bidi(settings.bidi_enabled);
    term.set_enable_shaping(settings.enable_shaping);
    term.set_enable_sixel(settings.sixel_enabled);
    term.set_cjk_ambiguous_width(settings.cjk_ambiguous_width.clamp(1, 2) as i32);
    term.set_word_char_exceptions(&settings.word_char_exceptions);

    let cursor_shape = match settings.cursor_shape {
        1 => vte4::CursorShape::Ibeam,
        2 => vte4::CursorShape::Underline,
        _ => vte4::CursorShape::Block,
    };
    term.set_cursor_shape(cursor_shape);

    let cursor_blink = match settings.cursor_blink {
        1 => vte4::CursorBlinkMode::On,
        2 => vte4::CursorBlinkMode::Off,
        _ => vte4::CursorBlinkMode::System,
    };
    term.set_cursor_blink_mode(cursor_blink);

    let text_blink = match settings.text_blink_mode {
        1 => vte4::TextBlinkMode::Always,
        _ => vte4::TextBlinkMode::Focused,
    };
    term.set_text_blink_mode(text_blink);

    let erase_back = match settings.backspace_binding {
        1 => vte4::EraseBinding::AsciiBackspace,
        2 => vte4::EraseBinding::AsciiDelete,
        3 => vte4::EraseBinding::DeleteSequence,
        _ => vte4::EraseBinding::Auto,
    };
    term.set_backspace_binding(erase_back);

    let erase_del = match settings.delete_binding {
        1 => vte4::EraseBinding::AsciiDelete,
        2 => vte4::EraseBinding::DeleteSequence,
        _ => vte4::EraseBinding::Auto,
    };
    term.set_delete_binding(erase_del);

    let palette = get_palette(&settings.color_scheme);
    let fg = parse_rgba(palette.foreground);
    let mut bg = parse_rgba(palette.background);
    let alpha = (100u8.saturating_sub(settings.transparency.min(100))) as f32 / 100.0;
    bg.set_alpha(alpha);
    let cursor = parse_rgba(palette.cursor);
    let ansi_colors: Vec<gdk::RGBA> = palette.ansi.iter().map(|c| parse_rgba(c)).collect();
    let ansi_refs: Vec<&gdk::RGBA> = ansi_colors.iter().collect();

    term.set_colors(Some(&fg), Some(&bg), &ansi_refs);
    term.set_color_cursor(Some(&cursor));
}

/// Refreshes global CSS and all open VTE terminals, then persists `settings.json`.
fn sync_and_persist_settings(
    settings_rc: &Rc<RefCell<AppSettings>>,
    terminals_rc: &Rc<RefCell<Vec<vte4::Terminal>>>,
    css_provider: &gtk4::CssProvider,
) {
    let st = settings_rc.borrow();
    let opacity_pct = 100u8.saturating_sub(st.transparency.min(100));
    let hb_opacity_pct = 100u8.saturating_sub(st.headerbar_transparency.min(100));
    let hb_alpha = (hb_opacity_pct as f32) / 100.0;

    let css = format!(
        "{}\n{}\nheaderbar {{ opacity: {:.2}; }}",
        generate_theme_css(&st.color_scheme, opacity_pct),
        menu_custom_css(),
        hb_alpha.clamp(0.25, 1.0)
    );
    css_provider.load_from_string(&css);

    for term in terminals_rc.borrow().iter() {
        apply_all_settings_to_terminal(term, &st);
    }

    let cfg_path = platform::config_dir().join("settings.json");
    let _ = st.save_to_path(&cfg_path);
}

/// Opens the complete 4-tab `adw::PreferencesWindow` (`src/onyxsh/ui/dialogs/preferences_dialog.py`).
pub fn show_preferences_window(
    parent: &adw::ApplicationWindow,
    settings_rc: Rc<RefCell<AppSettings>>,
    terminals_rc: Rc<RefCell<Vec<vte4::Terminal>>>,
    css_provider: gtk4::CssProvider,
    status_lbl: &gtk4::Label,
) {
    let prefs_win = adw::PreferencesWindow::builder()
        .title("Preferências")
        .transient_for(parent)
        .modal(false)
        .default_width(900)
        .default_height(680)
        .search_enabled(true)
        .build();
    prefs_win.add_css_class("onyxsh-dialog");

    let apply_cb: Rc<dyn Fn()> = {
        let st_rc = Rc::clone(&settings_rc);
        let terms_rc = Rc::clone(&terminals_rc);
        let css_prov = css_provider.clone();
        Rc::new(move || {
            sync_and_persist_settings(&st_rc, &terms_rc, &css_prov);
        })
    };

    // ══════════════════════════════════════════════════════════════════════════
    // PAGE 1: Aparência (`preferences-desktop-display-symbolic`)
    // ══════════════════════════════════════════════════════════════════════════
    let appearance_page = adw::PreferencesPage::builder()
        .title("Aparência")
        .icon_name("preferences-desktop-display-symbolic")
        .build();

    let font_group = adw::PreferencesGroup::new();

    // 1.1 Fonte do Terminal
    let font_row = adw::ActionRow::builder()
        .title("Fonte do Terminal")
        .build();
    let font_dialog = gtk4::FontDialog::new();
    let initial_font_desc = pango::FontDescription::from_string(&settings_rc.borrow().font);
    let font_btn = gtk4::FontDialogButton::builder()
        .dialog(&font_dialog)
        .font_desc(&initial_font_desc)
        .valign(gtk4::Align::Center)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        font_btn.connect_font_desc_notify(move |btn| {
            if let Some(desc) = btn.font_desc() {
                st_rc.borrow_mut().font = desc.to_string();
                apply();
            }
        });
    }
    font_row.add_suffix(&font_btn);
    font_row.set_activatable_widget(Some(&font_btn));
    font_group.add(&font_row);

    // 1.2 Espaçamento entre linhas
    let line_spacing_row = adw::ActionRow::builder()
        .title("Espaçamento entre linhas")
        .build();
    let spacing_spin = gtk4::SpinButton::with_range(0.80, 2.00, 0.05);
    spacing_spin.set_digits(2);
    spacing_spin.set_valign(gtk4::Align::Center);
    spacing_spin.set_value(settings_rc.borrow().line_spacing);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        spacing_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().line_spacing = spin.value();
            apply();
        });
    }
    line_spacing_row.add_suffix(&spacing_spin);
    line_spacing_row.set_activatable_widget(Some(&spacing_spin));
    font_group.add(&line_spacing_row);

    appearance_page.add(&font_group);

    let misc_group = adw::PreferencesGroup::new();

    // 1.3 Transparência do Terminal
    let transparency_row = adw::ActionRow::builder()
        .title("Transparência do Terminal")
        .build();
    let transparency_scale = gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 1.0);
    transparency_scale.set_digits(0);
    transparency_scale.set_draw_value(true);
    transparency_scale.set_value_pos(gtk4::PositionType::Right);
    transparency_scale.set_hexpand(true);
    transparency_scale.set_width_request(220);
    transparency_scale.set_value(settings_rc.borrow().transparency as f64);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        transparency_scale.connect_value_changed(move |scale| {
            st_rc.borrow_mut().transparency = scale.value() as u8;
            apply();
        });
    }
    transparency_row.add_suffix(&transparency_scale);
    transparency_row.set_activatable_widget(Some(&transparency_scale));
    misc_group.add(&transparency_row);

    // 1.4 Transparência da Barra de Cabeçalho
    let hb_transparency_row = adw::ActionRow::builder()
        .title("Transparência da Barra de Cabeçalho")
        .build();
    let hb_transparency_scale =
        gtk4::Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 100.0, 1.0);
    hb_transparency_scale.set_digits(0);
    hb_transparency_scale.set_draw_value(true);
    hb_transparency_scale.set_value_pos(gtk4::PositionType::Right);
    hb_transparency_scale.set_hexpand(true);
    hb_transparency_scale.set_width_request(220);
    hb_transparency_scale.set_value(settings_rc.borrow().headerbar_transparency as f64);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        hb_transparency_scale.connect_value_changed(move |scale| {
            st_rc.borrow_mut().headerbar_transparency = scale.value() as u8;
            apply();
        });
    }
    hb_transparency_row.add_suffix(&hb_transparency_scale);
    hb_transparency_row.set_activatable_widget(Some(&hb_transparency_scale));
    misc_group.add(&hb_transparency_row);

    // 1.5 Use Cores Brilhantes para Texto em Negrito
    let bold_bright_row = adw::SwitchRow::builder()
        .title("Use Cores Brilhantes para Texto em Negrito")
        .active(settings_rc.borrow().bold_is_bright)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        bold_bright_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().bold_is_bright = sw.is_active();
            apply();
        });
    }
    misc_group.add(&bold_bright_row);

    // 1.6 Painel de Sessões com Ocultação Automática
    let auto_hide_row = adw::SwitchRow::builder()
        .title("Painel de Sessões com Ocultação Automática")
        .subtitle("Ocultar painel ao ativar sessões")
        .active(settings_rc.borrow().auto_hide_sidebar)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        auto_hide_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().auto_hide_sidebar = sw.is_active();
            apply();
        });
    }
    misc_group.add(&auto_hide_row);

    // 1.7 Texto Piscante
    let text_blink_model = gtk4::StringList::new(&["Quando focado", "Sempre"]);
    let text_blink_row = adw::ComboRow::builder()
        .title("Texto Piscante")
        .model(&text_blink_model)
        .selected(settings_rc.borrow().text_blink_mode.min(1))
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        text_blink_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().text_blink_mode = row.selected();
            apply();
        });
    }
    misc_group.add(&text_blink_row);

    // 1.8 Alinhamento de Guia
    let tab_align_model = gtk4::StringList::new(&["Esquerda", "Centro"]);
    let initial_align_idx = if settings_rc.borrow().tab_alignment == "left" {
        0
    } else {
        1
    };
    let tab_align_row = adw::ComboRow::builder()
        .title("Alinhamento de Guia")
        .model(&tab_align_model)
        .selected(initial_align_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        tab_align_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().tab_alignment = if row.selected() == 0 {
                "left".to_string()
            } else {
                "center".to_string()
            };
            apply();
        });
    }
    misc_group.add(&tab_align_row);

    // 1.9 Tema de Ícone
    let icon_theme_model =
        gtk4::StringList::new(&["Ícones do OnyxSH (Embutidos)", "Ícones do Sistema"]);
    let initial_icon_idx = if settings_rc.borrow().icon_theme_strategy == "onyxsh" {
        0
    } else {
        1
    };
    let icon_theme_row = adw::ComboRow::builder()
        .title("Tema de Ícone")
        .model(&icon_theme_model)
        .selected(initial_icon_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        icon_theme_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().icon_theme_strategy = if row.selected() == 0 {
                "onyxsh".to_string()
            } else {
                "system".to_string()
            };
            apply();
        });
    }
    misc_group.add(&icon_theme_row);

    // 1.10 Botões da Janela Quando Maximizada
    let hb_buttons_model = gtk4::StringList::new(&[
        "Detectar automaticamente",
        "Sempre ocultar",
        "Nunca ocultar",
    ]);
    let initial_hb_btn_idx = match settings_rc
        .borrow()
        .hide_headerbar_buttons_when_maximized
        .as_str()
    {
        "always" => 1,
        "never" => 2,
        _ => 0,
    };
    let hb_buttons_row = adw::ComboRow::builder()
        .title("Botões da Janela Quando Maximizada")
        .subtitle("Para integração com painéis do KDE Plasma")
        .model(&hb_buttons_model)
        .selected(initial_hb_btn_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        hb_buttons_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().hide_headerbar_buttons_when_maximized = match row.selected() {
                1 => "always".to_string(),
                2 => "never".to_string(),
                _ => "auto".to_string(),
            };
            apply();
        });
    }
    misc_group.add(&hb_buttons_row);

    appearance_page.add(&misc_group);
    prefs_win.add(&appearance_page);

    // ══════════════════════════════════════════════════════════════════════════
    // PAGE 2: Terminal (`utilities-terminal-symbolic`)
    // ══════════════════════════════════════════════════════════════════════════
    let terminal_page = adw::PreferencesPage::builder()
        .title("Terminal")
        .icon_name("utilities-terminal-symbolic")
        .build();

    let behavior_group = adw::PreferencesGroup::new();
    let instance_model = gtk4::StringList::new(&[
        "Abrir uma nova aba",
        "Abrir uma nova janela",
        "Focar janela existente",
    ]);
    let initial_inst_idx = match settings_rc.borrow().new_instance_behavior.as_str() {
        "new_window" => 1,
        "focus_existing" => 2,
        _ => 0,
    };
    let instance_row = adw::ComboRow::builder()
        .title("Quando já estiver em execução")
        .subtitle("Ação quando o aplicativo é iniciado enquanto já está aberto")
        .model(&instance_model)
        .selected(initial_inst_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        instance_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().new_instance_behavior = match row.selected() {
                1 => "new_window".to_string(),
                2 => "focus_existing".to_string(),
                _ => "new_tab".to_string(),
            };
            apply();
        });
    }
    behavior_group.add(&instance_row);
    terminal_page.add(&behavior_group);

    // 2.2 Cursor Group
    let cursor_group = adw::PreferencesGroup::new();
    let cursor_shape_model = gtk4::StringList::new(&["Bloco", "I-Beam", "Sublinhado"]);
    let cursor_shape_row = adw::ComboRow::builder()
        .title("Forma do Cursor")
        .model(&cursor_shape_model)
        .selected(settings_rc.borrow().cursor_shape.min(2))
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        cursor_shape_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().cursor_shape = row.selected();
            apply();
        });
    }
    cursor_group.add(&cursor_shape_row);

    let cursor_blink_model = gtk4::StringList::new(&["Seguir Sistema", "Ativado", "Desativado"]);
    let cursor_blink_row = adw::ComboRow::builder()
        .title("Cursor Piscando")
        .model(&cursor_blink_model)
        .selected(settings_rc.borrow().cursor_blink.min(2))
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        cursor_blink_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().cursor_blink = row.selected();
            apply();
        });
    }
    cursor_group.add(&cursor_blink_row);
    terminal_page.add(&cursor_group);

    // 2.3 Scrolling Group
    let scrolling_group = adw::PreferencesGroup::new();

    let scrollback_row = adw::ActionRow::builder()
        .title("Linhas de Rolagem")
        .subtitle("0 para ilimitado")
        .build();
    let scrollback_spin = gtk4::SpinButton::with_range(0.0, 1_000_000.0, 1_000.0);
    scrollback_spin.set_valign(gtk4::Align::Center);
    scrollback_spin.set_value(settings_rc.borrow().scrollback_lines as f64);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        scrollback_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().scrollback_lines = spin.value() as u32;
            apply();
        });
    }
    scrollback_row.add_suffix(&scrollback_spin);
    scrollback_row.set_activatable_widget(Some(&scrollback_spin));
    scrolling_group.add(&scrollback_row);

    let mouse_scroll_row = adw::ActionRow::builder()
        .title("Sensibilidade de Rolagem do Mouse")
        .subtitle("Mais baixo é mais lento")
        .build();
    let mouse_scroll_spin = gtk4::SpinButton::with_range(1.0, 500.0, 1.0);
    mouse_scroll_spin.set_valign(gtk4::Align::Center);
    mouse_scroll_spin.set_value(settings_rc.borrow().mouse_scroll_sensitivity);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        mouse_scroll_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().mouse_scroll_sensitivity = spin.value();
            apply();
        });
    }
    mouse_scroll_row.add_suffix(&mouse_scroll_spin);
    mouse_scroll_row.set_activatable_widget(Some(&mouse_scroll_spin));
    scrolling_group.add(&mouse_scroll_row);

    let touchpad_scroll_row = adw::ActionRow::builder()
        .title("Sensibilidade de Rolagem do Touchpad")
        .subtitle("Mais baixo é mais lento")
        .build();
    let touchpad_scroll_spin = gtk4::SpinButton::with_range(1.0, 500.0, 1.0);
    touchpad_scroll_spin.set_valign(gtk4::Align::Center);
    touchpad_scroll_spin.set_value(settings_rc.borrow().touchpad_scroll_sensitivity);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        touchpad_scroll_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().touchpad_scroll_sensitivity = spin.value();
            apply();
        });
    }
    touchpad_scroll_row.add_suffix(&touchpad_scroll_spin);
    touchpad_scroll_row.set_activatable_widget(Some(&touchpad_scroll_spin));
    scrolling_group.add(&touchpad_scroll_row);

    let scroll_paste_row = adw::SwitchRow::builder()
        .title("Rolagem ao Colar")
        .active(settings_rc.borrow().scroll_on_insert)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        scroll_paste_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().scroll_on_insert = sw.is_active();
            apply();
        });
    }
    scrolling_group.add(&scroll_paste_row);
    terminal_page.add(&scrolling_group);

    // 2.4 Shell & Bell Group
    let shell_group = adw::PreferencesGroup::new();
    let login_shell_row = adw::SwitchRow::builder()
        .title("Executar Comando como um Shell de Login")
        .active(settings_rc.borrow().use_login_shell)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        login_shell_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().use_login_shell = sw.is_active();
            apply();
        });
    }
    shell_group.add(&login_shell_row);

    let bell_row = adw::SwitchRow::builder()
        .title("Sino Audível")
        .active(settings_rc.borrow().bell_sound)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        bell_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().bell_sound = sw.is_active();
            apply();
        });
    }
    shell_group.add(&bell_row);
    terminal_page.add(&shell_group);

    // 2.5 Command Notifications Group
    let notif_group = adw::PreferencesGroup::builder()
        .title("Notificações de Comandos")
        .description("Alertas nativos do desktop para comandos longos")
        .build();

    let notify_long_row = adw::SwitchRow::builder()
        .title("Notificar Comandos Longos")
        .subtitle("Emitir notificação desktop quando um comando demorado terminar em segundo plano ou em aba inativa")
        .active(settings_rc.borrow().notify_long_commands)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        notify_long_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().notify_long_commands = sw.is_active();
            apply();
        });
    }
    notif_group.add(&notify_long_row);

    let threshold_row = adw::ActionRow::builder()
        .title("Tempo Limite Mínimo (Segundos)")
        .subtitle("Apenas comandos que demorarem mais que este tempo emitirão notificação")
        .build();
    let threshold_spin = gtk4::SpinButton::with_range(3.0, 300.0, 1.0);
    threshold_spin.set_valign(gtk4::Align::Center);
    threshold_spin.set_value(settings_rc.borrow().notify_long_commands_threshold as f64);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        threshold_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().notify_long_commands_threshold = spin.value() as u32;
            apply();
        });
    }
    threshold_row.add_suffix(&threshold_spin);
    threshold_row.set_activatable_widget(Some(&threshold_spin));
    notif_group.add(&threshold_row);

    let cond_model = gtk4::StringList::new(&[
        "Apenas quando a janela ou aba não estiver em foco",
        "Sempre notificar",
    ]);
    let initial_cond_idx = if settings_rc.borrow().notify_long_commands_condition == "always" {
        1
    } else {
        0
    };
    let cond_row = adw::ComboRow::builder()
        .title("Condição de Notificação")
        .subtitle("Quando a notificação desktop deve ser disparada")
        .model(&cond_model)
        .selected(initial_cond_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        cond_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().notify_long_commands_condition = if row.selected() == 1 {
                "always".to_string()
            } else {
                "unfocused".to_string()
            };
            apply();
        });
    }
    notif_group.add(&cond_row);

    let notify_sound_row = adw::SwitchRow::builder()
        .title("Alerta Sonoro")
        .subtitle("Tocar sino ou alerta sonoro ao concluir comando longo")
        .active(settings_rc.borrow().notify_long_commands_sound)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        notify_sound_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().notify_long_commands_sound = sw.is_active();
            apply();
        });
    }
    notif_group.add(&notify_sound_row);

    let test_notif_row = adw::ActionRow::builder()
        .title("Testar Notificação")
        .subtitle("Emitir uma notificação de teste agora para verificar o sistema")
        .build();
    let test_notif_btn = gtk4::Button::with_label("Testar Notificação Agora");
    test_notif_btn.set_valign(gtk4::Align::Center);
    test_notif_btn.add_css_class("suggested-action");
    {
        let status = status_lbl.clone();
        test_notif_btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("notify-send")
                .args([
                    "OnyxSH — Teste de Notificação",
                    "Comando longo finalizado com sucesso (exit 0 • 12.4s).",
                ])
                .spawn();
            status.set_text("🔔 Notificação de teste enviada para o desktop.");
        });
    }
    test_notif_row.add_suffix(&test_notif_btn);
    test_notif_row.set_activatable_widget(Some(&test_notif_btn));
    notif_group.add(&test_notif_row);
    terminal_page.add(&notif_group);

    // 2.6 Proactive Error Suggestions Group
    let error_group = adw::PreferencesGroup::builder()
        .title("Sugestões Proativas para Erros de Terminal")
        .description("Detecção inteligente de falhas no terminal com ações rápidas em 1 clique e diagnóstico por IA")
        .build();

    let proactive_err_row = adw::SwitchRow::builder()
        .title("Sugestões Proativas para Erros")
        .subtitle("Detectar falhas de comandos (ex.: permissão negada, porta em uso, pacote ausente) e sugerir correção imediata")
        .active(settings_rc.borrow().ai_proactive_error_suggestions)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        proactive_err_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().ai_proactive_error_suggestions = sw.is_active();
            apply();
        });
    }
    error_group.add(&proactive_err_row);

    let err_mode_model = gtk4::StringList::new(&[
        "Notificação (Toast) e Badge no Terminal",
        "Apenas Badge no Terminal",
        "Apenas Notificação (Toast)",
    ]);
    let initial_err_mode_idx = match settings_rc.borrow().ai_error_suggestion_mode.as_str() {
        "badge_only" => 1,
        "toast_only" => 2,
        _ => 0,
    };
    let err_mode_row = adw::ComboRow::builder()
        .title("Modo de Exibição das Sugestões")
        .subtitle("Como as sugestões de erro e ações rápidas devem ser apresentadas")
        .model(&err_mode_model)
        .selected(initial_err_mode_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        err_mode_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().ai_error_suggestion_mode = match row.selected() {
                1 => "badge_only".to_string(),
                2 => "toast_only".to_string(),
                _ => "toast_and_badge".to_string(),
            };
            apply();
        });
    }
    error_group.add(&err_mode_row);

    let auto_exec_err_row = adw::SwitchRow::builder()
        .title("Executar Ações Rápidas Imediatamente")
        .subtitle("Se ativado, executa o comando de correção no terminal imediatamente ao clicar; se desativado, apenas insere no prompt para revisão")
        .active(settings_rc.borrow().ai_error_auto_execute_quick_fix)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        auto_exec_err_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().ai_error_auto_execute_quick_fix = sw.is_active();
            apply();
        });
    }
    error_group.add(&auto_exec_err_row);
    terminal_page.add(&error_group);

    // 2.7 Autocomplete Group
    let ac_group = adw::PreferencesGroup::builder()
        .title("Autocomplete e Sugestões Inteligentes")
        .description("Sugestões preditivas inline e catálogo de comandos Linux")
        .build();

    let ac_enable_row = adw::SwitchRow::builder()
        .title("Ativar Autocomplete Inteligente")
        .subtitle("Exibir sugestões de comandos enquanto digita no terminal")
        .active(settings_rc.borrow().autocomplete_enabled)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        ac_enable_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().autocomplete_enabled = sw.is_active();
            apply();
        });
    }
    ac_group.add(&ac_enable_row);

    let ac_specs_row = adw::SwitchRow::builder()
        .title("Catálogo de Comandos Linux (Specs)")
        .subtitle("Sugerir subcomandos e parâmetros para rpm, dnf, apt, pacman, flatpak, systemctl, docker, git e ferramentas comuns")
        .active(settings_rc.borrow().autocomplete_specs_enabled)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        ac_specs_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().autocomplete_specs_enabled = sw.is_active();
            apply();
        });
    }
    ac_group.add(&ac_specs_row);

    let ac_hist_row = adw::SwitchRow::builder()
        .title("Sugerir a partir do Histórico")
        .subtitle("Completar com comandos usados anteriormente no mesmo diretório ou host")
        .active(settings_rc.borrow().autocomplete_history_enabled)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        ac_hist_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().autocomplete_history_enabled = sw.is_active();
            apply();
        });
    }
    ac_group.add(&ac_hist_row);

    let ac_snip_row = adw::SwitchRow::builder()
        .title("Sugerir Snippets Salvos")
        .subtitle("Incluir templates do gerenciador de snippets nas sugestões")
        .active(settings_rc.borrow().autocomplete_snippets_enabled)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        ac_snip_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().autocomplete_snippets_enabled = sw.is_active();
            apply();
        });
    }
    ac_group.add(&ac_snip_row);
    terminal_page.add(&ac_group);

    // 2.8 Selection & OSC 52 Group
    let sel_group = adw::PreferencesGroup::builder()
        .title("Seleção de Texto")
        .description("Ações rápidas flutuantes ao selecionar texto no terminal")
        .build();

    let sel_toolbar_row = adw::SwitchRow::builder()
        .title("Barra Flutuante de Ações na Seleção")
        .subtitle("Exibir menu rápido com Copiar, Perguntar à IA e Refatorar ao selecionar texto")
        .active(settings_rc.borrow().show_selection_toolbar)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        sel_toolbar_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().show_selection_toolbar = sw.is_active();
            apply();
        });
    }
    sel_group.add(&sel_toolbar_row);

    let osc52_row = adw::SwitchRow::builder()
        .title("Ponte de Clipboard OSC 52")
        .subtitle("Permitir que aplicações de terminal e TUIs (OpenCode, Neovim, Tmux) copiem texto para o clipboard do sistema")
        .active(settings_rc.borrow().enable_osc52_clipboard)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        osc52_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().enable_osc52_clipboard = sw.is_active();
            apply();
        });
    }
    sel_group.add(&osc52_row);
    terminal_page.add(&sel_group);

    prefs_win.add(&terminal_page);

    // ══════════════════════════════════════════════════════════════════════════
    // PAGE 3: Perfis e Dados (`folder-saved-search-symbolic`)
    // ══════════════════════════════════════════════════════════════════════════
    let profiles_page = adw::PreferencesPage::builder()
        .title("Perfis e Dados")
        .icon_name("folder-saved-search-symbolic")
        .build();

    let startup_group = adw::PreferencesGroup::builder()
        .title("Restauração de Sessão")
        .description("Comportamento de inicialização e restauração de abas da sessão anterior")
        .build();

    let policy_model = gtk4::StringList::new(&[
        "Sempre restaurar",
        "Perguntar ao iniciar",
        "Nunca restaurar",
    ]);
    let initial_policy_idx = match settings_rc.borrow().session_restore_policy.as_str() {
        "always" => 0,
        "ask" => 1,
        _ => 2,
    };
    let restore_policy_row = adw::ComboRow::builder()
        .title("Na Inicialização")
        .subtitle("Escolha se o terminal deve recuperar as abas e divisões da última sessão")
        .model(&policy_model)
        .selected(initial_policy_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        restore_policy_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().session_restore_policy = match row.selected() {
                0 => "always".to_string(),
                1 => "ask".to_string(),
                _ => "never".to_string(),
            };
            apply();
        });
    }
    startup_group.add(&restore_policy_row);

    let ssh_reconnect_row = adw::SwitchRow::builder()
        .title("Reconectar sessões SSH automaticamente")
        .subtitle("Tenta restabelecer conexões remotas que estavam abertas")
        .active(settings_rc.borrow().session_restore_ssh_auto_reconnect)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        ssh_reconnect_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().session_restore_ssh_auto_reconnect = sw.is_active();
            apply();
        });
    }
    startup_group.add(&ssh_reconnect_row);

    let panels_restore_row = adw::SwitchRow::builder()
        .title("Restaurar Painéis Laterais e Assistente de IA")
        .subtitle("Lembrar visibilidade da barra lateral e do chat de IA")
        .active(settings_rc.borrow().session_restore_ui_panels)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        panels_restore_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().session_restore_ui_panels = sw.is_active();
            apply();
        });
    }
    startup_group.add(&panels_restore_row);
    profiles_page.add(&startup_group);

    // 3.2 Backup & Restore Group
    let backup_group = adw::PreferencesGroup::new();

    let backup_now_row = adw::ActionRow::builder()
        .title("Criar Backup")
        .subtitle("Backup criptografado de sessões e configurações")
        .build();
    let backup_now_btn = gtk4::Button::with_label("Criar Backup...");
    backup_now_btn.set_valign(gtk4::Align::Center);
    {
        let pw_ref = prefs_win.clone();
        let status = status_lbl.clone();
        backup_now_btn.connect_clicked(move |_| {
            let file_dialog = gtk4::FileDialog::builder()
                .title("Salvar Backup do OnyxSH")
                .initial_name("onyxsh_config_backup.json")
                .build();
            let status_inner = status.clone();
            file_dialog.save(Some(&pw_ref), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(out_path) = file.path() {
                        let cfg_dir = platform::config_dir();
                        match onyxsh_core::backup::create_config_backup(&cfg_dir, &out_path) {
                            Ok(bundle) => {
                                status_inner.set_text(&format!(
                                    "Backup SHA-256 salvo ({} arquivos): {}",
                                    bundle.files.len(),
                                    out_path.display()
                                ));
                            }
                            Err(e) => {
                                status_inner.set_text(&format!("Falha ao criar backup: {e}"));
                            }
                        }
                    }
                }
            });
        });
    }
    backup_now_row.add_suffix(&backup_now_btn);
    backup_now_row.set_activatable_widget(Some(&backup_now_btn));
    backup_group.add(&backup_now_row);

    let restore_row = adw::ActionRow::builder()
        .title("Restaurar a partir do Backup")
        .build();
    let restore_btn = gtk4::Button::with_label("Restaurar...");
    restore_btn.set_valign(gtk4::Align::Center);
    {
        let pw_ref = prefs_win.clone();
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        let status = status_lbl.clone();
        restore_btn.connect_clicked(move |_| {
            let file_dialog = gtk4::FileDialog::builder()
                .title("Selecionar Arquivo de Backup do OnyxSH")
                .build();
            let st_inner = Rc::clone(&st_rc);
            let apply_inner = Rc::clone(&apply);
            let status_inner = status.clone();
            file_dialog.open(Some(&pw_ref), gio::Cancellable::NONE, move |res| {
                if let Ok(file) = res {
                    if let Some(in_path) = file.path() {
                        let cfg_dir = platform::config_dir();
                        match onyxsh_core::backup::restore_config_backup(&in_path, &cfg_dir) {
                            Ok(count) => {
                                let cfg_path = cfg_dir.join("settings.json");
                                if let Ok(loaded) = AppSettings::load_from_path(&cfg_path) {
                                    *st_inner.borrow_mut() = loaded;
                                    apply_inner();
                                }
                                status_inner.set_text(&format!(
                                    "Backup restaurado com sucesso ({count} arquivos verificados via SHA-256)."
                                ));
                            }
                            Err(e) => {
                                status_inner.set_text(&format!("Falha ao restaurar backup: {e}"));
                            }
                        }
                    }
                }
            });
        });
    }
    restore_row.add_suffix(&restore_btn);
    restore_row.set_activatable_widget(Some(&restore_btn));
    backup_group.add(&restore_row);
    profiles_page.add(&backup_group);

    // 3.3 Remote Editing Group
    let remote_edit_group = adw::PreferencesGroup::new();
    let use_tmp_row = adw::SwitchRow::builder()
        .title("Usar Diretório Temporário do Sistema")
        .subtitle("Use /tmp para edição remota de arquivos")
        .active(settings_rc.borrow().use_system_tmp_for_edit)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        use_tmp_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().use_system_tmp_for_edit = sw.is_active();
            apply();
        });
    }
    remote_edit_group.add(&use_tmp_row);

    let clear_tmp_row = adw::SwitchRow::builder()
        .title("Limpar Arquivos de Edição Remota ao Sair")
        .active(settings_rc.borrow().clear_remote_edit_files_on_exit)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        clear_tmp_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().clear_remote_edit_files_on_exit = sw.is_active();
            apply();
        });
    }
    remote_edit_group.add(&clear_tmp_row);
    profiles_page.add(&remote_edit_group);

    // 3.4 SSH & Health Check Group
    let ssh_group = adw::PreferencesGroup::builder()
        .title("SSH e Verificação de Saúde")
        .build();

    let health_check_row = adw::SwitchRow::builder()
        .title("Monitoramento de Saúde e Latência (RTT)")
        .subtitle("Mede continuamente a latência e detecta perda de sinal nas conexões SSH")
        .active(settings_rc.borrow().ssh_health_check_enabled)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        health_check_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().ssh_health_check_enabled = sw.is_active();
            apply();
        });
    }
    ssh_group.add(&health_check_row);

    let health_interval_row = adw::ActionRow::builder()
        .title("Intervalo de Verificação de Latência")
        .subtitle("Segundos entre medições de latência RTT em segundo plano")
        .build();
    let health_interval_spin = gtk4::SpinButton::with_range(3.0, 60.0, 1.0);
    health_interval_spin.set_valign(gtk4::Align::Center);
    health_interval_spin.set_value(settings_rc.borrow().ssh_health_check_interval as f64);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        health_interval_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().ssh_health_check_interval = spin.value() as u32;
            apply();
        });
    }
    health_interval_row.add_suffix(&health_interval_spin);
    health_interval_row.set_activatable_widget(Some(&health_interval_spin));
    ssh_group.add(&health_interval_row);

    let auto_reconn_row = adw::SwitchRow::builder()
        .title("Reconexão Automática Inteligente em Quedas")
        .subtitle("Tenta reconectar sessões SSH caídas com contagem regressiva e alerta na tela")
        .active(settings_rc.borrow().ssh_auto_reconnect_enabled)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        auto_reconn_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().ssh_auto_reconnect_enabled = sw.is_active();
            apply();
        });
    }
    ssh_group.add(&auto_reconn_row);

    let reconn_attempts_row = adw::ActionRow::builder()
        .title("Tentativas Máximas de Reconexão")
        .subtitle("Número de tentativas antes de desistir da reconexão automática")
        .build();
    let reconn_attempts_spin = gtk4::SpinButton::with_range(1.0, 20.0, 1.0);
    reconn_attempts_spin.set_valign(gtk4::Align::Center);
    reconn_attempts_spin.set_value(settings_rc.borrow().ssh_auto_reconnect_attempts as f64);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        reconn_attempts_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().ssh_auto_reconnect_attempts = spin.value() as u32;
            apply();
        });
    }
    reconn_attempts_row.add_suffix(&reconn_attempts_spin);
    reconn_attempts_row.set_activatable_widget(Some(&reconn_attempts_spin));
    ssh_group.add(&reconn_attempts_row);

    let keepalive_row = adw::ActionRow::builder()
        .title("SSH KeepAlive (ServerAliveInterval)")
        .subtitle("Intervalo em segundos para enviar pacotes de sonda KeepAlive")
        .build();
    let keepalive_spin = gtk4::SpinButton::with_range(5.0, 120.0, 5.0);
    keepalive_spin.set_valign(gtk4::Align::Center);
    keepalive_spin.set_value(settings_rc.borrow().ssh_keepalive_interval as f64);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        keepalive_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().ssh_keepalive_interval = spin.value() as u32;
            apply();
        });
    }
    keepalive_row.add_suffix(&keepalive_spin);
    keepalive_row.set_activatable_widget(Some(&keepalive_spin));
    ssh_group.add(&keepalive_row);

    let persist_row = adw::ActionRow::builder()
        .title("Persistência de Conexão SSH")
        .subtitle("Segundos para manter conexões ativas (0 para desativar)")
        .build();
    let persist_spin = gtk4::SpinButton::with_range(0.0, 3600.0, 60.0);
    persist_spin.set_valign(gtk4::Align::Center);
    persist_spin.set_value(settings_rc.borrow().ssh_control_persist_duration as f64);
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        persist_spin.connect_value_changed(move |spin| {
            st_rc.borrow_mut().ssh_control_persist_duration = spin.value() as u32;
            apply();
        });
    }
    persist_row.add_suffix(&persist_spin);
    persist_row.set_activatable_widget(Some(&persist_spin));
    ssh_group.add(&persist_row);
    profiles_page.add(&ssh_group);

    prefs_win.add(&profiles_page);

    // ══════════════════════════════════════════════════════════════════════════
    // PAGE 4: Avançado (`preferences-other-symbolic`)
    // ══════════════════════════════════════════════════════════════════════════
    let advanced_page = adw::PreferencesPage::builder()
        .title("Avançado")
        .icon_name("preferences-other-symbolic")
        .build();

    let features_group = adw::PreferencesGroup::new();

    let bidi_row = adw::SwitchRow::builder()
        .title("Suporte a Texto Bidirecional")
        .subtitle("Para árabe e hebraico (afeta o desempenho)")
        .active(settings_rc.borrow().bidi_enabled)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        bidi_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().bidi_enabled = sw.is_active();
            apply();
        });
    }
    features_group.add(&bidi_row);

    let shaping_row = adw::SwitchRow::builder()
        .title("Ativar Formatação de Texto em Árabe")
        .subtitle("Render ligaduras para o script árabe")
        .active(settings_rc.borrow().enable_shaping)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        shaping_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().enable_shaping = sw.is_active();
            apply();
        });
    }
    features_group.add(&shaping_row);

    let sixel_row = adw::SwitchRow::builder()
        .title("Suporte a Gráficos SIXEL")
        .subtitle("Exibir imagens SIXEL (experimental)")
        .active(settings_rc.borrow().sixel_enabled)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        sixel_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().sixel_enabled = sw.is_active();
            apply();
        });
    }
    features_group.add(&sixel_row);
    advanced_page.add(&features_group);

    // 4.2 Compatibility Group
    let compat_group = adw::PreferencesGroup::new();

    let backspace_model = gtk4::StringList::new(&[
        "Automático",
        "ASCII BACKSPACE (^H)",
        "ASCII DELETE",
        "Sequência de Escape",
    ]);
    let backspace_row = adw::ComboRow::builder()
        .title("Tecla Backspace")
        .model(&backspace_model)
        .selected(settings_rc.borrow().backspace_binding.min(3))
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        backspace_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().backspace_binding = row.selected();
            apply();
        });
    }
    compat_group.add(&backspace_row);

    let delete_model = gtk4::StringList::new(&[
        "Automático",
        "ASCII DELETE",
        "Sequência de Escape",
    ]);
    let delete_row = adw::ComboRow::builder()
        .title("Tecla Delete")
        .model(&delete_model)
        .selected(settings_rc.borrow().delete_binding.min(2))
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        delete_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().delete_binding = row.selected();
            apply();
        });
    }
    compat_group.add(&delete_row);

    let cjk_model = gtk4::StringList::new(&[
        "Estreito (célula única)",
        "Largo (célula dupla)",
    ]);
    let initial_cjk_idx = settings_rc
        .borrow()
        .cjk_ambiguous_width
        .saturating_sub(1)
        .min(1);
    let cjk_row = adw::ComboRow::builder()
        .title("Caracteres de largura ambígua")
        .model(&cjk_model)
        .selected(initial_cjk_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        cjk_row.connect_selected_notify(move |row| {
            st_rc.borrow_mut().cjk_ambiguous_width = row.selected() + 1;
            apply();
        });
    }
    compat_group.add(&cjk_row);
    advanced_page.add(&compat_group);

    // 4.3 Word Characters Group
    let word_group = adw::PreferencesGroup::new();
    let word_chars_row = adw::EntryRow::builder()
        .title("Caracteres de Palavra")
        .text(&settings_rc.borrow().word_char_exceptions)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        word_chars_row.connect_changed(move |entry| {
            st_rc.borrow_mut().word_char_exceptions = entry.text().to_string();
            apply();
        });
    }
    word_group.add(&word_chars_row);
    advanced_page.add(&word_group);

    // 4.4 Logging Group
    let log_group = adw::PreferencesGroup::new();
    let log_to_file_row = adw::SwitchRow::builder()
        .title("Salvar Logs em Arquivo")
        .active(settings_rc.borrow().log_to_file)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        log_to_file_row.connect_active_notify(move |sw| {
            st_rc.borrow_mut().log_to_file = sw.is_active();
            apply();
        });
    }
    log_group.add(&log_to_file_row);

    let log_levels = ["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"];
    let log_level_model = gtk4::StringList::new(&log_levels);
    let curr_lvl = settings_rc.borrow().console_log_level.to_uppercase();
    let initial_lvl_idx = log_levels
        .iter()
        .position(|&l| l == curr_lvl)
        .unwrap_or(3) as u32;
    let log_level_row = adw::ComboRow::builder()
        .title("Nível de Log do Console")
        .model(&log_level_model)
        .selected(initial_lvl_idx)
        .build();
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        log_level_row.connect_selected_notify(move |row| {
            let idx = row.selected() as usize;
            let lvl = log_levels.get(idx).copied().unwrap_or("ERROR");
            st_rc.borrow_mut().console_log_level = lvl.to_string();
            apply();
        });
    }
    log_group.add(&log_level_row);
    advanced_page.add(&log_group);

    // 4.5 Diagnostics & Technical Support Group
    let diag_group = adw::PreferencesGroup::builder()
        .title("Diagnóstico e Suporte Técnico")
        .description("Gere um relatório técnico sanitizado do sistema para envio ao suporte e abertura de issues.")
        .build();

    let diag_row = adw::ActionRow::builder()
        .title("Gerar Diagnóstico Seguro do Sistema")
        .subtitle("Coleta ambiente, GPU, bibliotecas e logs mascarando dados pessoais")
        .build();
    let diag_btn = gtk4::Button::with_label("Gerar...");
    diag_btn.set_valign(gtk4::Align::Center);
    diag_btn.add_css_class("accent");
    {
        let win_ref = parent.clone();
        let status = status_lbl.clone();
        diag_btn.connect_clicked(move |_| {
            crate::global_search_and_shortcuts::show_system_diagnostics_dialog(&win_ref, &status);
        });
    }
    diag_row.add_suffix(&diag_btn);
    diag_row.set_activatable_widget(Some(&diag_btn));
    diag_group.add(&diag_row);
    advanced_page.add(&diag_group);

    // 4.6 Reset All Settings Group
    let reset_group = adw::PreferencesGroup::new();
    let reset_row = adw::ActionRow::builder()
        .title("Redefinir Todas as Configurações")
        .build();
    let reset_btn = gtk4::Button::with_label("Redefinir");
    reset_btn.set_valign(gtk4::Align::Center);
    reset_btn.add_css_class("destructive-action");
    {
        let st_rc = Rc::clone(&settings_rc);
        let apply = Rc::clone(&apply_cb);
        let pw_ref = prefs_win.clone();
        let status = status_lbl.clone();
        reset_btn.connect_clicked(move |_| {
            *st_rc.borrow_mut() = AppSettings::default();
            apply();
            status.set_text("Todas as configurações foram restauradas para o padrão.");
            pw_ref.close();
        });
    }
    reset_row.add_suffix(&reset_btn);
    reset_row.set_activatable_widget(Some(&reset_btn));
    reset_group.add(&reset_row);
    advanced_page.add(&reset_group);

    prefs_win.add(&advanced_page);

    // Escape key closes PreferencesWindow
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        let pw_ref = prefs_win.clone();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Escape {
                pw_ref.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        prefs_win.add_controller(key_ctrl);
    }

    prefs_win.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comprehensive_preferences_settings_roundtrip() {
        let mut st = AppSettings::default();
        assert_eq!(st.font, "Ubuntu Mono Regular 12");
        assert!((st.line_spacing - 1.0).abs() < f64::EPSILON);
        assert_eq!(st.transparency, 16);
        assert_eq!(st.headerbar_transparency, 12);
        assert!(st.auto_hide_sidebar);
        assert_eq!(st.tab_alignment, "center");
        assert_eq!(st.scrollback_lines, 5000);
        assert_eq!(st.word_char_exceptions, "-_.:/~");
        assert_eq!(st.console_log_level, "ERROR");

        st.line_spacing = 1.25;
        st.cursor_shape = 1;
        st.sixel_enabled = false;
        st.ssh_keepalive_interval = 30;

        let json = serde_json::to_string(&st).expect("serialize settings");
        let loaded: AppSettings = serde_json::from_str(&json).expect("deserialize settings");
        assert!((loaded.line_spacing - 1.25).abs() < f64::EPSILON);
        assert_eq!(loaded.cursor_shape, 1);
        assert!(!loaded.sixel_enabled);
        assert_eq!(loaded.ssh_keepalive_interval, 30);
    }
}
