//! OnyxSH Native Rust Application Entrypoint (GTK4 + Libadwaita + VTE4).
//! Ports `src/onyxsh/main.py` and `src/onyxsh/window.py`.

pub mod agent_scope_dialog;
pub mod ai_assistant_ui;
pub mod ai_config_ui;
pub mod audit_log_dialog;
pub mod checksum_dialog;
pub mod command_manager_ui;
pub mod conversation_history_dialog;
pub mod dialogs;
pub mod export_dialog;
pub mod filemanager_ui;
pub mod global_search_and_shortcuts;
pub mod highlight_dialog_ui;
pub mod main_menu;
pub mod preferences_ui;
pub mod quick_look_dialog;
pub mod resource_dashboard;
pub mod session_sidebar;
pub mod state;
pub mod terminal_search;
pub mod terminal_ux;
pub mod tftp_server_dialog;
pub mod theme_engine;
pub mod tunnel_ui;
pub mod window;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use onyxsh_core::{
    config::{migrate_legacy_config_dirs_if_needed, AppSettings},
    logger::{default_log_file_path, init_logger_with_file, read_recent_sanitized_logs},
    platform, APP_ID, APP_NAME, APP_VERSION,
};

/// Parsed command-line options (`src/onyxsh/main.py`).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CliOptions {
    pub open_new_tab: bool,
    pub working_directory: Option<String>,
    pub execute_command: Option<String>,
    pub show_version: bool,
    pub diagnose: bool,
}

/// Parses command-line arguments (`--tab`, `--working-directory`, `-e`/`--execute`, `--version`, `--diagnose`).
pub fn parse_cli_args<I, S>(args: I) -> CliOptions
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut opts = CliOptions::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        let a = arg.as_ref();
        match a {
            "--tab" | "-t" => opts.open_new_tab = true,
            "--version" | "-v" => opts.show_version = true,
            "--diagnose" => opts.diagnose = true,
            "--working-directory" | "--cwd" => {
                if let Some(dir) = iter.next() {
                    opts.working_directory = Some(dir.as_ref().to_string());
                }
            }
            "--execute" | "-e" => {
                if let Some(cmd) = iter.next() {
                    opts.execute_command = Some(cmd.as_ref().to_string());
                }
            }
            _ => {
                if let Some(val) = a.strip_prefix("--working-directory=") {
                    opts.working_directory = Some(val.to_string());
                } else if let Some(val) = a.strip_prefix("--cwd=") {
                    opts.working_directory = Some(val.to_string());
                } else if let Some(val) = a.strip_prefix("--execute=") {
                    opts.execute_command = Some(val.to_string());
                }
            }
        }
    }
    opts
}

fn print_diagnostics(settings: &AppSettings) {
    let cfg_dir = platform::config_dir();
    let log_file = default_log_file_path();
    println!("{APP_NAME} {APP_VERSION} Diagnostics");
    println!("  OS Context: {}", platform::detect_os_context());
    println!("  Flatpak Sandbox: {}", platform::is_flatpak_sandbox());
    println!("  Config Dir: {}", cfg_dir.display());
    println!("  Data Dir: {}", platform::data_dir().display());
    println!("  Color Scheme: {}", settings.color_scheme);
    println!("  Instance Behavior: {}", settings.new_instance_behavior);
    println!("  Session Restore Policy: {}", settings.session_restore_policy);
    println!("  Log to File: {} ({})", settings.log_to_file, log_file.display());
    let recent = read_recent_sanitized_logs(&log_file, 25);
    println!("  Recent Sanitized Log Lines: {}", recent.len());
    for line in recent {
        println!("    {line}");
    }
}

fn main() -> glib::ExitCode {
    let cli = parse_cli_args(std::env::args().skip(1));
    if cli.show_version {
        println!("{APP_NAME} {APP_VERSION}");
        return glib::ExitCode::SUCCESS;
    }

    let cfg_dir = platform::config_dir();
    if let Ok(Some(legacy_from)) = migrate_legacy_config_dirs_if_needed(&cfg_dir) {
        eprintln!(
            "Migrated legacy OnyxSH configuration from {} to {}",
            legacy_from.display(),
            cfg_dir.display()
        );
    }

    let config_path = cfg_dir.join("settings.json");
    let settings = AppSettings::load_from_path(&config_path).unwrap_or_default();

    let log_level = settings.console_log_level.to_lowercase();
    let log_path = if settings.log_to_file {
        Some(default_log_file_path())
    } else {
        None
    };
    init_logger_with_file(&log_level, log_path.as_deref());

    if cli.diagnose {
        print_diagnostics(&settings);
        return glib::ExitCode::SUCCESS;
    }

    let active_lang = onyxsh_core::i18n::init_i18n();

    tracing::info!(
        app_id = APP_ID,
        version = APP_VERSION,
        locale = %active_lang,
        flatpak = platform::is_flatpak_sandbox(),
        os = %platform::detect_os_context(),
        "Starting {APP_NAME} v{APP_VERSION} (Rust Native GTK4/Libadwaita/VTE4)"
    );

    let single_instance = settings.new_instance_behavior != "new_window";
    let flags = if single_instance {
        gtk4::gio::ApplicationFlags::HANDLES_COMMAND_LINE
    } else {
        gtk4::gio::ApplicationFlags::NON_UNIQUE
    };

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(flags)
        .build();

    if single_instance {
        let st_cl = settings.clone();
        app.connect_command_line(move |app, cmd| {
            let raw_args: Vec<String> = cmd
                .arguments()
                .into_iter()
                .skip(1)
                .map(|s| s.to_string_lossy().to_string())
                .collect();
            let opts = parse_cli_args(&raw_args);
            if let Some(win) = app.windows().into_iter().next() {
                if let Ok(payload) = serde_json::to_string(&opts) {
                    let _ = gtk4::prelude::WidgetExt::activate_action(
                        &win,
                        "win.open-cli-tab",
                        Some(&payload.to_variant()),
                    );
                }
                win.present();
            } else {
                window::build_main_window(app, st_cl.clone(), opts);
            }
            glib::ExitCode::SUCCESS.into()
        });
        let raw_args: Vec<String> = std::env::args().collect();
        app.run_with_args(&raw_args)
    } else {
        let initial_cli = cli.clone();
        app.connect_activate(move |app| {
            window::build_main_window(app, settings.clone(), initial_cli.clone());
        });
        let prog_arg: Vec<String> = std::env::args().take(1).collect();
        app.run_with_args(&prog_arg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_arg_parsing() {
        let opts = parse_cli_args([
            "--tab",
            "--working-directory",
            "/home/vagnarok",
            "-e",
            "htop",
            "--diagnose",
        ]);
        assert!(opts.open_new_tab);
        assert!(opts.diagnose);
        assert_eq!(opts.working_directory.as_deref(), Some("/home/vagnarok"));
        assert_eq!(opts.execute_command.as_deref(), Some("htop"));
    }
}
