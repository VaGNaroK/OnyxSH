//! Phase 13: Command Manager, Dynamic Form Builder, Pinned Command Toolbar, and Broadcast Bar
//! (`src/onyxsh/data/command_manager_models.py`, `ui/dialogs/command_manager_dialog.py`,
//! `ui/widgets/form_widget_builder.py`, and `ui/window_ui.py`).

use gtk4::gdk;
use gtk4::pango;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::i18n::{tr, tr_fmt};
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
    pub hidden: bool,
    #[serde(default)]
    pub is_builtin: bool,
    #[serde(default)]
    pub form_fields: Vec<CommandFormField>,
}

/// Returns the path to `~/.config/onyxsh/command_buttons.json`.
pub fn command_buttons_file_path() -> PathBuf {
    platform::config_dir().join("command_buttons.json")
}

/// Built-in default command buttons matching `src/onyxsh/data/command_manager_models.py` and `CORRIGIR/gerenciador_comandos.png`.
pub fn default_command_buttons() -> Vec<CommandButton> {
    vec![
        // 1. APT
        CommandButton {
            id: "builtin_apt".to_string(),
            name: "APT".to_string(),
            description: "Operações com gerenciador de pacotes Debian/Ubuntu".to_string(),
            command_template: "sudo apt {{action}} {{package}}".to_string(),
            category: "Sistema".to_string(),
            icon_name: "system-software-install-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "action".to_string(),
                    label: "Ação".to_string(),
                    field_type: FieldType::Dropdown,
                    default_value: "install".to_string(),
                    options: vec![
                        "install".to_string(),
                        "remove".to_string(),
                        "purge".to_string(),
                        "update".to_string(),
                        "upgrade -y".to_string(),
                        "autoremove".to_string(),
                        "search".to_string(),
                        "show".to_string(),
                        "clean".to_string(),
                        "autoclean".to_string(),
                    ],
                },
                CommandFormField {
                    id: "package".to_string(),
                    label: "Pacote".to_string(),
                    field_type: FieldType::Text,
                    default_value: "".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 2. Compactar
        CommandButton {
            id: "builtin_compress".to_string(),
            name: "Compactar".to_string(),
            description: "Cria arquivos compactados a partir de arquivos e pastas".to_string(),
            command_template: "tar -cJvf {{output}} {{input}}".to_string(),
            category: "Operações de Arquivo".to_string(),
            icon_name: "package-x-generic-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "format".to_string(),
                    label: "Formato".to_string(),
                    field_type: FieldType::Dropdown,
                    default_value: "tar.xz".to_string(),
                    options: vec![
                        "tar.xz".to_string(),
                        "tar.gz".to_string(),
                        "tar.bz2".to_string(),
                        "tar.zst".to_string(),
                        "zip".to_string(),
                    ],
                },
                CommandFormField {
                    id: "input".to_string(),
                    label: "Arquivos/Pastas de Origem".to_string(),
                    field_type: FieldType::Text,
                    default_value: "".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "output".to_string(),
                    label: "Nome do Arquivo de Destino".to_string(),
                    field_type: FieldType::Text,
                    default_value: "archive.tar.xz".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 3. Criar Nova Branch no Git
        CommandButton {
            id: "snippet_git_new_branch".to_string(),
            name: "Criar Nova Branch no Git".to_string(),
            description: "Cria e muda para uma nova branch local no repositório Git".to_string(),
            command_template: "git checkout -b {{new_branch}}".to_string(),
            category: "Git & Version Control".to_string(),
            icon_name: "edit-copy-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "new_branch".to_string(),
                    label: "Nome da Nova Branch".to_string(),
                    field_type: FieldType::Text,
                    default_value: "feature/nova-funcionalidade".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 4. DNF
        CommandButton {
            id: "builtin_dnf".to_string(),
            name: "DNF".to_string(),
            description: "Operações com gerenciador de pacotes Fedora/RHEL".to_string(),
            command_template: "sudo dnf {{action}} {{package}}".to_string(),
            category: "Sistema".to_string(),
            icon_name: "system-software-install-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "action".to_string(),
                    label: "Ação".to_string(),
                    field_type: FieldType::Dropdown,
                    default_value: "install".to_string(),
                    options: vec![
                        "install".to_string(),
                        "remove".to_string(),
                        "upgrade -y".to_string(),
                        "check-update".to_string(),
                        "autoremove".to_string(),
                        "search".to_string(),
                        "info".to_string(),
                        "clean all".to_string(),
                    ],
                },
                CommandFormField {
                    id: "package".to_string(),
                    label: "Pacote".to_string(),
                    field_type: FieldType::Text,
                    default_value: "".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 5. Docker Logs
        CommandButton {
            id: "snippet_docker_logs".to_string(),
            name: "Docker Logs".to_string(),
            description: "Acompanha logs de um container Docker em tempo real".to_string(),
            command_template: "docker logs -f {{container}} --tail {{lines}}".to_string(),
            category: "Containers & Docker".to_string(),
            icon_name: "utilities-terminal-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: true,
            hidden: false,
            is_builtin: true,
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
            ],
        },
        // 6. Encontrar Arquivos
        CommandButton {
            id: "builtin_find".to_string(),
            name: "Encontrar Arquivos".to_string(),
            description: "Busca arquivos e diretórios com filtros avançados".to_string(),
            command_template: "find {{path}} {{name_pattern}} {{file_type}}".to_string(),
            category: "Operações de Arquivo".to_string(),
            icon_name: "system-search-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "path".to_string(),
                    label: "Diretório Inicial".to_string(),
                    field_type: FieldType::Text,
                    default_value: ".".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "name_pattern".to_string(),
                    label: "Padrão de Nome (ex: *.log)".to_string(),
                    field_type: FieldType::Text,
                    default_value: "*".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "file_type".to_string(),
                    label: "Tipo de Arquivo".to_string(),
                    field_type: FieldType::Dropdown,
                    default_value: "".to_string(),
                    options: vec![
                        "".to_string(),
                        "-type f".to_string(),
                        "-type d".to_string(),
                        "-type l".to_string(),
                    ],
                },
            ],
        },
        // 7. Extrair
        CommandButton {
            id: "builtin_extract".to_string(),
            name: "Extrair".to_string(),
            description: "Extrai arquivos de arquivos compactados".to_string(),
            command_template: "tar -xvf {{input}}".to_string(),
            category: "Operações de Arquivo".to_string(),
            icon_name: "package-x-generic-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "input".to_string(),
                    label: "Arquivo Compactado".to_string(),
                    field_type: FieldType::Text,
                    default_value: "archive.tar.xz".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 8. Flatpak
        CommandButton {
            id: "builtin_flatpak".to_string(),
            name: "Flatpak".to_string(),
            description: "Gerenciamento de aplicativos e runtimes Flatpak".to_string(),
            command_template: "flatpak {{action}} {{package}}".to_string(),
            category: "Sistema".to_string(),
            icon_name: "system-software-install-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "action".to_string(),
                    label: "Ação".to_string(),
                    field_type: FieldType::Dropdown,
                    default_value: "install".to_string(),
                    options: vec![
                        "install".to_string(),
                        "uninstall".to_string(),
                        "update".to_string(),
                        "list --app".to_string(),
                        "search".to_string(),
                        "info".to_string(),
                        "run".to_string(),
                    ],
                },
                CommandFormField {
                    id: "package".to_string(),
                    label: "Aplicativo ou ID".to_string(),
                    field_type: FieldType::Text,
                    default_value: "".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 9. Git Commit
        CommandButton {
            id: "snippet_git_commit".to_string(),
            name: "Git Commit".to_string(),
            description: "Faz commit das alterações no Git com mensagem descritiva".to_string(),
            command_template: "git commit -m \"{{message}}\"".to_string(),
            category: "Git & Version Control".to_string(),
            icon_name: "document-save-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "message".to_string(),
                    label: "Mensagem do Commit".to_string(),
                    field_type: FieldType::Text,
                    default_value: "feat: ".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 10. Journalctl
        CommandButton {
            id: "builtin_journalctl".to_string(),
            name: "Journalctl".to_string(),
            description: "Visualiza logs do sistema com journalctl".to_string(),
            command_template: "journalctl {{unit}} {{follow}} -n {{lines}}".to_string(),
            category: "Sistema".to_string(),
            icon_name: "document-open-recent-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "unit".to_string(),
                    label: "Unidade / Serviço (opcional)".to_string(),
                    field_type: FieldType::Text,
                    default_value: "".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "follow".to_string(),
                    label: "Seguir em tempo real (-f)".to_string(),
                    field_type: FieldType::Switch,
                    default_value: "-f".to_string(),
                    options: vec!["-f".to_string(), "".to_string()],
                },
                CommandFormField {
                    id: "lines".to_string(),
                    label: "Quantidade de Linhas (-n)".to_string(),
                    field_type: FieldType::Number,
                    default_value: "100".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 11. Listar Arquivos
        CommandButton {
            id: "builtin_ls".to_string(),
            name: "Listar Arquivos".to_string(),
            description: "Lista conteúdos do diretório com detalhes (ls -lah)".to_string(),
            command_template: "ls -lah ".to_string(),
            category: "Operações de Arquivo".to_string(),
            icon_name: "folder-open-symbolic".to_string(),
            execution_mode: ExecutionMode::InsertAndExecute,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: Vec::new(),
        },
        // 12. Logs de Serviço do Systemd
        CommandButton {
            id: "snippet_journalctl_service".to_string(),
            name: "Logs de Serviço do Systemd".to_string(),
            description: "Acompanha logs de uma unidade do systemd em tempo real".to_string(),
            command_template: "journalctl -u {{service}} -f -n {{lines}}".to_string(),
            category: "Sistema".to_string(),
            icon_name: "system-search-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "service".to_string(),
                    label: "Nome do Serviço".to_string(),
                    field_type: FieldType::Text,
                    default_value: "nginx".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "lines".to_string(),
                    label: "Quantidade de Linhas (-n)".to_string(),
                    field_type: FieldType::Number,
                    default_value: "50".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 13. Pacman
        CommandButton {
            id: "builtin_pacman".to_string(),
            name: "Pacman".to_string(),
            description: "Operações com gerenciador de pacotes Arch Linux".to_string(),
            command_template: "sudo pacman {{action}} {{package}}".to_string(),
            category: "Sistema".to_string(),
            icon_name: "system-software-install-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "action".to_string(),
                    label: "Ação".to_string(),
                    field_type: FieldType::Dropdown,
                    default_value: "-Syu".to_string(),
                    options: vec![
                        "-Syu".to_string(),
                        "-S".to_string(),
                        "-R".to_string(),
                        "-Rs".to_string(),
                        "-Ss".to_string(),
                        "-Qi".to_string(),
                        "-Sc".to_string(),
                    ],
                },
                CommandFormField {
                    id: "package".to_string(),
                    label: "Pacote".to_string(),
                    field_type: FieldType::Text,
                    default_value: "".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 14. Shell Interativo no Docker (Exec)
        CommandButton {
            id: "snippet_docker_exec".to_string(),
            name: "Shell Interativo no Docker (Exec)".to_string(),
            description: "Abre um shell interativo dentro de um container Docker em execução".to_string(),
            command_template: "docker exec -it {{container}} {{shell}}".to_string(),
            category: "Containers & Docker".to_string(),
            icon_name: "utilities-terminal-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "container".to_string(),
                    label: "Nome ou ID do Container".to_string(),
                    field_type: FieldType::Text,
                    default_value: "nginx-proxy".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "shell".to_string(),
                    label: "Shell a Executar".to_string(),
                    field_type: FieldType::Text,
                    default_value: "bash".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 15. Sincronizar Arquivos com Rsync
        CommandButton {
            id: "snippet_rsync_remote".to_string(),
            name: "Sincronizar Arquivos com Rsync".to_string(),
            description: "Sincroniza arquivos locais para host remoto preservando atributos".to_string(),
            command_template: "rsync -avzP {{src}} {{user}}@{{host}}:{{dest}}".to_string(),
            category: "Network & SSH".to_string(),
            icon_name: "folder-remote-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: false,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "src".to_string(),
                    label: "Origem (Local)".to_string(),
                    field_type: FieldType::Text,
                    default_value: ".".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "user".to_string(),
                    label: "Usuário Remoto".to_string(),
                    field_type: FieldType::Text,
                    default_value: "root".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "host".to_string(),
                    label: "Host Remoto (IP / Domínio)".to_string(),
                    field_type: FieldType::Text,
                    default_value: "192.168.1.100".to_string(),
                    options: Vec::new(),
                },
                CommandFormField {
                    id: "dest".to_string(),
                    label: "Destino Remoto".to_string(),
                    field_type: FieldType::Text,
                    default_value: "/tmp/".to_string(),
                    options: Vec::new(),
                },
            ],
        },
        // 16. Systemctl
        CommandButton {
            id: "builtin_systemctl".to_string(),
            name: "Systemctl".to_string(),
            description: "Gerencia serviços do sistema com systemctl".to_string(),
            command_template: "sudo systemctl {{action}} {{service}}".to_string(),
            category: "Sistema".to_string(),
            icon_name: "preferences-system-symbolic".to_string(),
            execution_mode: ExecutionMode::ShowForm,
            pinned: true,
            hidden: false,
            is_builtin: true,
            form_fields: vec![
                CommandFormField {
                    id: "action".to_string(),
                    label: "Ação".to_string(),
                    field_type: FieldType::Dropdown,
                    default_value: "status".to_string(),
                    options: vec![
                        "status".to_string(),
                        "start".to_string(),
                        "stop".to_string(),
                        "restart".to_string(),
                        "reload".to_string(),
                        "enable".to_string(),
                        "disable".to_string(),
                        "is-active".to_string(),
                    ],
                },
                CommandFormField {
                    id: "service".to_string(),
                    label: "Nome do Serviço".to_string(),
                    field_type: FieldType::Text,
                    default_value: "nginx".to_string(),
                    options: Vec::new(),
                },
            ],
        },
    ]
}

/// Loads command buttons from `~/.config/onyxsh/command_buttons.json`, falling back to defaults.
pub fn load_command_buttons() -> Vec<CommandButton> {
    let path = command_buttons_file_path();
    let defaults = default_command_buttons();
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(parsed) = serde_json::from_str::<Vec<CommandButton>>(&raw) {
            if !parsed.is_empty() {
                let existing_ids: std::collections::HashSet<String> =
                    parsed.iter().map(|c| c.id.clone()).collect();
                let mut combined = parsed;
                for def in defaults {
                    if !existing_ids.contains(&def.id) {
                        combined.push(def);
                    }
                }
                return combined;
            }
        }
    }
    defaults
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
pub fn evaluate_command_safety_badge(cmd: &str) -> (String, Option<String>) {
    let guard = ProductionGuard::new();
    if let Some(v) = guard.evaluate_command(cmd) {
        (tr("🔴 Alto Risco (ProductionGuard)"), Some(v.risk_summary))
    } else {
        (tr("🟢 Comando Seguro"), None)
    }
}

/// Opens the dynamic `CommandFormDialog` (`ui/widgets/form_widget_builder.py`) for a parameterized command.
pub fn show_command_form_dialog(
    parent: &adw::ApplicationWindow,
    cmd_btn: &CommandButton,
    dispatch_cb: Rc<dyn Fn(&str, bool, bool)>,
) {
    let translated_name = tr(&cmd_btn.name);
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr_fmt(
            "Parâmetros — {name}",
            &[("name", &translated_name)],
        ))
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

    let desc_lbl = gtk4::Label::new(Some(&tr(&cmd_btn.description)));
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
    let preview_title = gtk4::Label::new(Some(&tr("Preview do Comando:")));
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
                sl.set_text(&badge);
            }
        })
    };

    // Build dynamic fields
    for field in &cmd_btn.form_fields {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 10);
        let lbl = gtk4::Label::new(Some(&format!("{}:", tr(&field.label))));
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

    let btn_insert = gtk4::Button::with_label(&tr("Insert into terminal"));
    let btn_run = gtk4::Button::with_label(&tr("Executar Agora"));
    btn_run.add_css_class("suggested-action");
    let btn_broadcast = gtk4::Button::with_label(&tr("Broadcast p/ Todos"));

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
    // 1. Static CSS Provider for Command Manager dialog styling
    static CSS_INIT: std::sync::Once = std::sync::Once::new();
    CSS_INIT.call_once(|| {
        let provider = gtk4::CssProvider::new();
        provider.load_from_string(
            r#"
            .command-manager-dialog .command-button {
                padding: 6px 14px;
                border-radius: 8px;
                background: alpha(currentColor, 0.05);
                border: 1px solid alpha(currentColor, 0.08);
                min-width: 70px;
            }
            .command-manager-dialog .command-button:hover {
                background: alpha(currentColor, 0.10);
            }
            .command-manager-dialog .commands-flow-box {
                padding: 4px;
            }
            .command-manager-dialog .commands-flow-box > flowboxchild {
                padding: 3px;
                background: transparent;
            }
            .command-manager-dialog .command-input-frame {
                border-radius: 10px;
                background: alpha(currentColor, 0.03);
                border: 1px solid alpha(currentColor, 0.08);
            }
            .command-manager-dialog .command-input-frame:focus-within {
                border-color: alpha(@accent_bg_color, 0.5);
            }
            .command-manager-dialog .bash-textview {
                background: transparent;
                font-family: monospace;
                font-size: 1.05em;
                padding: 8px 12px;
            }
            .command-manager-dialog .execute-button {
                padding: 7px 18px;
                border-radius: 8px;
                font-weight: 600;
            }
            .command-manager-dialog .add-command-button {
                padding: 8px 18px;
                border-radius: 8px;
            }
            "#,
        );
        if let Some(display) = gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });

    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(false)
        .title(&tr("Command Manager"))
        .default_width(820)
        .default_height(560)
        .build();
    dlg.add_css_class("onyxsh-dialog");
    dlg.add_css_class("command-manager-dialog");

    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    // HeaderBar
    let header = adw::HeaderBar::new();
    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text(&tr("Search..."))
        .width_chars(20)
        .build();
    header.pack_start(&search_entry);

    let btn_restore_hidden = gtk4::Button::from_icon_name("view-reveal-symbolic");
    btn_restore_hidden.set_tooltip_text(Some(&tr("Restore hidden commands")));
    btn_restore_hidden.set_visible(false);
    header.pack_end(&btn_restore_hidden);
    main_box.append(&header);

    // Command input section ("Enviar para todos os terminais")
    let input_section = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    input_section.set_margin_top(12);
    input_section.set_margin_bottom(10);
    input_section.set_margin_start(16);
    input_section.set_margin_end(16);

    let hint_lbl = gtk4::Label::new(Some(&tr("Send to all terminals")));
    hint_lbl.add_css_class("dim-label");
    hint_lbl.add_css_class("caption");
    hint_lbl.set_halign(gtk4::Align::Start);
    input_section.append(&hint_lbl);

    let input_overlay = gtk4::Overlay::new();

    let input_frame = gtk4::Frame::new(None);
    input_frame.add_css_class("view");
    input_frame.add_css_class("command-input-frame");

    let scrolled_text = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .min_content_height(60)
        .max_content_height(140)
        .propagate_natural_height(true)
        .build();

    let command_tv = gtk4::TextView::new();
    command_tv.add_css_class("bash-textview");
    command_tv.set_right_margin(85);
    scrolled_text.set_child(Some(&command_tv));
    input_frame.set_child(Some(&scrolled_text));
    input_overlay.set_child(Some(&input_frame));

    let btn_execute = gtk4::Button::with_label(&tr("Execute"));
    btn_execute.add_css_class("suggested-action");
    btn_execute.add_css_class("execute-button");
    btn_execute.set_halign(gtk4::Align::End);
    btn_execute.set_valign(gtk4::Align::Center);
    btn_execute.set_margin_end(8);
    btn_execute.set_tooltip_text(Some(&tr("Executar comando em todos os terminais")));
    input_overlay.add_overlay(&btn_execute);

    {
        let tv_buf = command_tv.buffer();
        let d = Rc::clone(&dispatch_cb);
        btn_execute.connect_clicked(move |_| {
            let start = tv_buf.start_iter();
            let end = tv_buf.end_iter();
            let text = tv_buf.text(&start, &end, false).trim().to_string();
            if !text.is_empty() {
                d(&text, true, true);
                tv_buf.set_text("");
            }
        });
    }

    input_section.append(&input_overlay);
    main_box.append(&input_section);

    let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    sep.set_margin_start(16);
    sep.set_margin_end(16);
    main_box.append(&sep);

    // Scrolled area for command buttons grid
    let scrolled_commands = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .build();

    let flow_box = gtk4::FlowBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .homogeneous(false)
        .max_children_per_line(5)
        .min_children_per_line(2)
        .row_spacing(8)
        .column_spacing(8)
        .margin_top(14)
        .margin_bottom(14)
        .margin_start(16)
        .margin_end(16)
        .valign(gtk4::Align::Start)
        .css_classes(["commands-flow-box"])
        .build();

    scrolled_commands.set_child(Some(&flow_box));
    main_box.append(&scrolled_commands);

    // Bottom action bar with "Adicionar Comando" button
    let bottom_bar = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(12)
        .margin_top(8)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .halign(gtk4::Align::Center)
        .build();

    let btn_add_cmd = gtk4::Button::builder()
        .css_classes(["add-command-button"])
        .build();
    let add_content = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    add_content.append(&gtk4::Image::from_icon_name("list-add-symbolic"));
    add_content.append(&gtk4::Label::new(Some(&tr("Add Command"))));
    btn_add_cmd.set_child(Some(&add_content));
    bottom_bar.append(&btn_add_cmd);
    main_box.append(&bottom_bar);

    let populate_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let populate: Rc<dyn Fn()> = {
        let fb = flow_box.clone();
        let se = search_entry.clone();
        let cmds_rc = Rc::clone(&buttons_rc);
        let rebuild_tb = Rc::clone(&rebuild_toolbar_cb);
        let disp = Rc::clone(&dispatch_cb);
        let win_ref = parent.clone();
        let pop_holder = Rc::clone(&populate_holder);
        let dlg_weak = dlg.downgrade();
        let restore_btn = btn_restore_hidden.clone();

        Rc::new(move || {
            fb.remove_all();

            let q = se.text().trim().to_lowercase();
            let mut list: Vec<(usize, CommandButton)> = cmds_rc
                .borrow()
                .iter()
                .cloned()
                .enumerate()
                .collect();

            // Check if any command is hidden
            let has_hidden = list.iter().any(|(_, c)| c.hidden);
            restore_btn.set_visible(has_hidden);

            // Filter
            list.retain(|(_, c)| {
                if c.hidden {
                    return false;
                }
                if q.is_empty() {
                    return true;
                }
                let tr_name = tr(&c.name).to_lowercase();
                let tr_desc = tr(&c.description).to_lowercase();
                let tr_cat = tr(&c.category).to_lowercase();
                c.name.to_lowercase().contains(&q)
                    || tr_name.contains(&q)
                    || c.description.to_lowercase().contains(&q)
                    || tr_desc.contains(&q)
                    || c.command_template.to_lowercase().contains(&q)
                    || c.category.to_lowercase().contains(&q)
                    || tr_cat.contains(&q)
            });

            // Sort alphabetically by localized display name
            list.sort_by(|(_, a), (_, b)| {
                tr(&a.name)
                    .to_lowercase()
                    .cmp(&tr(&b.name).to_lowercase())
            });

            for (orig_idx, cmd) in list {
                let disp_name = tr(&cmd.name);
                let disp_desc = tr(&cmd.description);
                let btn = gtk4::Button::builder()
                    .css_classes(["command-button"])
                    .tooltip_text(&disp_desc)
                    .build();

                let b_content = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                b_content.append(&gtk4::Image::from_icon_name(&cmd.icon_name));
                let lbl = gtk4::Label::new(Some(&disp_name));
                lbl.set_ellipsize(pango::EllipsizeMode::End);
                b_content.append(&lbl);
                btn.set_child(Some(&b_content));

                // Left click: execute or open form
                {
                    let cmd_c = cmd.clone();
                    let w = win_ref.clone();
                    let d = Rc::clone(&disp);
                    btn.connect_clicked(move |_| {
                        if !cmd_c.form_fields.is_empty()
                            || cmd_c.execution_mode == ExecutionMode::ShowForm
                        {
                            show_command_form_dialog(&w, &cmd_c, Rc::clone(&d));
                        } else {
                            let exec = cmd_c.execution_mode == ExecutionMode::InsertAndExecute;
                            d(&cmd_c.command_template, exec, false);
                        }
                    });
                }

                // Right click: context menu Popover
                {
                    let popover = gtk4::Popover::new();
                    let mbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                    mbox.set_margin_start(4);
                    mbox.set_margin_end(4);
                    mbox.set_margin_top(4);
                    mbox.set_margin_bottom(4);

                    let m_exec_all = gtk4::Button::with_label(&tr("Execute in All Terminals"));
                    m_exec_all.add_css_class("flat");
                    let m_edit = gtk4::Button::with_label(&tr("Edit"));
                    m_edit.add_css_class("flat");
                    let m_duplicate = gtk4::Button::with_label(&tr("Duplicate"));
                    m_duplicate.add_css_class("flat");
                    let m_pin = gtk4::Button::with_label(&if cmd.pinned {
                        tr("Unpin from Toolbar")
                    } else {
                        tr("Pin to Toolbar")
                    });
                    m_pin.add_css_class("flat");
                    let m_hide = gtk4::Button::with_label(&if cmd.is_builtin {
                        tr("Hide")
                    } else {
                        tr("Delete")
                    });
                    m_hide.add_css_class("flat");

                    mbox.append(&m_exec_all);
                    mbox.append(&m_edit);
                    mbox.append(&m_duplicate);
                    mbox.append(&m_pin);
                    mbox.append(&m_hide);
                    popover.set_child(Some(&mbox));
                    popover.set_parent(&btn);

                    // Connect context menu actions
                    {
                        let cmd_c = cmd.clone();
                        let d = Rc::clone(&disp);
                        let p = popover.clone();
                        let w = win_ref.clone();
                        m_exec_all.connect_clicked(move |_| {
                            p.popdown();
                            if !cmd_c.form_fields.is_empty()
                                || cmd_c.execution_mode == ExecutionMode::ShowForm
                            {
                                show_command_form_dialog(&w, &cmd_c, Rc::clone(&d));
                            } else {
                                d(&cmd_c.command_template, true, true);
                            }
                        });
                    }
                    {
                        let cmd_c = cmd.clone();
                        let p = popover.clone();
                        let c_rc = Rc::clone(&cmds_rc);
                        let r_tb = Rc::clone(&rebuild_tb);
                        let ph = Rc::clone(&pop_holder);
                        let dw = dlg_weak.clone();
                        m_edit.connect_clicked(move |_| {
                            p.popdown();
                            if let Some(dlg_parent) = dw.upgrade() {
                                let p_cb = ph.borrow().as_ref().unwrap().clone();
                                show_command_editor_dialog(
                                    &dlg_parent,
                                    Some((orig_idx, cmd_c.clone())),
                                    Rc::clone(&c_rc),
                                    Rc::clone(&r_tb),
                                    p_cb,
                                );
                            }
                        });
                    }
                    {
                        let cmd_c = cmd.clone();
                        let p = popover.clone();
                        let c_rc = Rc::clone(&cmds_rc);
                        let r_tb = Rc::clone(&rebuild_tb);
                        let ph = Rc::clone(&pop_holder);
                        m_duplicate.connect_clicked(move |_| {
                            p.popdown();
                            let mut dup = cmd_c.clone();
                            let millis = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis())
                                .unwrap_or(0);
                            dup.id = format!("{}_copy_{}", dup.id, millis);
                            dup.name = tr_fmt("{name} (Cópia)", &[("name", &tr(&dup.name))]);
                            dup.is_builtin = false;
                            c_rc.borrow_mut().push(dup);
                            let _ = save_command_buttons(&c_rc.borrow());
                            r_tb();
                            if let Some(f) = ph.borrow().as_ref() {
                                f();
                            }
                        });
                    }
                    {
                        let p = popover.clone();
                        let c_rc = Rc::clone(&cmds_rc);
                        let r_tb = Rc::clone(&rebuild_tb);
                        let ph = Rc::clone(&pop_holder);
                        m_pin.connect_clicked(move |_| {
                            p.popdown();
                            if let Some(c) = c_rc.borrow_mut().get_mut(orig_idx) {
                                c.pinned = !c.pinned;
                            }
                            let _ = save_command_buttons(&c_rc.borrow());
                            r_tb();
                            if let Some(f) = ph.borrow().as_ref() {
                                f();
                            }
                        });
                    }
                    {
                        let is_b = cmd.is_builtin;
                        let p = popover.clone();
                        let c_rc = Rc::clone(&cmds_rc);
                        let r_tb = Rc::clone(&rebuild_tb);
                        let ph = Rc::clone(&pop_holder);
                        m_hide.connect_clicked(move |_| {
                            p.popdown();
                            if is_b {
                                if let Some(c) = c_rc.borrow_mut().get_mut(orig_idx) {
                                    c.hidden = true;
                                }
                            } else if orig_idx < c_rc.borrow().len() {
                                c_rc.borrow_mut().remove(orig_idx);
                            }
                            let _ = save_command_buttons(&c_rc.borrow());
                            r_tb();
                            if let Some(f) = ph.borrow().as_ref() {
                                f();
                            }
                        });
                    }

                    let gesture = gtk4::GestureClick::new();
                    gesture.set_button(3); // Right click
                    let pop_c = popover.clone();
                    gesture.connect_pressed(move |_, _, x, y| {
                        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                        pop_c.set_pointing_to(Some(&rect));
                        pop_c.popup();
                    });
                    btn.add_controller(gesture);
                }

                fb.append(&btn);
            }
        })
    };

    *populate_holder.borrow_mut() = Some(Rc::clone(&populate));
    populate();

    {
        let pop = Rc::clone(&populate);
        search_entry.connect_search_changed(move |_| pop());
    }

    // Restore hidden commands button
    {
        let c_rc = Rc::clone(&buttons_rc);
        let r_tb = Rc::clone(&rebuild_toolbar_cb);
        let ph = Rc::clone(&populate_holder);
        btn_restore_hidden.connect_clicked(move |_| {
            for c in c_rc.borrow_mut().iter_mut() {
                c.hidden = false;
            }
            let _ = save_command_buttons(&c_rc.borrow());
            r_tb();
            if let Some(f) = ph.borrow().as_ref() {
                f();
            }
        });
    }

    // Add Command Button clicked
    {
        let c_rc = Rc::clone(&buttons_rc);
        let r_tb = Rc::clone(&rebuild_toolbar_cb);
        let ph = Rc::clone(&populate_holder);
        let dw = dlg.downgrade();
        btn_add_cmd.connect_clicked(move |_| {
            if let Some(dlg_parent) = dw.upgrade() {
                let p_cb = ph.borrow().as_ref().unwrap().clone();
                show_command_editor_dialog(
                    &dlg_parent,
                    None,
                    Rc::clone(&c_rc),
                    Rc::clone(&r_tb),
                    p_cb,
                );
            }
        });
    }

    dlg.set_content(Some(&main_box));
    dlg.present();
}

