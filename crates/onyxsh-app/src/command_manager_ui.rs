//! Phase 13: Command Manager, Dynamic Form Builder, Pinned Command Toolbar, and Broadcast Bar
//! (`src/onyxsh/data/command_manager_models.py`, `ui/dialogs/command_manager_dialog.py`,
//! `ui/widgets/form_widget_builder.py`, and `ui/window_ui.py`).

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::platform;
use onyxsh_terminal::broadcast::{BroadcastOutcome, BroadcastRouter, BroadcastScope};
use onyxsh_terminal::production_guard::ProductionGuard;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use vte4::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Password,
    Number,
    Switch,
    Dropdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    InsertOnly,
    InsertAndExecute,
    ShowForm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandFormField {
    pub id: String,
    pub label: String,
    pub field_type: FieldType,
    pub default_value: String,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandButton {
    pub id: String,
    pub name: String,
    pub description: String,
    pub command_template: String,
    pub category: String,
    pub icon_name: String,
    pub execution_mode: ExecutionMode,
    pub pinned: bool,
    #[serde(default)]
    pub form_fields: Vec<CommandFormField>,
}

/// Returns the path to `~/.config/onyxsh/command_buttons.json`.
pub fn command_buttons_file_path() -> PathBuf {
    platform::config_dir().join("command_buttons.json")
}

/// Built-in default DevOps, Network, Docker, Systemd, and Git command buttons.
pub fn default_command_buttons() -> Vec<CommandButton> {
    vec![
        CommandButton {
            id: "sys_ports".to_string(),
            name: "Portas em Escuta".to_string(),
            description: "Lista portas TCP/UDP abertas e processos associados (ss -tulnp)".to_string(),
            command_template: "ss -tulnp".to_string(),
            category: "Rede & Segurança".to_string(),
            icon_name: "network-wired-symbolic".to_string(),
            execution_mode: ExecutionMode::InsertAndExecute,
            pinned: true,
            form_fields: Vec::new(),
        },
        CommandButton {
            id: "docker_logs".to_string(),
            name: "Docker Logs".to_string(),
            description: "Acompanha logs de um container Docker com limite de linhas".to_string(),
            command_template: "docker logs --tail {{lines}} {{follow}} {{container}}".to_string(),
            category: "Docker & Containers".to_string(),
            icon_name: "utilities-terminal-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: true,
            form_fields: vec![
                CommandFormField {
                    id: "container".to_string(),
                    label: "Nome ou ID do Container".to_string(),
                    field_type: FieldType::Text,
                    default_value: "nginx-proxy".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "lines".to_string(),
                    label: "Quantidade de Linhas (--tail)".to_string(),
                    field_type: FieldType::Number,
                    default_value: "100".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "follow".to_string(),
                    label: "Seguir em tempo real (-f)".to_string(),
                    field_type: FieldType::Switch,
                    default_value: "-f".to_string(),
                    options: vec!["-f".to_string(), "".to_string()],
                },
            ],
        },
        CommandButton {
            id: "systemd_service".to_string(),
            name: "Systemd Control".to_string(),
            description: "Gerencia unidades do systemd (status, restart, logs)".to_string(),
            command_template: "sudo systemctl {{action}} {{service}}".to_string(),
            category: "Sistema & Serviços".to_string(),
            icon_name: "emblem-system-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: true,
            form_fields: vec![
                CommandFormField {
                    id: "action".to_string(),
                    label: "Ação".to_string(),
                    field_type: FieldType::Dropdown,
                    default_value: "status".to_string(),
                    options: vec![
                        "status".to_string(),
                        "restart".to_string(),
                        "reload".to_string(),
                        "stop".to_string(),
                        "start".to_string(),
                    ],
                },
                CommandFormField {
                    id: "service".to_string(),
                    label: "Nome do Serviço".to_string(),
                    field_type: FieldType::Text,
                    default_value: "sshd".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        CommandButton {
            id: "disk_usage".to_string(),
            name: "Uso de Disco".to_string(),
            description: "Exibe espaço em disco legível por partição (df -hT)".to_string(),
            command_template: "df -hT".to_string(),
            category: "Sistema & Serviços".to_string(),
            icon_name: "drive-harddisk-symbolic".to_string(),
            execution_mode: ExecutionMode::InsertAndExecute,
            pinned: true,
            form_fields: Vec::new(),
        },
        CommandButton {
            id: "git_status_short".to_string(),
            name: "Git Status & Branch".to_string(),
            description: "Mostra branch atual, commits recentes e arquivos modificados".to_string(),
            command_template: "git status -sb && git log --oneline -n 5".to_string(),
            category: "Git & Dev".to_string(),
            icon_name: "media-playlist-shuffle-symbolic".to_string(),
            execution_mode: ExecutionMode::InsertAndExecute,
            pinned: true,
            form_fields: Vec::new(),
        },
    ]
}

/// Loads command buttons from `~/.config/onyxsh/command_buttons.json`, falling back to defaults.
pub fn load_command_buttons() -> Vec<CommandButton> {
    let path = command_buttons_file_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(parsed) = serde_json::from_str::<Vec<CommandButton>>(&raw) {
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }
    default_command_buttons()
}

/// Persists command buttons to `~/.config/onyxsh/command_buttons.json`.
pub fn save_command_buttons(buttons: &[CommandButton]) -> std::io::Result<()> {
    let path = command_buttons_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(buttons).map_err(std::io::Error::other)?;
    fs::write(path, json)
}

/// Substitutes `{{key}}` and `{key}` placeholders in `template` using `values`.
pub fn render_command_template(template: &str, values: &BTreeMap<String, String>) -> String {
    let mut rendered = template.to_string();
    for (k, v) in values {
        let double_pat = format!("{{{{{k}}}}}");
        let single_pat = format!("{{{k}}}");
        rendered = rendered.replace(&double_pat, v);
        rendered = rendered.replace(&single_pat, v);
    }
    // Collapse multiple spaces created by empty optional flags
    rendered
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Color-codes a command's risk level using [`ProductionGuard`].
pub fn evaluate_command_safety_badge(cmd: &str) -> (&'static str, Option<String>) {
    let guard = ProductionGuard::new();
    if let Some(v) = guard.evaluate_command(cmd) {
        ("🔴 Alto Risco (ProductionGuard)", Some(v.risk_summary))
    } else {
        ("🟢 Comando Seguro", None)
    }
}

/// Opens the dynamic `CommandFormDialog` (`ui/widgets/form_widget_builder.py`) for a parameterized command.
pub fn show_command_form_dialog(
    parent: &adw::ApplicationWindow,
    cmd_btn: &CommandButton,
    dispatch_cb: Rc<dyn Fn(&str, bool, bool)>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(format!("Parâmetros — {}", cmd_btn.name))
        .default_width(520)
        .default_height(440)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(8);
    content.set_margin_bottom(14);

    let desc_lbl = gtk4::Label::new(Some(&cmd_btn.description));
    desc_lbl.add_css_class("dim-label");
    desc_lbl.set_halign(gtk4::Align::Start);
    content.append(&desc_lbl);

    let values_map: Rc<RefCell<BTreeMap<String, String>>> = Rc::new(RefCell::new(BTreeMap::new()));
    for field in &cmd_btn.form_fields {
        values_map
            .borrow_mut()
            .insert(field.id.clone(), field.default_value.clone());
    }

    // Live Preview Box
    let preview_frame = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    let preview_title = gtk4::Label::new(Some("Preview do Comando:"));
    preview_title.add_css_class("caption");
    preview_title.set_halign(gtk4::Align::Start);

    let preview_entry = gtk4::Entry::new();
    preview_entry.add_css_class("monospace");
    preview_entry.set_editable(true);

    let safety_lbl = gtk4::Label::new(None);
    safety_lbl.add_css_class("caption");
    safety_lbl.set_halign(gtk4::Align::Start);

    let update_preview: Rc<dyn Fn()> = {
        let tpl = cmd_btn.command_template.clone();
        let vmap = Rc::clone(&values_map);
        let pe = preview_entry.clone();
        let sl = safety_lbl.clone();
        Rc::new(move || {
            let rendered = render_command_template(&tpl, &vmap.borrow());
            pe.set_text(&rendered);
            let (badge, reason) = evaluate_command_safety_badge(&rendered);
            if let Some(r) = reason {
                sl.set_text(&format!("{badge}: {r}"));
            } else {
                sl.set_text(badge);
            }
        })
    };

    // Build dynamic fields
    for field in &cmd_btn.form_fields {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        let lbl = gtk4::Label::new(Some(&format!("{}:", field.label)));
        lbl.set_halign(gtk4::Align::Start);
        lbl.set_hexpand(true);
        row.append(&lbl);

        let fid = field.id.clone();
        let vmap = Rc::clone(&values_map);
        let up = Rc::clone(&update_preview);

        match field.field_type {
            FieldType::Text | FieldType::Number => {
                let entry = gtk4::Entry::builder().text(&field.default_value).build();
                entry.connect_changed(move |e| {
                    vmap.borrow_mut().insert(fid.clone(), e.text().to_string());
                    up();
                });
                row.append(&entry);
            }
            FieldType::Password => {
                let entry = gtk4::PasswordEntry::builder()
                    .show_peek_icon(true)
                    .build();
                entry.set_text(&field.default_value);
                entry.connect_changed(move |e| {
                    vmap.borrow_mut().insert(fid.clone(), e.text().to_string());
                    up();
                });
                row.append(&entry);
            }
            FieldType::Switch => {
                let on_val = field
                    .options
                    .first()
                    .cloned()
                    .unwrap_or_else(|| field.default_value.clone());
                let off_val = field.options.get(1).cloned().unwrap_or_default();
                let sw = gtk4::Switch::builder()
                    .active(!field.default_value.is_empty())
                    .valign(gtk4::Align::Center)
                    .build();
                sw.connect_state_set(move |_, active| {
                    let val = if active {
                        on_val.clone()
                    } else {
                        off_val.clone()
                    };
                    vmap.borrow_mut().insert(fid.clone(), val);
                    up();
                    gtk4::glib::Propagation::Proceed
                });
                row.append(&sw);
            }
            FieldType::Dropdown => {
                let str_refs: Vec<&str> = field.options.iter().map(String::as_str).collect();
                let slist = gtk4::StringList::new(&str_refs);
                let dd = gtk4::DropDown::new(Some(slist), gtk4::Expression::NONE);
                let opts = field.options.clone();
                dd.connect_selected_notify(move |d| {
                    let idx = d.selected() as usize;
                    if let Some(chosen) = opts.get(idx) {
                        vmap.borrow_mut().insert(fid.clone(), chosen.clone());
                        up();
                    }
                });
                row.append(&dd);
            }
        }

        content.append(&row);
    }

    update_preview();

    preview_frame.append(&preview_title);
    preview_frame.append(&preview_entry);
    preview_frame.append(&safety_lbl);
    content.append(&preview_frame);

    // Action Buttons: [Inserir Apenas] [Executar Agora] [Enviar p/ Todos (Broadcast)]
    let actions_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    actions_hbox.set_halign(gtk4::Align::End);

    let btn_insert = gtk4::Button::with_label("Inserir no Terminal");
    let btn_run = gtk4::Button::with_label("Executar Agora");
    btn_run.add_css_class("suggested-action");
    let btn_broadcast = gtk4::Button::with_label("Broadcast p/ Todos");

    actions_hbox.append(&btn_insert);
    actions_hbox.append(&btn_run);
    actions_hbox.append(&btn_broadcast);
    content.append(&actions_hbox);

    {
        let pe = preview_entry.clone();
        let cb = Rc::clone(&dispatch_cb);
        let d = dlg.clone();
        btn_insert.connect_clicked(move |_| {
            cb(&pe.text(), false, false);
            d.close();
        });
    }
    {
        let pe = preview_entry.clone();
        let cb = Rc::clone(&dispatch_cb);
        let d = dlg.clone();
        btn_run.connect_clicked(move |_| {
            cb(&pe.text(), true, false);
            d.close();
        });
    }
    {
        let pe = preview_entry;
        let cb = dispatch_cb;
        let d = dlg.clone();
        btn_broadcast.connect_clicked(move |_| {
            cb(&pe.text(), true, true);
            d.close();
        });
    }

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Opens the Command Manager Dialog (`ui/dialogs/command_manager_dialog.py`) to browse, run, pin/unpin, or create commands.
pub fn show_command_manager_dialog(
    parent: &adw::ApplicationWindow,
    buttons_rc: Rc<RefCell<Vec<CommandButton>>>,
    rebuild_toolbar_cb: Rc<dyn Fn()>,
    dispatch_cb: Rc<dyn Fn(&str, bool, bool)>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Gerenciador de Comandos & Formulários — OnyxSH")
        .default_width(680)
        .default_height(500)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    let hb = adw::HeaderBar::new();

    let btn_new_cmd = gtk4::Button::builder()
        .icon_name("list-add-symbolic")
        .label("Novo Comando")
        .build();
    btn_new_cmd.add_css_class("suggested-action");
    hb.pack_start(&btn_new_cmd);
    vbox.append(&hb);

    let filter_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    filter_bar.set_margin_start(14);
    filter_bar.set_margin_end(14);
    filter_bar.set_margin_top(6);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Buscar comando por nome, categoria ou template...")
        .hexpand(true)
        .build();
    filter_bar.append(&search_entry);
    vbox.append(&filter_bar);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);

    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_box.set_margin_start(14);
    list_box.set_margin_end(14);
    list_box.set_margin_top(6);
    list_box.set_margin_bottom(14);
    scroll.set_child(Some(&list_box));
    vbox.append(&scroll);

    let populate_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let populate: Rc<dyn Fn()> = {
        let lb = list_box.clone();
        let se = search_entry.clone();
        let cmds_rc = Rc::clone(&buttons_rc);
        let rebuild_tb = Rc::clone(&rebuild_toolbar_cb);
        let disp = Rc::clone(&dispatch_cb);
        let win_ref = parent.clone();
        let pop_holder = Rc::clone(&populate_holder);

        Rc::new(move || {
            while let Some(c) = lb.first_child() {
                lb.remove(&c);
            }
            let q = se.text().trim().to_lowercase();
            let list = cmds_rc.borrow().clone();

            for (idx, cmd) in list.iter().enumerate() {
                if !q.is_empty()
                    && !cmd.name.to_lowercase().contains(&q)
                    && !cmd.category.to_lowercase().contains(&q)
                    && !cmd.command_template.to_lowercase().contains(&q)
                {
                    continue;
                }

                let row_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
                row_hbox.set_margin_start(10);
                row_hbox.set_margin_end(10);
                row_hbox.set_margin_top(8);
                row_hbox.set_margin_bottom(8);

                let icon = gtk4::Image::from_icon_name(&cmd.icon_name);
                row_hbox.append(&icon);

                let text_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                text_vbox.set_hexpand(true);

                let title_lbl =
                    gtk4::Label::new(Some(&format!("{}  [{}]", cmd.name, cmd.category)));
                title_lbl.add_css_class("heading");
                title_lbl.set_halign(gtk4::Align::Start);

                let tpl_lbl = gtk4::Label::new(Some(&cmd.command_template));
                tpl_lbl.add_css_class("monospace");
                tpl_lbl.add_css_class("caption");
                tpl_lbl.add_css_class("dim-label");
                tpl_lbl.set_halign(gtk4::Align::Start);

                text_vbox.append(&title_lbl);
                text_vbox.append(&tpl_lbl);
                row_hbox.append(&text_vbox);

                // Pin/Unpin toggle button
                let pin_btn = gtk4::ToggleButton::builder()
                    .icon_name("view-pin-symbolic")
                    .tooltip_text("Fixar / Desafixar na Command Toolbar")
                    .active(cmd.pinned)
                    .build();
                {
                    let cmds_mut = Rc::clone(&cmds_rc);
                    let r_tb = Rc::clone(&rebuild_tb);
                    pin_btn.connect_toggled(move |btn| {
                        if let Some(item) = cmds_mut.borrow_mut().get_mut(idx) {
                            item.pinned = btn.is_active();
                        }
                        let _ = save_command_buttons(&cmds_mut.borrow());
                        r_tb();
                    });
                }
                row_hbox.append(&pin_btn);

                // Run / Open Form button
                let run_btn = gtk4::Button::with_label(if cmd.form_fields.is_empty() {
                    "Executar"
                } else {
                    "Formulário..."
                });
                run_btn.add_css_class("suggested-action");
                {
                    let cmd_clone = cmd.clone();
                    let w = win_ref.clone();
                    let d = Rc::clone(&disp);
                    run_btn.connect_clicked(move |_| {
                        if !cmd_clone.form_fields.is_empty()
                            || cmd_clone.execution_mode == ExecutionMode::ShowForm
                        {
                            show_command_form_dialog(&w, &cmd_clone, Rc::clone(&d));
                        } else {
                            let exec = cmd_clone.execution_mode == ExecutionMode::InsertAndExecute;
                            d(&cmd_clone.command_template, exec, false);
                        }
                    });
                }
                row_hbox.append(&run_btn);

                // Delete button
                let del_btn = gtk4::Button::builder()
                    .icon_name("user-trash-symbolic")
                    .tooltip_text("Remover Comando")
                    .build();
                del_btn.add_css_class("flat");
                {
                    let cmds_mut = Rc::clone(&cmds_rc);
                    let r_tb = Rc::clone(&rebuild_tb);
                    let ph = Rc::clone(&pop_holder);
                    del_btn.connect_clicked(move |_| {
                        let mut guard = cmds_mut.borrow_mut();
                        if idx < guard.len() {
                            guard.remove(idx);
                        }
                        let _ = save_command_buttons(&guard);
                        drop(guard);
                        r_tb();
                        if let Some(p) = ph.borrow().as_ref() {
                            p();
                        }
                    });
                }
                row_hbox.append(&del_btn);

                lb.append(&row_hbox);
            }
        })
    };

    *populate_holder.borrow_mut() = Some(Rc::clone(&populate));
    populate();

    {
        let pop = Rc::clone(&populate);
        search_entry.connect_search_changed(move |_| pop());
    }

    // Create New Command modal
    {
        let win_ref = parent.clone();
        let cmds_rc = Rc::clone(&buttons_rc);
        let rebuild_tb = Rc::clone(&rebuild_toolbar_cb);
        let pop = Rc::clone(&populate);

        btn_new_cmd.connect_clicked(move |_| {
            let add_dlg = adw::Window::builder()
                .transient_for(&win_ref)
                .modal(true)
                .title("Novo Botão de Comando")
                .default_width(440)
                .default_height(320)
                .build();

            let avbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
            avbox.append(&adw::HeaderBar::new());

            let form = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
            form.set_margin_start(16);
            form.set_margin_end(16);
            form.set_margin_top(8);
            form.set_margin_bottom(14);

            let e_name = gtk4::Entry::builder()
                .placeholder_text("Nome do botão (ex: Reiniciar Nginx)")
                .build();
            let e_cat = gtk4::Entry::builder()
                .placeholder_text("Categoria (ex: Produção)")
                .text("Custom")
                .build();
            let e_tpl = gtk4::Entry::builder()
                .placeholder_text("Comando (ex: sudo systemctl restart nginx)")
                .build();
            e_tpl.add_css_class("monospace");

            let chk_pin = gtk4::CheckButton::with_label("Fixar na Command Toolbar superior");
            chk_pin.set_active(true);

            let save_btn = gtk4::Button::with_label("Salvar Comando");
            save_btn.add_css_class("suggested-action");

            form.append(&e_name);
            form.append(&e_cat);
            form.append(&e_tpl);
            form.append(&chk_pin);
            form.append(&save_btn);

            let cmds_inner = Rc::clone(&cmds_rc);
            let r_tb = Rc::clone(&rebuild_tb);
            let p_fn = Rc::clone(&pop);
            let d_close = add_dlg.clone();

            save_btn.connect_clicked(move |_| {
                let name = e_name.text().trim().to_string();
                let tpl = e_tpl.text().trim().to_string();
                if name.is_empty() || tpl.is_empty() {
                    return;
                }
                let mut list = cmds_inner.borrow_mut();
                let next_id = format!("custom_{}", list.len() + 1);
                list.push(CommandButton {
                    id: next_id,
                    name,
                    description: "Comando personalizado".to_string(),
                    command_template: tpl,
                    category: e_cat.text().trim().to_string(),
                    icon_name: "utilities-terminal-symbolic".to_string(),
                    execution_mode: ExecutionMode::InsertAndExecute,
                    pinned: chk_pin.is_active(),
                    form_fields: Vec::new(),
                });
                let _ = save_command_buttons(&list);
                drop(list);
                r_tb();
                p_fn();
                d_close.close();
            });

            avbox.append(&form);
            add_dlg.set_content(Some(&avbox));
            add_dlg.present();
        });
    }

    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Builds the Pinned Command Toolbar + Multi-Terminal Broadcast Bar (`crates/onyxsh-app/src/command_manager_ui.rs`).
/// Returns `(toolbar_vbox, rebuild_pinned_fn, toggle_broadcast_btn)`.
pub fn build_command_and_broadcast_bars(
    parent_window: &adw::ApplicationWindow,
    buttons_rc: Rc<RefCell<Vec<CommandButton>>>,
    terminals_rc: Rc<RefCell<Vec<vte4::Terminal>>>,
    active_term_cb: Rc<dyn Fn() -> Option<vte4::Terminal>>,
    status_lbl: &gtk4::Label,
) -> (gtk4::Box, Rc<dyn Fn()>, gtk4::ToggleButton) {
    let wrapper_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // Dispatcher closure: `(cmd_str, execute_newline, broadcast_all)`
    let dispatch_cb: Rc<dyn Fn(&str, bool, bool)> = {
        let terms_all = Rc::clone(&terminals_rc);
        let active_fn = Rc::clone(&active_term_cb);
        let status = status_lbl.clone();

        Rc::new(move |cmd_str: &str, execute_newline: bool, broadcast_all: bool| {
            let payload = if execute_newline {
                format!("{cmd_str}\n")
            } else {
                cmd_str.to_string()
            };

            if broadcast_all {
                let router = BroadcastRouter::new(BroadcastScope::AllTerminals, true);
                let list = terms_all.borrow();
                let ids: Vec<u64> = (0..list.len() as u64).collect();
                match router.dispatch_command(0, &ids, &ids, cmd_str) {
                    BroadcastOutcome::BlockedByGuard { violation, .. } => {
                        status.set_text(&format!(
                            "🛑 Broadcast bloqueado pelo ProductionGuard: {}",
                            violation.risk_summary
                        ));
                    }
                    BroadcastOutcome::FanOut { targets, .. } => {
                        for idx in &targets {
                            if let Some(t) = list.get(*idx as usize) {
                                t.feed_child(payload.as_bytes());
                            }
                        }
                        status.set_text(&format!(
                            "📡 Comando enviado em Broadcast para {} terminal(is).",
                            targets.len()
                        ));
                    }
                    BroadcastOutcome::SingleTarget(_) => {}
                }
            } else if let Some(term) = active_fn() {
                let guard = ProductionGuard::new();
                if let Some(violation) = guard.evaluate_command(cmd_str) {
                    status.set_text(&format!(
                        "⚠️ Alerta ProductionGuard ({:?}): {}",
                        violation.severity, violation.risk_summary
                    ));
                }
                term.feed_child(payload.as_bytes());
                status.set_text(&format!("Comando enviado: {cmd_str}"));
            }
        })
    };

    // ── 1. Pinned Command Toolbar ─────────────────────────────────────────────
    let cmd_toolbar_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    cmd_toolbar_hbox.set_margin_start(10);
    cmd_toolbar_hbox.set_margin_end(10);
    cmd_toolbar_hbox.set_margin_top(3);
    cmd_toolbar_hbox.set_margin_bottom(3);

    let pinned_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .hexpand(true)
        .build();
    let pinned_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    pinned_scroll.set_child(Some(&pinned_hbox));
    cmd_toolbar_hbox.append(&pinned_scroll);

    let btn_toggle_broadcast = gtk4::ToggleButton::builder()
        .icon_name("network-transmit-receive-symbolic")
        .label("Broadcast")
        .tooltip_text("Barra de Execução Simultânea (Multi-Terminal Broadcast)")
        .build();
    btn_toggle_broadcast.add_css_class("flat");
    cmd_toolbar_hbox.append(&btn_toggle_broadcast);

    wrapper_vbox.append(&cmd_toolbar_hbox);

    // ── 2. Multi-Terminal Broadcast Bar (`broadcast.rs`) ──────────────────────
    let broadcast_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    broadcast_bar.set_margin_start(10);
    broadcast_bar.set_margin_end(10);
    broadcast_bar.set_margin_top(4);
    broadcast_bar.set_margin_bottom(4);
    broadcast_bar.set_visible(false);

    let bc_badge = gtk4::Label::new(Some("📡 Broadcast (🛡️ ProductionGuard Ativo):"));
    bc_badge.add_css_class("heading");
    broadcast_bar.append(&bc_badge);

    let bc_entry = gtk4::Entry::builder()
        .placeholder_text("Digite um comando para enviar a TODOS os terminais abertos...")
        .hexpand(true)
        .build();
    bc_entry.add_css_class("monospace");
    broadcast_bar.append(&bc_entry);

    let bc_send_btn = gtk4::Button::with_label("Enviar p/ Todos");
    bc_send_btn.add_css_class("suggested-action");
    broadcast_bar.append(&bc_send_btn);

    wrapper_vbox.append(&broadcast_bar);

    {
        let bb = broadcast_bar.clone();
        let be = bc_entry.clone();
        btn_toggle_broadcast.connect_toggled(move |btn| {
            let active = btn.is_active();
            bb.set_visible(active);
            if active {
                be.grab_focus();
            }
        });
    }

    {
        let disp = Rc::clone(&dispatch_cb);
        let be = bc_entry.clone();
        let send_fn = move || {
            let txt = be.text().trim().to_string();
            if !txt.is_empty() {
                disp(&txt, true, true);
                be.set_text("");
            }
        };
        let send_rc = Rc::new(send_fn);
        let s1 = Rc::clone(&send_rc);
        bc_send_btn.connect_clicked(move |_| s1());
        let s2 = Rc::clone(&send_rc);
        bc_entry.connect_activate(move |_| s2());
    }

    // ── 3. Rebuild Pinned Buttons Closure ─────────────────────────────────────
    let rebuild_pinned: Rc<dyn Fn()> = {
        let phbox = pinned_hbox.clone();
        let cmds_rc = Rc::clone(&buttons_rc);
        let win_ref = parent_window.clone();
        let disp = Rc::clone(&dispatch_cb);

        Rc::new(move || {
            while let Some(c) = phbox.first_child() {
                phbox.remove(&c);
            }
            let list = cmds_rc.borrow().clone();
            for cmd in list.into_iter().filter(|b| b.pinned) {
                let btn = gtk4::Button::new();
                btn.add_css_class("flat");
                btn.set_tooltip_text(Some(&format!(
                    "{} — {}\n$ {}",
                    cmd.name, cmd.description, cmd.command_template
                )));

                let b_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
                let icon = gtk4::Image::from_icon_name(&cmd.icon_name);
                let lbl = gtk4::Label::new(Some(&cmd.name));
                lbl.add_css_class("caption");
                b_hbox.append(&icon);
                b_hbox.append(&lbl);
                btn.set_child(Some(&b_hbox));

                let cmd_clone = cmd.clone();
                let w = win_ref.clone();
                let d = Rc::clone(&disp);
                btn.connect_clicked(move |_| {
                    if !cmd_clone.form_fields.is_empty()
                        || cmd_clone.execution_mode == ExecutionMode::ShowForm
                    {
                        show_command_form_dialog(&w, &cmd_clone, Rc::clone(&d));
                    } else {
                        let exec = cmd_clone.execution_mode == ExecutionMode::InsertAndExecute;
                        d(&cmd_clone.command_template, exec, false);
                    }
                });

                phbox.append(&btn);
            }
        })
    };

    rebuild_pinned();

    (wrapper_vbox, rebuild_pinned, btn_toggle_broadcast)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_command_template_and_safety_badge() {
        let mut vals = BTreeMap::new();
        vals.insert("container".to_string(), "redis-cache".to_string());
        vals.insert("lines".to_string(), "50".to_string());
        vals.insert("follow".to_string(), "-f".to_string());

        let rendered = render_command_template(
            "docker logs --tail {{lines}} {{follow}} {{container}}",
            &vals,
        );
        assert_eq!(rendered, "docker logs --tail 50 -f redis-cache");

        let (safe_badge, reason) = evaluate_command_safety_badge(&rendered);
        assert!(safe_badge.contains("Seguro"));
        assert!(reason.is_none());

        let (danger_badge, danger_reason) = evaluate_command_safety_badge("sudo rm -rf /");
        assert!(danger_badge.contains("Alto Risco"));
        assert!(danger_reason.is_some());
    }
}
