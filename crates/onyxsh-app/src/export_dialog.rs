//! Export Terminal & Runbook (Caderno de Bordo) Dialog for OnyxSH.
//!
//! Full port of `src/onyxsh/ui/dialogs/export_dialog.py`:
//! - Dual-mode `gtk4::StackSwitcher` + `gtk4::Stack`:
//!   1. `📜 Exportação Direta` (Direct Export):
//!      - Scope: `Buffer Completo do Terminal` vs `Apenas Texto Selecionado`
//!      - Formats: `Texto Puro (.txt)`, `Arquivo de Log (.log)`, `Documento Markdown (.md)`,
//!        `Página HTML Estilizada (.html)`, `Gravação Asciinema (.cast)`
//!   2. `📘 Caderno de Bordo (Runbook)`:
//!      - Metadata: `Título do Procedimento`, `Operador / Autor`, `Objetivo / Contexto`,
//!        `Status do Procedimento` (`Concluído`, `Em Andamento`, `Incidente Investigado`, `Manutenção Preventiva`)
//!      - Interactive Steps & Annotations (`adw::ExpanderRow` per command with include checkbox,
//!        `Anotação / Comentário:` entry row, and output snippet row)
//!      - `Conclusão & Lições Aprendidas` (`Notas Finais do Procedimento`)
//!      - Runbook Formats: `Markdown Runbook (.md)`, `Relatório HTML Interativo (.html)`,
//!        `Log Estruturado do Runbook (.log)` + `Recolher saídas longas de comandos` (`adw::SwitchRow`)
//! - Shared Live Preview (`Pré-visualização do Arquivo`) + `Copiar para a Área de Transferência` + `Salvar como Arquivo...`

use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform;
use onyxsh_terminal::exporter::{format_direct_export, ExportMetadata};
use onyxsh_terminal::osc7::create_display_path;
use onyxsh_terminal::runbook::{RunbookData, RunbookMetadata, RunbookStep};
use onyxsh_terminal::semantic_tracker::SemanticCommand;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use vte4::prelude::*;

/// Direct export format descriptors matching `ExportTerminalDialog.FORMAT_OPTIONS` in Python.
pub const DIRECT_FORMAT_OPTIONS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "txt",
        "Plain Text (.txt)",
        "Clean unformatted terminal output",
        ".txt",
        "text/plain",
    ),
    (
        "log",
        "Log File (.log)",
        "Output with session, date and system headers",
        ".log",
        "text/plain",
    ),
    (
        "md",
        "Markdown Document (.md)",
        "Formatted with code blocks and metadata table",
        ".md",
        "text/markdown",
    ),
    (
        "html",
        "Styled HTML Page (.html)",
        "Self-contained modern dark-themed webpage",
        ".html",
        "text/html",
    ),
    (
        "cast",
        "Asciinema Recording (.cast)",
        "Asciinema v2 JSON format for CLI & web players",
        ".cast",
        "application/json",
    ),
];

/// Runbook (Caderno de Bordo) format descriptors matching `ExportTerminalDialog.RUNBOOK_FORMAT_OPTIONS`.
pub const RUNBOOK_FORMAT_OPTIONS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "runbook_md",
        "Caderno de Bordo Markdown (.md)",
        "Formato de documentação com callouts, badges e etapas",
        ".md",
        "text/markdown",
    ),
    (
        "runbook_html",
        "Interactive HTML Report (.html)",
        "Página web autônoma em tema escuro com estilo de impressão / PDF",
        ".html",
        "text/html",
    ),
    (
        "runbook_log",
        "Log Estruturado de Caderno de Bordo (.log)",
        "Text report for ticketing systems and audit attachments",
        ".log",
        "text/plain",
    ),
];

/// Status options for the Runbook procedure dropdown.
pub const RUNBOOK_STATUS_OPTIONS: &[&str] = &[
    "Concluído",
    "Em Andamento",
    "Incidente Investigado",
    "Manutenção Preventiva",
];

