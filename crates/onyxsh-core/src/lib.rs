//! Core infrastructure, configuration, platform detection, security validators,
//! sessions, OpenSSH config parser, snippet resolver, window state,
//! internationalization, and logging for OnyxSH.

pub mod backup;
pub mod command_history;
pub mod config;
pub mod crypto;
pub mod i18n;
pub mod importers;
pub mod keyring;
pub mod logger;
pub mod platform;
pub mod security;
pub mod sessions;
pub mod snippets;
pub mod ssh_command;
pub mod ssh_config;
pub mod window_state;

pub const APP_ID: &str = "io.github.vagnarok.OnyxSH";
pub const APP_NAME: &str = "OnyxSH";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEVELOPER_NAME: &str = "VaGNaroK";
pub const DEVELOPERS: &[&str] = &[
    "VaGNaroK (Maintainer)",
    "Leonardo Berbert (Autor Original do Zashterminal)",
];
pub const COPYRIGHT: &str = "© 2026 VaGNaroK | Baseado originalmente no Zashterminal de Leonardo Berbert";
pub const WEBSITE: &str = "https://github.com/VaGNaroK/OnyxSH";
pub const ISSUE_URL: &str = "https://github.com/VaGNaroK/OnyxSH/issues";