/// Opens the Command Editor dialog to create a new command or edit an existing one.
fn show_command_editor_dialog(
    parent: &adw::Window,
    cmd_to_edit: Option<(usize, CommandButton)>,
    buttons_rc: Rc<RefCell<Vec<CommandButton>>>,
    rebuild_toolbar_cb: Rc<dyn Fn()>,
    populate_cb: Rc<dyn Fn()>,
) {
    let is_editing = cmd_to_edit.is_some();
    let initial_cmd = cmd_to_edit.as_ref().map(|(_, c)| c.clone()).unwrap_or_else(|| {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        CommandButton {
            id: format!("custom_{}", millis),
            name: String::new(),
            description: String::new(),
            command_template: String::new(),
            category: tr("Custom"),
            icon_name: "utilities-terminal-symbolic".to_string(),
            execution_mode: ExecutionMode::InsertAndExecute,
            pinned: false,
            hidden: false,
            is_builtin: false,
            form_fields: Vec::new(),
        }
    });

    let edit_dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&if is_editing {
            tr_fmt("Editar Comando: {name}", &[("name", &tr(&initial_cmd.name))])
        } else {
            tr("Add Command")
        })
        .default_width(480)
        .default_height(420)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let form = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    form.set_margin_start(16);
    form.set_margin_end(16);
    form.set_margin_top(8);
    form.set_margin_bottom(16);

    let e_name = gtk4::Entry::builder()
        .placeholder_text(&tr("Nome do comando (ex: Reiniciar Nginx)"))
        .text(&tr(&initial_cmd.name))
        .build();

    let e_desc = gtk4::Entry::builder()
        .placeholder_text(&tr("Description (optional)"))
        .text(&tr(&initial_cmd.description))
        .build();

    let e_tpl = gtk4::Entry::builder()
        .placeholder_text(&tr("Template do comando (ex: sudo systemctl restart nginx)"))
        .text(&initial_cmd.command_template)
        .build();
    e_tpl.add_css_class("monospace");

    let e_cat = gtk4::Entry::builder()
        .placeholder_text(&tr("Categoria"))
        .text(&tr(&initial_cmd.category))
        .build();

    let e_icon = gtk4::Entry::builder()
        .placeholder_text(&tr("Nome do ícone (ex: utilities-terminal-symbolic)"))
        .text(&initial_cmd.icon_name)
        .build();

    let chk_pin = gtk4::CheckButton::with_label(&tr("Pin to Toolbar"));
    chk_pin.set_active(initial_cmd.pinned);

    let btn_save = gtk4::Button::with_label(&tr("Salvar Comando"));
    btn_save.add_css_class("suggested-action");

    form.append(&gtk4::Label::builder().label(&tr("Nome:")).halign(gtk4::Align::Start).build());
    form.append(&e_name);
    form.append(&gtk4::Label::builder().label(&tr("Descrição:")).halign(gtk4::Align::Start).build());
    form.append(&e_desc);
    form.append(&gtk4::Label::builder().label(&tr("Comando / Template:")).halign(gtk4::Align::Start).build());
    form.append(&e_tpl);
    form.append(&gtk4::Label::builder().label(&tr("Categoria:")).halign(gtk4::Align::Start).build());
    form.append(&e_cat);
    form.append(&gtk4::Label::builder().label(&tr("Ícone:")).halign(gtk4::Align::Start).build());
    form.append(&e_icon);
    form.append(&chk_pin);
    form.append(&btn_save);

    let d_close = edit_dlg.clone();
    btn_save.connect_clicked(move |_| {
        let name = e_name.text().trim().to_string();
        let tpl = e_tpl.text().trim().to_string();
        if name.is_empty() || tpl.is_empty() {
            return;
        }

        let mut list = buttons_rc.borrow_mut();
        if let Some((idx, _)) = cmd_to_edit {
            if let Some(cmd) = list.get_mut(idx) {
                cmd.name = name;
                cmd.description = e_desc.text().trim().to_string();
                cmd.command_template = tpl;
                cmd.category = e_cat.text().trim().to_string();
                cmd.icon_name = e_icon.text().trim().to_string();
                cmd.pinned = chk_pin.is_active();
            }
        } else {
            let millis = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let next_id = format!("custom_{}", millis);
            list.push(CommandButton {
                id: next_id,
                name,
                description: e_desc.text().trim().to_string(),
                command_template: tpl,
                category: e_cat.text().trim().to_string(),
                icon_name: if e_icon.text().trim().is_empty() {
                    "utilities-terminal-symbolic".to_string()
                } else {
                    e_icon.text().trim().to_string()
                },
                execution_mode: ExecutionMode::InsertAndExecute,
                pinned: chk_pin.is_active(),
                hidden: false,
                is_builtin: false,
                form_fields: Vec::new(),
            });
        }
        let _ = save_command_buttons(&list);
        drop(list);
        rebuild_toolbar_cb();
        populate_cb();
        d_close.close();
    });

    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .child(&form)
        .build();
    vbox.append(&scroll);
    edit_dlg.set_content(Some(&vbox));
    edit_dlg.present();
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
                        status.set_text(&tr_fmt(
                            "🛑 Broadcast bloqueado pelo ProductionGuard: {risk}",
                            &[("risk", &violation.risk_summary)],
                        ));
                    }
                    BroadcastOutcome::FanOut { targets, .. } => {
                        for idx in &targets {
                            if let Some(t) = list.get(*idx as usize) {
                                t.feed_child(payload.as_bytes());
                            }
                        }
                        status.set_text(&tr_fmt(
                            "📡 Comando enviado em Broadcast para {count} terminal(is).",
                            &[("count", &targets.len().to_string())],
                        ));
                    }
                    BroadcastOutcome::SingleTarget(_) => {}
                }
            } else if let Some(term) = active_fn() {
                let guard = ProductionGuard::new();
                if let Some(violation) = guard.evaluate_command(cmd_str) {
                    status.set_text(&tr_fmt(
                        "⚠️ Alerta ProductionGuard ({severity}): {risk}",
                        &[
                            ("severity", &format!("{:?}", violation.severity)),
                            ("risk", &violation.risk_summary),
                        ],
                    ));
                }
                term.feed_child(payload.as_bytes());
                status.set_text(&tr_fmt("Comando enviado: {cmd}", &[("cmd", cmd_str)]));
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
        .tooltip_text(&tr(
            "Barra de Execução Simultânea (Multi-Terminal Broadcast)",
        ))
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

    let bc_badge = gtk4::Label::new(Some(&tr("📡 Broadcast (🛡️ ProductionGuard Ativo):")));
    bc_badge.add_css_class("heading");
    broadcast_bar.append(&bc_badge);

    let bc_entry = gtk4::Entry::builder()
        .placeholder_text(&tr(
            "Digite um comando para enviar a TODOS os terminais abertos...",
        ))
        .hexpand(true)
        .build();
    bc_entry.add_css_class("monospace");
    broadcast_bar.append(&bc_entry);

    let bc_send_btn = gtk4::Button::with_label(&tr("Enviar p/ Todos"));
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
                let disp_name = tr(&cmd.name);
                let disp_desc = tr(&cmd.description);
                let btn = gtk4::Button::new();
                btn.add_css_class("flat");
                btn.set_tooltip_text(Some(&format!(
                    "{} — {}\n$ {}",
                    disp_name, disp_desc, cmd.command_template
                )));

                let b_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
                let icon = gtk4::Image::from_icon_name(&cmd.icon_name);
                let lbl = gtk4::Label::new(Some(&disp_name));
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
        assert_eq!(safe_badge, tr("🟢 Comando Seguro"));
        assert!(reason.is_none());

        let (danger_badge, danger_reason) = evaluate_command_safety_badge("sudo rm -rf /");
        assert_eq!(danger_badge, tr("🔴 Alto Risco (ProductionGuard)"));
        assert!(danger_reason.is_some());
    }

    #[test]
    fn test_builtin_command_buttons_defaults_and_sorting() {
        let buttons = default_command_buttons();
        assert_eq!(buttons.len(), 16);
        let names: Vec<&str> = buttons.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "APT",
                "Compactar",
                "Criar Nova Branch no Git",
                "DNF",
                "Docker Logs",
                "Encontrar Arquivos",
                "Extrair",
                "Flatpak",
                "Git Commit",
                "Journalctl",
                "Listar Arquivos",
                "Logs de Serviço do Systemd",
                "Pacman",
                "Shell Interativo no Docker (Exec)",
                "Sincronizar Arquivos com Rsync",
                "Systemctl",
            ]
        );
        for btn in &buttons {
            assert!(btn.is_builtin);
            assert!(!btn.hidden);
        }
    }
}