/// Extracts runtime [`ExportMetadata`] from an active `vte4::Terminal`.
pub fn extract_terminal_export_metadata(term: &vte4::Terminal, tab_title: Option<&str>) -> ExportMetadata {
    let home_str = platform::home_dir().to_string_lossy().to_string();
    let cwd = term
        .current_directory_uri()
        .map(|uri| {
            let path = uri.as_str().strip_prefix("file://").unwrap_or(uri.as_str());
            create_display_path(path, &home_str)
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "~".to_string());

    let host = std::env::var("HOSTNAME")
        .or_else(|_| std::fs::read_to_string("/etc/hostname").map(|s| s.trim().to_string()))
        .unwrap_or_else(|_| "localhost".to_string());

    let session_name = tab_title
        .filter(|t| !t.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| tr("Local Terminal"));

    let cols = (term.column_count().max(20)) as u32;
    let rows = (term.row_count().max(5)) as u32;

    let unix_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1793793600);

    let timestamp = glib::DateTime::now_local()
        .ok()
        .and_then(|dt| dt.format("%Y-%m-%d %H:%M:%S").ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "2026-11-05 12:00:00".to_string());

    ExportMetadata {
        session_name,
        host,
        cwd,
        cols,
        rows,
        timestamp,
        unix_timestamp,
    }
}

/// Extracts full or selected text from a `vte4::Terminal`.
pub fn extract_terminal_buffer_text(term: &vte4::Terminal, selection_only: bool) -> String {
    if selection_only && term.has_selection() {
        if let Some(sel) = term.text_selected(vte4::Format::Text) {
            let s = sel.to_string();
            if !s.trim().is_empty() {
                return s;
            }
        }
    }

    let col_count = term.column_count().max(80);
    let row_count = term.row_count().max(24);
    let (range_opt, _) = term.text_range_format(
        vte4::Format::Text,
        0,
        0,
        row_count.saturating_add(5000),
        col_count,
    );
    if let Some(txt) = range_opt {
        let s = txt.to_string();
        if !s.trim().is_empty() {
            return s;
        }
    }

    term.text_format(vte4::Format::Text)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Builds initial [`RunbookData`] from raw terminal text and [`ExportMetadata`].
pub fn build_runbook_data_from_text(raw_text: &str, meta: &ExportMetadata) -> RunbookData {
    let steps = RunbookData::parse_fallback_steps(raw_text);
    build_runbook_data_from_steps(steps, meta)
}

fn build_runbook_data_from_steps(steps: Vec<RunbookStep>, meta: &ExportMetadata) -> RunbookData {
    let author = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "operador".to_string());

    let total_commands = steps.len();

    RunbookData {
        metadata: RunbookMetadata {
            title: tr("Procedimento Operacional / Runbook"),
            author,
            objective: String::new(),
            general_notes: String::new(),
            conclusion: String::new(),
            status: tr("Concluído"),
            session_name: meta.session_name.clone(),
            host: meta.host.clone(),
            date_str: meta.timestamp.clone(),
            total_commands,
        },
        steps,
    }
}

/// Converts captured [`SemanticCommand`] entries into [`RunbookStep`] records using an output extractor.
pub fn build_runbook_steps_from_semantic<F>(
    cmds: &[SemanticCommand],
    mut extract_output: F,
) -> Vec<RunbookStep>
where
    F: FnMut(i64, i64) -> String,
{
    cmds.iter()
        .filter(|c| !c.command_text.trim().is_empty())
        .map(|cmd| {
            let output = match cmd.output_end_row {
                Some(end_row) if end_row >= cmd.output_start_row => {
                    extract_output(cmd.output_start_row, end_row)
                }
                _ => String::new(),
            };
            RunbookStep {
                command: cmd.command_text.clone(),
                output,
                exit_code: cmd.exit_code,
                duration_str: cmd.formatted_duration(),
                timestamp_str: String::new(),
                cwd: cmd.cwd.clone(),
                annotation: String::new(),
                included: true,
            }
        })
        .collect()
}

/// Builds initial [`RunbookData`] for a live VTE terminal, preferring captured OSC 133 semantic
/// commands when available and falling back to regex prompt parsing from `raw_text`.
pub fn build_runbook_data_for_terminal(
    term: &vte4::Terminal,
    raw_text: &str,
    meta: &ExportMetadata,
) -> RunbookData {
    let recorded = crate::terminal_ux::recorded_semantic_commands_for(term);
    if !recorded.is_empty() {
        let cols = (term.column_count().max(1) - 1).max(0);
        let steps: Vec<RunbookStep> = recorded
            .iter()
            .filter(|cmd| !cmd.command_text.trim().is_empty())
            .map(|cmd| {
                let output = crate::terminal_ux::captured_output_for_command_id(term, &cmd.command_id)
                    .unwrap_or_else(|| {
                        if let Some(end_row) = cmd.output_end_row {
                            let start = cmd.output_start_row.min(end_row);
                            let end = cmd.output_start_row.max(end_row);
                            let (txt_opt, _) = term.text_range_format(
                                vte4::Format::Text,
                                start,
                                0,
                                end,
                                cols,
                            );
                            txt_opt
                                .map(|g| g.to_string().trim_end().to_string())
                                .unwrap_or_default()
                        } else {
                            String::new()
                        }
                    });
                RunbookStep {
                    command: cmd.command_text.clone(),
                    output,
                    exit_code: cmd.exit_code,
                    duration_str: cmd.formatted_duration(),
                    timestamp_str: String::new(),
                    cwd: cmd.cwd.clone(),
                    annotation: String::new(),
                    included: true,
                }
            })
            .collect();
        if !steps.is_empty() {
            return build_runbook_data_from_steps(steps, meta);
        }
    }
    build_runbook_data_from_text(raw_text, meta)
}

