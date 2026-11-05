//! Core infrastructure, configuration, platform detection, security validators,
//! sessions, OpenSSH config parser, snippet resolver, window state,
//! internationalization, and logging for OnyxSH.

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
pub mod ssh_config;
pub mod window_state;

pub const APP_ID: &str = "io.github.vagnarok.OnyxSH";
pub const APP_NAME: &str = "OnyxSH";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
