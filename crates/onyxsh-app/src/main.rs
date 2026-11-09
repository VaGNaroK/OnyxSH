//! OnyxSH Native Rust Application Entrypoint (GTK4 + Libadwaita + VTE4).
//! Ports `src/onyxsh/main.py` and `src/onyxsh/window.py`.

pub mod ai_assistant_ui;
pub mod ai_config_ui;
pub mod audit_log_dialog;
pub mod checksum_dialog;
pub mod command_manager_ui;
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
pub mod theme_engine;
pub mod tunnel_ui;
pub mod window;

use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use onyxsh_core::{
    config::AppSettings, logger::init_logger, platform, APP_ID, APP_NAME, APP_VERSION,
};

/// Parsed command-line options (`src/onyxsh/main.py`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CliOptions {
    pub open_new_tab: bool,
    pub working_directory: Option<String>,
    pub execute_command: Option<String>,
    pub show_version: bool,
}

/// Parses command-line arguments (`--tab`, `--working-directory`, `-e`/`--execute`, `--version`).
pub fn parse_cli_args<I, S>(args: I) -> CliOptions
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut opts = CliOptions::default();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_ref() {
            "--tab" | "-t" => opts.open_new_tab = true,
            "--version" | "-v" => opts.show_version = true,
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
            _ => {}
        }
    }
    opts
}

fn main() -> glib::ExitCode {
    let cli = parse_cli_args(std::env::args().skip(1));
    if cli.show_version {
        println!("{APP_NAME} {APP_VERSION}");
        return glib::ExitCode::SUCCESS;
    }

    init_logger("info");
    let config_path = platform::config_dir().join("settings.json");
    let settings = AppSettings::load_from_path(&config_path).unwrap_or_default();

    tracing::info!(
        app_id = APP_ID,
        version = APP_VERSION,
        flatpak = platform::is_flatpak_sandbox(),
        os = %platform::detect_os_context(),
        "Starting {APP_NAME} v{APP_VERSION} (Rust Native GTK4/Libadwaita/VTE4)"
    );

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let initial_cwd = cli.working_directory.clone();
    app.connect_activate(move |app| {
        window::build_main_window(app, settings.clone(), initial_cwd.clone());
    });

    // Pass only program name to GTK so custom CLI flags don't trip GApplication's default parser
    let prog_arg: Vec<String> = std::env::args().take(1).collect();
    app.run_with_args(&prog_arg)
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
        ]);
        assert!(opts.open_new_tab);
        assert_eq!(opts.working_directory.as_deref(), Some("/home/vagnarok"));
        assert_eq!(opts.execute_command.as_deref(), Some("htop"));
    }
}