/// Truncates formatted export content for the 25-line Live Preview box (`_update_preview` in Python).
pub fn build_preview_snippet(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines {
        content.to_string()
    } else {
        format!(
            "{}\n\n{}",
            lines[..max_lines].join("\n"),
            tr("... (restante da saída truncado na pré-visualização)")
        )
    }
}

#[derive(Debug, Clone)]
struct ExportDialogState {
    is_runbook_mode: bool,
    selection_only: bool,
    selected_format: String,
    selected_runbook_format: String,
    collapse_long_outputs: bool,
    full_buffer_text: String,
    selected_buffer_text: String,
    export_meta: ExportMetadata,
    runbook_data: RunbookData,
}

impl ExportDialogState {
    fn compute_active_output(&self) -> (String, &'static str, &'static str) {
        if self.is_runbook_mode {
            self.runbook_data
                .format_runbook(&self.selected_runbook_format, self.collapse_long_outputs)
        } else {
            let source = if self.selection_only && !self.selected_buffer_text.trim().is_empty() {
                &self.selected_buffer_text
            } else {
                &self.full_buffer_text
            };
            format_direct_export(source, &self.selected_format, &self.export_meta)
        }
    }
}

/// Opens the complete "Exportar Terminal e Caderno de Bordo" Libadwaita dialog
/// (`src/onyxsh/ui/dialogs/export_dialog.py`).
pub fn show_export_terminal_dialog(
    parent: &adw::ApplicationWindow,
    term: &vte4::Terminal,
    tab_title: Option<&str>,
    start_in_runbook_mode: bool,
    status_lbl: &gtk4::Label,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Export Terminal & Runbook"))
        .default_width(720)
        .default_height(660)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();

    let btn_cancel_top = gtk4::Button::with_label(&tr("Cancelar"));
    {
        let d = dlg.clone();
        btn_cancel_top.connect_clicked(move |_| d.close());
    }
    header.pack_start(&btn_cancel_top);
    toolbar_view.add_top_bar(&header);

    let has_selection = term.has_selection();
    let full_buffer_text = extract_terminal_buffer_text(term, false);
    let selected_buffer_text = if has_selection {
        extract_terminal_buffer_text(term, true)
    } else {
        String::new()
    };
    let export_meta = extract_terminal_export_metadata(term, tab_title);
    let runbook_data = build_runbook_data_for_terminal(term, &full_buffer_text, &export_meta);

    let state_rc = Rc::new(RefCell::new(ExportDialogState {
        is_runbook_mode: start_in_runbook_mode,
        selection_only: false,
        selected_format: "md".to_string(),
        selected_runbook_format: "runbook_md".to_string(),
        collapse_long_outputs: true,
        full_buffer_text,
        selected_buffer_text,
        export_meta,
        runbook_data,
    }));

    // Main outer vertical box inside ScrolledWindow
    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    main_box.set_margin_top(14);
    main_box.set_margin_bottom(14);
    main_box.set_margin_start(18);
    main_box.set_margin_end(18);

    // 1. Mode Switcher (`📜 Exportação Direta` vs `📘 Caderno de Bordo (Runbook)`)
    let switcher_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    switcher_box.set_halign(gtk4::Align::Center);
    switcher_box.set_margin_bottom(4);

    let stack_switcher = gtk4::StackSwitcher::new();
    let stack = gtk4::Stack::builder()
        .transition_type(gtk4::StackTransitionType::SlideLeftRight)
        .transition_duration(200)
        .vhomogeneous(false)
        .interpolate_size(true)
        .build();
    stack_switcher.set_stack(Some(&stack));
    switcher_box.append(&stack_switcher);
    main_box.append(&switcher_box);

    // 3. Shared Live Preview Card (created before pages so callbacks can refresh it)
    let preview_group = adw::PreferencesGroup::builder()
        .title(&tr("Pré-visualização do Arquivo"))
        .build();
    let preview_card = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    preview_card.add_css_class("card");

    let preview_scroller = gtk4::ScrolledWindow::builder()
        .min_content_height(125)
        .max_content_height(155)
        .hexpand(true)
        .vexpand(false)
        .build();

    let preview_text_view = gtk4::TextView::builder()
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk4::WrapMode::None)
        .top_margin(10)
        .bottom_margin(10)
        .left_margin(12)
        .right_margin(12)
        .build();
    preview_text_view.add_css_class("card");
    let preview_buffer = preview_text_view.buffer();
    preview_scroller.set_child(Some(&preview_text_view));
    preview_card.append(&preview_scroller);
    preview_group.add(&preview_card);

    let update_preview: Rc<dyn Fn()> = {
        let st_rc = Rc::clone(&state_rc);
        let buf = preview_buffer.clone();
        Rc::new(move || {
            let (content, _, _) = st_rc.borrow().compute_active_output();
            let snippet = build_preview_snippet(&content, 25);
            buf.set_text(&snippet);
        })
    };

    // ── Page 1: Direct Export (`📜 Exportação Direta`) ─────────────────────────
    let direct_page = gtk4::Box::new(gtk4::Orientation::Vertical, 14);

    let scope_group = adw::PreferencesGroup::builder()
        .title(&tr("Escopo da Exportação"))
        .description(&tr(
            "Escolha entre exportar todo o histórico do terminal ou apenas a seleção ativa.",
        ))
        .build();

    let scope_full_row = adw::ActionRow::builder()
        .title(&tr("Buffer Completo do Terminal"))
        .subtitle(&tr("Todos os comandos e saídas gravados nesta sessão"))
        .build();
    let scope_full_check = gtk4::CheckButton::builder().active(true).build();
    scope_full_row.add_prefix(&scope_full_check);
    scope_full_row.set_activatable_widget(Some(&scope_full_check));
    scope_group.add(&scope_full_row);

    let scope_sel_subtitle = if has_selection {
        tr("Apenas o trecho atualmente destacado no terminal")
    } else {
        tr("Nenhum texto selecionado no terminal")
    };
    let scope_sel_row = adw::ActionRow::builder()
        .title(&tr("Apenas Texto Selecionado"))
        .subtitle(&scope_sel_subtitle)
        .sensitive(has_selection)
        .build();
    let scope_sel_check = gtk4::CheckButton::builder()
        .group(&scope_full_check)
        .active(false)
        .sensitive(has_selection)
        .build();
    scope_sel_row.add_prefix(&scope_sel_check);
    scope_sel_row.set_activatable_widget(Some(&scope_sel_check));
    scope_group.add(&scope_sel_row);

    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        let sel_chk = scope_sel_check.clone();
        scope_full_check.connect_toggled(move |_| {
            st_rc.borrow_mut().selection_only = sel_chk.is_active();
            up();
        });
    }
    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        scope_sel_check.connect_toggled(move |chk| {
            st_rc.borrow_mut().selection_only = chk.is_active();
            up();
        });
    }

    direct_page.append(&scope_group);

    let format_group = adw::PreferencesGroup::builder()
        .title(&tr("Formato do Arquivo"))
        .description(&tr("Selecione o formato de destino do texto exportado."))
        .build();

    let mut first_fmt_check: Option<gtk4::CheckButton> = None;
    for &(fmt_id, title, desc, _ext, _mime) in DIRECT_FORMAT_OPTIONS {
        let row = adw::ActionRow::builder()
            .title(&tr(title))
            .subtitle(&tr(desc))
            .build();
        let check = if let Some(ref group_leader) = first_fmt_check {
            gtk4::CheckButton::builder()
                .group(group_leader)
                .active(fmt_id == "md")
                .build()
        } else {
            let c = gtk4::CheckButton::builder()
                .active(fmt_id == "md")
                .build();
            first_fmt_check = Some(c.clone());
            c
        };

        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        let fmt_owned = fmt_id.to_string();
        check.connect_toggled(move |chk| {
            if chk.is_active() {
                st_rc.borrow_mut().selected_format = fmt_owned.clone();
                up();
            }
        });

        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));
        format_group.add(&row);
    }

    direct_page.append(&format_group);
    stack.add_titled(&direct_page, Some("direct"), &tr("📜 Exportação Direta"));

    // ── Page 2: Caderno de Bordo / Runbook (`📘 Caderno de Bordo (Runbook)`) ──
    let runbook_page = gtk4::Box::new(gtk4::Orientation::Vertical, 14);

    // 2.1 Metadata Group
    let meta_group = adw::PreferencesGroup::builder()
        .title(&tr("Metadados do Caderno de Bordo"))
        .description(&tr(
            "Informações gerais para o cabeçalho do relatório operacional.",
        ))
        .build();

    let initial_meta = state_rc.borrow().runbook_data.metadata.clone();

    let rb_title_row = adw::EntryRow::builder()
        .title(&tr("Título do Procedimento"))
        .text(&initial_meta.title)
        .build();
    meta_group.add(&rb_title_row);

    let rb_author_row = adw::EntryRow::builder()
        .title(&tr("Operador / Autor"))
        .text(&initial_meta.author)
        .build();
    meta_group.add(&rb_author_row);

    let rb_objective_row = adw::EntryRow::builder()
        .title(&tr("Objetivo / Contexto"))
        .text(&initial_meta.objective)
        .build();
    meta_group.add(&rb_objective_row);

    let status_labels: Vec<String> = RUNBOOK_STATUS_OPTIONS.iter().map(|s| tr(s)).collect();
    let status_refs: Vec<&str> = status_labels.iter().map(String::as_str).collect();
    let status_model = gtk4::StringList::new(&status_refs);
    let rb_status_row = adw::ComboRow::builder()
        .title(&tr("Status do Procedimento"))
        .model(&status_model)
        .selected(0)
        .build();
    meta_group.add(&rb_status_row);

    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        rb_title_row.connect_changed(move |entry| {
            st_rc.borrow_mut().runbook_data.metadata.title = entry.text().trim().to_string();
            up();
        });
    }
    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        rb_author_row.connect_changed(move |entry| {
            st_rc.borrow_mut().runbook_data.metadata.author = entry.text().trim().to_string();
            up();
        });
    }
    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        rb_objective_row.connect_changed(move |entry| {
            st_rc.borrow_mut().runbook_data.metadata.objective = entry.text().trim().to_string();
            up();
        });
    }
    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        rb_status_row.connect_selected_notify(move |combo| {
            let idx = combo.selected() as usize;
            if let Some(&status_str) = RUNBOOK_STATUS_OPTIONS.get(idx) {
                st_rc.borrow_mut().runbook_data.metadata.status = tr(status_str);
                up();
            }
        });
    }

    runbook_page.append(&meta_group);

    // 2.2 Steps & Annotations List Group
    let initial_steps = state_rc.borrow().runbook_data.steps.clone();
    let steps_group = adw::PreferencesGroup::builder()
        .title(&tr("Etapas da Sessão e Anotações"))
        .description(&tr_fmt(
            "Selecione os comandos a incluir e adicione comentários técnicos explicativos para cada etapa. ({count} identificadas)",
            &[("count", &initial_steps.len().to_string())],
        ))
        .build();

    if initial_steps.is_empty() {
        let empty_row = adw::ActionRow::builder()
            .title(&tr("Nenhum comando registrado"))
            .subtitle(&tr(
                "Nenhum comando foi executado nesta sessão do terminal ainda.",
            ))
            .build();
        steps_group.add(&empty_row);
    } else {
        let steps_scroller = gtk4::ScrolledWindow::builder()
            .min_content_height(150)
            .max_content_height(260)
            .hexpand(true)
            .vexpand(false)
            .build();
        steps_scroller.add_css_class("card");

        let steps_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
        steps_box.set_margin_top(8);
        steps_box.set_margin_bottom(8);
        steps_box.set_margin_start(8);
        steps_box.set_margin_end(8);

        for (step_idx, step) in initial_steps.iter().enumerate() {
            let num = step_idx + 1;
            let status_str = step
                .exit_code
                .map(|c| format!("exit {c}"))
                .unwrap_or_else(|| tr("OK"));
            let dur_str = if step.duration_str.is_empty() {
                String::new()
            } else {
                format!(" • ⏱ {}", step.duration_str)
            };
            let time_str = if step.timestamp_str.is_empty() {
                String::new()
            } else {
                format!(" • 🕒 {}", step.timestamp_str)
            };
            let sub_info = format!("{status_str}{dur_str}{time_str}");

            let safe_cmd = glib::markup_escape_text(&step.command);
            let safe_sub = glib::markup_escape_text(&sub_info);

            let expander = adw::ExpanderRow::builder()
                .title(&format!("#{num}  {safe_cmd}"))
                .subtitle(safe_sub.as_str())
                .build();

            let check = gtk4::CheckButton::builder()
                .active(step.included)
                .build();
            {
                let st_rc = Rc::clone(&state_rc);
                let up = Rc::clone(&update_preview);
                check.connect_toggled(move |chk| {
                    if let Some(s) = st_rc.borrow_mut().runbook_data.steps.get_mut(step_idx) {
                        s.included = chk.is_active();
                    }
                    up();
                });
            }
            expander.add_prefix(&check);

            let entry_row = adw::EntryRow::builder()
                .title(&tr("Anotação / Comentário:"))
                .text(&step.annotation)
                .build();
            {
                let st_rc = Rc::clone(&state_rc);
                let up = Rc::clone(&update_preview);
                entry_row.connect_changed(move |entry| {
                    if let Some(s) = st_rc.borrow_mut().runbook_data.steps.get_mut(step_idx) {
                        s.annotation = entry.text().to_string();
                    }
                    up();
                });
            }
            expander.add_row(&entry_row);

            let out_snippet = step.output.trim();
            if !out_snippet.is_empty() {
                let first_line: String = out_snippet
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(90)
                    .collect();
                let lines_count = out_snippet.lines().count();
                let safe_first = glib::markup_escape_text(&first_line);
                let lines_str = tr_fmt("{count} linhas", &[("count", &lines_count.to_string())]);
                let out_row = adw::ActionRow::builder()
                    .title(&tr("Saída do Comando"))
                    .subtitle(&format!("{safe_first}... ({lines_str})"))
                    .build();
                expander.add_row(&out_row);
            }

            steps_box.append(&expander);
        }

        steps_scroller.set_child(Some(&steps_box));
        steps_group.add(&steps_scroller);
    }

    runbook_page.append(&steps_group);

    // 2.3 Conclusion Group
    let conclusion_group = adw::PreferencesGroup::builder()
        .title(&tr("Conclusão &amp; Lições Aprendidas"))
        .build();
    let rb_conclusion_row = adw::EntryRow::builder()
        .title(&tr("Notas Finais do Procedimento"))
        .text(&initial_meta.conclusion)
        .build();
    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        rb_conclusion_row.connect_changed(move |entry| {
            st_rc.borrow_mut().runbook_data.metadata.conclusion = entry.text().trim().to_string();
            up();
        });
    }
    conclusion_group.add(&rb_conclusion_row);
    runbook_page.append(&conclusion_group);

    // 2.4 Runbook Format & Export Options Group
    let rb_options_group = adw::PreferencesGroup::builder()
        .title(&tr("Formato do Caderno de Bordo"))
        .build();

    let mut first_rb_check: Option<gtk4::CheckButton> = None;
    for &(fmt_id, title, desc, _ext, _mime) in RUNBOOK_FORMAT_OPTIONS {
        let row = adw::ActionRow::builder()
            .title(&tr(title))
            .subtitle(&tr(desc))
            .build();
        let check = if let Some(ref leader) = first_rb_check {
            gtk4::CheckButton::builder()
                .group(leader)
                .active(fmt_id == "runbook_md")
                .build()
        } else {
            let c = gtk4::CheckButton::builder()
                .active(fmt_id == "runbook_md")
                .build();
            first_rb_check = Some(c.clone());
            c
        };

        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        let fmt_owned = fmt_id.to_string();
        check.connect_toggled(move |chk| {
            if chk.is_active() {
                st_rc.borrow_mut().selected_runbook_format = fmt_owned.clone();
                up();
            }
        });

        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));
        rb_options_group.add(&row);
    }

    let collapse_switch = adw::SwitchRow::builder()
        .title(&tr("Recolher saídas longas de comandos"))
        .subtitle(&tr(
            "Usa blocos retráteis (&lt;details&gt;) para saídas com mais de 15 linhas",
        ))
        .active(true)
        .build();
    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        collapse_switch.connect_active_notify(move |sw| {
            st_rc.borrow_mut().collapse_long_outputs = sw.is_active();
            up();
        });
    }
    rb_options_group.add(&collapse_switch);

    runbook_page.append(&rb_options_group);
    stack.add_titled(
        &runbook_page,
        Some("runbook"),
        &tr("📘 Caderno de Bordo (Runbook)"),
    );

    if start_in_runbook_mode {
        stack.set_visible_child_name("runbook");
    }

    {
        let st_rc = Rc::clone(&state_rc);
        let up = Rc::clone(&update_preview);
        stack.connect_visible_child_name_notify(move |stk| {
            let is_rb = stk.visible_child_name().as_deref() == Some("runbook");
            st_rc.borrow_mut().is_runbook_mode = is_rb;
            up();
        });
    }

    main_box.append(&stack);
    main_box.append(&preview_group);

    // 4. Action Buttons Bar (`Copiar para a Área de Transferência` & `Salvar como Arquivo...`)
    let actions_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
    actions_box.set_halign(gtk4::Align::End);
    actions_box.set_margin_top(6);

    let copy_btn = gtk4::Button::builder()
        .label(&tr("Copiar para a Área de Transferência"))
        .icon_name("edit-copy-symbolic")
        .build();
    let save_btn = gtk4::Button::builder()
        .label(&tr("Salvar como Arquivo..."))
        .icon_name("document-save-symbolic")
        .build();
    save_btn.add_css_class("suggested-action");

    {
        let st_rc = Rc::clone(&state_rc);
        let status = status_lbl.clone();
        let copy_btn_ref = copy_btn.clone();
        copy_btn.connect_clicked(move |_| {
            let (content, _, _) = st_rc.borrow().compute_active_output();
            let is_rb = st_rc.borrow().is_runbook_mode;
            if let Some(display) = gdk::Display::default() {
                display.clipboard().set_text(&content);
                copy_btn_ref.set_label(&tr("Copiado!"));
                copy_btn_ref.set_icon_name("object-select-symbolic");
                status.set_text(&if is_rb {
                    tr("Caderno de Bordo copiado para a área de transferência.")
                } else {
                    tr("Saída do terminal copiada para a área de transferência.")
                });

                let btn_restore = copy_btn_ref.clone();
                glib::timeout_add_local_once(Duration::from_millis(2000), move || {
                    btn_restore.set_label(&tr("Copiar para a Área de Transferência"));
                    btn_restore.set_icon_name("edit-copy-symbolic");
                });
            }
        });
    }

    {
        let st_rc = Rc::clone(&state_rc);
        let dlg_ref = dlg.clone();
        let status = status_lbl.clone();

        save_btn.connect_clicked(move |_| {
            let (content, ext, _mime) = st_rc.borrow().compute_active_output();
            let is_rb = st_rc.borrow().is_runbook_mode;

            let ts_str = glib::DateTime::now_local()
                .ok()
                .and_then(|dt| dt.format("%Y%m%d_%H%M%S").ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "20261105_120000".to_string());

            let prefix = if is_rb {
                "onyxsh_runbook"
            } else {
                "onyxsh_export"
            };
            let default_filename = format!("{prefix}_{ts_str}{ext}");
            let title = if is_rb {
                tr("Salvar Caderno de Bordo")
            } else {
                tr("Salvar Saída do Terminal")
            };

            let file_dialog = gtk4::FileDialog::builder()
                .title(&title)
                .initial_name(&default_filename)
                .build();

            let filters = gio::ListStore::new::<gtk4::FileFilter>();
            let active_filter = gtk4::FileFilter::new();
            active_filter.set_name(Some(&format!("*{ext}")));
            active_filter.add_pattern(&format!("*{ext}"));
            filters.append(&active_filter);

            let all_filter = gtk4::FileFilter::new();
            all_filter.set_name(Some(&tr("Todos os Arquivos")));
            all_filter.add_pattern("*");
            filters.append(&all_filter);

            file_dialog.set_filters(Some(&filters));
            file_dialog.set_default_filter(Some(&active_filter));

            let dlg_close = dlg_ref.clone();
            let status_inner = status.clone();
            file_dialog.save(
                Some(&dlg_ref),
                gio::Cancellable::NONE,
                move |res| {
                    if let Ok(file) = res {
                        if let Some(path) = file.path() {
                            match std::fs::write(&path, &content) {
                                Ok(_) => {
                                    status_inner.set_text(&tr_fmt(
                                        "Salvo com sucesso: {path}",
                                        &[("path", &path.display().to_string())],
                                    ));
                                    dlg_close.close();
                                }
                                Err(e) => {
                                    status_inner.set_text(&tr_fmt(
                                        "Falha ao salvar arquivo: {err}",
                                        &[("err", &e.to_string())],
                                    ));
                                }
                            }
                        }
                    }
                },
            );
        });
    }

    actions_box.append(&copy_btn);
    actions_box.append(&save_btn);
    main_box.append(&actions_box);

    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .child(&main_box)
        .build();

    toolbar_view.set_content(Some(&scroller));
    dlg.set_content(Some(&toolbar_view));

    update_preview();
    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_dialog_state_direct_and_runbook_modes() {
        let meta = ExportMetadata::default();
        let raw = "vagnarok@onyx:~$ sudo dnf update\nLast metadata expiration check: 0:05:12 ago.\nComplete!\nvagnarok@onyx:~$ uname -r\n6.12.0-onyx\n";
        let rb = build_runbook_data_from_text(raw, &meta);
        assert_eq!(rb.steps.len(), 2);
        assert_eq!(rb.steps[0].command, "sudo dnf update");
        assert_eq!(rb.steps[1].command, "uname -r");

        let mut state = ExportDialogState {
            is_runbook_mode: false,
            selection_only: false,
            selected_format: "md".to_string(),
            selected_runbook_format: "runbook_md".to_string(),
            collapse_long_outputs: true,
            full_buffer_text: raw.to_string(),
            selected_buffer_text: "uname -r\n6.12.0-onyx\n".to_string(),
            export_meta: meta,
            runbook_data: rb,
        };

        // 1. Direct export (.md, .txt, .log, .html, .cast)
        let (md_out, ext, mime) = state.compute_active_output();
        assert_eq!(ext, ".md");
        assert_eq!(mime, "text/markdown");
        assert!(md_out.contains(&tr("# 🖥️ OnyxSH Terminal Export")));
        assert!(md_out.contains("sudo dnf update"));

        state.selection_only = true;
        state.selected_format = "txt".to_string();
        let (txt_out, ext_txt, _) = state.compute_active_output();
        assert_eq!(ext_txt, ".txt");
        assert!(!txt_out.contains("sudo dnf update"));
        assert!(txt_out.contains("6.12.0-onyx"));

        // 2. Runbook mode (.md, .html, .log) with custom annotation and objective
        state.is_runbook_mode = true;
        state.runbook_data.metadata.title = "Atualização do Servidor".to_string();
        state.runbook_data.metadata.objective = "Aplicar patches de segurança".to_string();
        state.runbook_data.metadata.conclusion = "Sistema atualizado sem erros".to_string();
        state.runbook_data.steps[0].annotation = "Verificado pacotes DNF".to_string();

        let (rb_md, rb_ext, _) = state.compute_active_output();
        assert_eq!(rb_ext, ".md");
        assert!(rb_md.contains("# 📘 Atualização do Servidor"));
        assert!(rb_md.contains("Aplicar patches de segurança"));
        assert!(rb_md.contains("Verificado pacotes DNF"));
        assert!(rb_md.contains("Sistema atualizado sem erros"));

        state.selected_runbook_format = "runbook_html".to_string();
        let (rb_html, html_ext, _) = state.compute_active_output();
        assert_eq!(html_ext, ".html");
        assert!(rb_html.contains("<!DOCTYPE html>"));
        assert!(rb_html.contains("Atualização do Servidor"));

        state.selected_runbook_format = "runbook_log".to_string();
        let (rb_log, log_ext, _) = state.compute_active_output();
        assert_eq!(log_ext, ".log");
        assert!(rb_log.contains(&tr("[OBJETIVO / CONTEXTO]")));
        assert!(rb_log.contains(&tr_fmt("💬 Nota: {note}", &[("note", "Verificado pacotes DNF")])));

        // 3. Preview truncation
        let long_text = (1..=40).map(|i| format!("Line {i}")).collect::<Vec<_>>().join("\n");
        let snippet = build_preview_snippet(&long_text, 25);
        assert!(snippet.contains("Line 25"));
        assert!(!snippet.contains("Line 26"));
        assert!(snippet.contains(&tr("... (restante da saída truncado na pré-visualização)")));
    }

    #[test]
    fn test_build_runbook_steps_from_semantic_commands() {
        let cmds = vec![
            SemanticCommand {
                command_id: "cmd_1".to_string(),
                command_text: "systemctl status nginx".to_string(),
                prompt_row: 1,
                command_row: 1,
                output_start_row: 2,
                output_end_row: Some(4),
                duration_secs: Some(1.5),
                exit_code: Some(0),
                cwd: "/etc/nginx".to_string(),
            },
            SemanticCommand {
                command_id: "cmd_2".to_string(),
                command_text: "   ".to_string(),
                prompt_row: 5,
                command_row: 5,
                output_start_row: 6,
                output_end_row: Some(6),
                duration_secs: Some(0.1),
                exit_code: Some(0),
                cwd: "/etc/nginx".to_string(),
            },
        ];
        let steps = build_runbook_steps_from_semantic(&cmds, |start, end| {
            format!("rows {start}..={end} active (running)")
        });
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].command, "systemctl status nginx");
        assert_eq!(steps[0].output, "rows 2..=4 active (running)");
        assert_eq!(steps[0].exit_code, Some(0));
        assert_eq!(steps[0].duration_str, "1.5s");
        assert_eq!(steps[0].cwd, "/etc/nginx");
        assert!(steps[0].included);
    }
}
