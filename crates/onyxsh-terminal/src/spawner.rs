//! Terminal Spawner, Shell Integration Snippet Generator (OSC 133 & OSC 7), and ProcessTracker.
//! Port of `src/onyxsh/terminal/spawner.py`.

use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform::{expand_user_path, home_dir, CommandBuilder};
use onyxsh_core::sessions::SessionItem;
use onyxsh_core::ssh_command::{build_remote_command, RemoteCommandOptions, SshToolKind};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TMP_SHELL_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Generates the Bash integration script emitting OSC 133 (A/B/C/D) and OSC 7 directory hooks.
pub fn bash_integration_script() -> &'static str {
    r#"# OnyxSH Bash Integration (OSC 133 & OSC 7)
__onyxsh_osc7() {
    local host="${HOSTNAME:-$(hostname 2>/dev/null || uname -n)}"
    printf '\033]7;file://%s%s\007' "$host" "$PWD"
}
__onyxsh_prompt_cmd() {
    local exit_code="$?"
    local last_cmd=""
    if command -v history >/dev/null 2>&1; then
        last_cmd=$(HISTTIMEFORMAT= LC_ALL=C history 1 2>/dev/null | sed -e 's/^[[:space:]]*[0-9]*[[:space:]]*\*\?[[:space:]]*//' | tr '\r\n\007\033' '    ')
    fi
    printf '\033]133;D;%s;%s\007' "$exit_code" "$last_cmd"
    __onyxsh_osc7
    printf '\033]133;A\007'
}
if declare -p PROMPT_COMMAND 2>/dev/null | grep -q 'declare -a'; then
    PROMPT_COMMAND=(__onyxsh_prompt_cmd "${PROMPT_COMMAND[@]}")
else
    PROMPT_COMMAND="__onyxsh_prompt_cmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
PS0='\033]133;C\007'
"#
}

/// Generates the Zsh integration script emitting OSC 133 (A/B/C/D) and OSC 7 directory hooks.
pub fn zsh_integration_script() -> &'static str {
    r#"# OnyxSH Zsh Integration (OSC 133 & OSC 7)
__onyxsh_precmd() {
    local exit_code="$?"
    printf '\033]133;D;%s\007' "$exit_code"
    printf '\033]7;file://%s%s\007' "${HOST:-localhost}" "$PWD"
    printf '\033]133;A\007'
}
__onyxsh_preexec() {
    printf '\033]133;C;%s\007' "$1"
}
precmd_functions+=(__onyxsh_precmd)
preexec_functions+=(__onyxsh_preexec)
"#
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedLaunch {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub working_directory: String,
    pub temp_dir: Option<PathBuf>,
}

fn create_unique_integration_dir(prefix: &str) -> std::io::Result<PathBuf> {
    let pid = std::process::id();
    let seq = TMP_SHELL_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("onyxsh-{prefix}-{pid}-{seq}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Prepares a local interactive shell launch with temporary OSC 133/7 integration files (Bash/Zsh).
pub fn prepare_local_shell_launch(
    shell_bin: Option<&str>,
    working_dir: Option<&str>,
    use_login_shell: bool,
    inject_osc_integration: bool,
) -> PreparedLaunch {
    let resolved_shell = shell_bin
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()));

    let cwd = working_dir
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| expand_user_path(s).to_string_lossy().into_owned())
        .unwrap_or_else(|| home_dir().to_string_lossy().into_owned());

    let mut env = HashMap::new();
    env.insert("TERM".to_string(), "xterm-256color".to_string());
    env.insert("COLORTERM".to_string(), "truecolor".to_string());
    env.insert("ONYXSH".to_string(), "1".to_string());

    let shell_name = Path::new(&resolved_shell)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("bash");

    let mut argv = vec![resolved_shell.clone()];
    let mut temp_dir = None;

    if inject_osc_integration && shell_name == "bash" {
        if let Ok(dir) = create_unique_integration_dir("bash") {
            let rc_path = dir.join("bashrc");
            let mut content = String::new();
            if use_login_shell {
                content.push_str(
                    "[ -f /etc/profile ] && . /etc/profile\n\
                     if [ -f \"$HOME/.bash_profile\" ]; then . \"$HOME/.bash_profile\"; \
                     elif [ -f \"$HOME/.bash_login\" ]; then . \"$HOME/.bash_login\"; \
                     elif [ -f \"$HOME/.profile\" ]; then . \"$HOME/.profile\"; fi\n",
                );
            }
            content.push_str("[ -f \"$HOME/.bashrc\" ] && . \"$HOME/.bashrc\"\n");
            content.push_str(bash_integration_script());
            if fs::write(&rc_path, content).is_ok() {
                argv.push("--rcfile".to_string());
                argv.push(rc_path.to_string_lossy().into_owned());
                temp_dir = Some(dir);
            } else {
                let _ = fs::remove_dir_all(&dir);
                if use_login_shell {
                    argv.push("-l".to_string());
                }
            }
        } else if use_login_shell {
            argv.push("-l".to_string());
        }
    } else if inject_osc_integration && shell_name == "zsh" {
        if let Ok(dir) = create_unique_integration_dir("zsh") {
            let orig_zdotdir = std::env::var("ZDOTDIR")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| home_dir().to_string_lossy().into_owned());
            let zshrc_path = dir.join(".zshrc");
            let content = format!(
                "if [ -f \"{orig_zdotdir}/.zshrc\" ]; then ZDOTDIR=\"{orig_zdotdir}\" . \"{orig_zdotdir}/.zshrc\"; fi\n{}",
                zsh_integration_script()
            );
            if fs::write(&zshrc_path, content).is_ok() {
                env.insert("ZDOTDIR".to_string(), dir.to_string_lossy().into_owned());
                if use_login_shell {
                    argv.push("-l".to_string());
                }
                temp_dir = Some(dir);
            } else {
                let _ = fs::remove_dir_all(&dir);
                if use_login_shell {
                    argv.push("-l".to_string());
                }
            }
        } else if use_login_shell {
            argv.push("-l".to_string());
        }
    } else if use_login_shell {
        argv.push("-l".to_string());
    }

    PreparedLaunch {
        argv,
        env,
        working_directory: cwd,
        temp_dir,
    }
}

fn find_binary_in_path(name: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            if dir.join(name).is_file() {
                return true;
            }
        }
    }
    false
}

/// Resolves argv for non-SSH protocols (`telnet`, `serial`).
pub fn resolve_non_ssh_command(
    session: &SessionItem,
    available_tools_override: Option<&[&str]>,
) -> Result<Vec<String>, String> {
    match session.session_type.as_str() {
        "telnet" => {
            let host = session.host.trim();
            if host.is_empty() {
                return Err(tr("Host Telnet não informado na sessão."));
            }
            let tool_ok = available_tools_override
                .map(|tools| tools.contains(&"telnet"))
                .unwrap_or_else(|| find_binary_in_path("telnet"));
            if !tool_ok {
                return Err(tr(
                    "Utilitário 'telnet' não encontrado no PATH. Instale o pacote telnet ou netkit-telnet.",
                ));
            }
            let port = if session.port == 0 { 23 } else { session.port };
            Ok(vec![
                "telnet".to_string(),
                host.to_string(),
                port.to_string(),
            ])
        }
        "serial" => {
            let device = if !session.host.trim().is_empty() {
                session.host.trim()
            } else {
                session.auth_value.trim()
            };
            if device.is_empty() {
                return Err(tr(
                    "Dispositivo serial não informado (ex: /dev/ttyUSB0).",
                ));
            }
            let baud = if let Ok(b) = session.auth_value.trim().parse::<u32>() {
                b
            } else if session.port == 0 || session.port == 22 {
                115_200u32
            } else {
                session.port as u32
            };
            let has_tool = |name: &str| -> bool {
                available_tools_override
                    .map(|tools| tools.contains(&name))
                    .unwrap_or_else(|| find_binary_in_path(name))
            };

            if has_tool("picocom") {
                Ok(vec![
                    "picocom".to_string(),
                    "-b".to_string(),
                    baud.to_string(),
                    device.to_string(),
                ])
            } else if has_tool("tio") {
                Ok(vec![
                    "tio".to_string(),
                    "-b".to_string(),
                    baud.to_string(),
                    device.to_string(),
                ])
            } else if has_tool("screen") {
                Ok(vec![
                    "screen".to_string(),
                    device.to_string(),
                    baud.to_string(),
                ])
            } else {
                Err(tr(
                    "Nenhuma ferramenta serial encontrada (picocom, tio ou screen). Instale picocom ou tio para conexões seriais.",
                ))
            }
        }
        other => Err(tr_fmt(
            "Protocolo de sessão desconhecido: {proto}",
            &[("proto", other)],
        )),
    }
}

/// Builds complete launch argv + env + working directory for any [`SessionItem`],
/// applying OpenSSH options (`sshpass`, `-i`, `ControlMaster`, `ServerAliveInterval`),
/// Flatpak `host-spawn` wrapping, or local shell integration.
pub fn build_session_launch(
    session: Option<&SessionItem>,
    working_dir_override: Option<&str>,
    resolved_password: Option<String>,
    settings: &AppSettings,
    command_builder: Option<&CommandBuilder>,
) -> Result<PreparedLaunch, String> {
    let default_builder = CommandBuilder::new();
    let builder = command_builder.unwrap_or(&default_builder);

    let Some(sess) = session else {
        let mut launch = prepare_local_shell_launch(
            None,
            working_dir_override,
            settings.use_login_shell,
            true,
        );
        launch.argv = builder.wrap_host_command(
            &launch.argv,
            Some(&launch.working_directory),
            Some(&launch.env),
        );
        return Ok(launch);
    };

    match sess.session_type.as_str() {
        "local" => {
            let cwd_opt = working_dir_override.or_else(|| {
                let d = sess.local_working_directory.trim();
                if d.is_empty() {
                    None
                } else {
                    Some(d)
                }
            });
            let mut launch =
                prepare_local_shell_launch(None, cwd_opt, settings.use_login_shell, true);
            launch.argv = builder.wrap_host_command(
                &launch.argv,
                Some(&launch.working_directory),
                Some(&launch.env),
            );
            Ok(launch)
        }
        "ssh" => {
            if sess.host.trim().is_empty() {
                return Err(tr("Host SSH não informado na sessão."));
            }
            let tool = if sess.ssh_connection_mode == "sftp" {
                SshToolKind::Sftp
            } else {
                SshToolKind::Ssh
            };
            let opts = RemoteCommandOptions {
                keepalive_interval: settings.ssh_keepalive_interval,
                keepalive_count_max: 3,
                control_persist_secs: settings.ssh_control_persist_duration,
                use_control_master: true,
                batch_mode: false,
                connect_timeout_secs: 15,
                resolved_password,
                remote_command: None,
            };
            let built = build_remote_command(tool, sess, &opts, Some(builder));
            let mut env = built.env;
            env.insert("TERM".to_string(), "xterm-256color".to_string());
            env.insert("COLORTERM".to_string(), "truecolor".to_string());
            Ok(PreparedLaunch {
                argv: built.argv,
                env,
                working_directory: home_dir().to_string_lossy().into_owned(),
                temp_dir: None,
            })
        }
        "telnet" | "serial" => {
            let cmd = resolve_non_ssh_command(sess, None)?;
            let mut env = HashMap::new();
            env.insert("TERM".to_string(), "xterm-256color".to_string());
            env.insert("COLORTERM".to_string(), "truecolor".to_string());
            let cwd = home_dir().to_string_lossy().into_owned();
            let argv = builder.wrap_host_command(&cmd, Some(&cwd), Some(&env));
            Ok(PreparedLaunch {
                argv,
                env,
                working_directory: cwd,
                temp_dir: None,
            })
        }
        other => Err(tr_fmt(
            "Protocolo de sessão não suportado: {proto}",
            &[("proto", other)],
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedProcess {
    pub pid: i32,
    pub command_line: String,
    pub temp_dir: Option<PathBuf>,
}

/// Tracks active child PTY processes and their temporary shell integration directories.
#[derive(Debug, Default)]
pub struct ProcessTracker {
    processes: HashMap<i32, TrackedProcess>,
}

impl ProcessTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, pid: i32, command_line: &str, temp_dir: Option<PathBuf>) {
        self.processes.insert(
            pid,
            TrackedProcess {
                pid,
                command_line: command_line.to_string(),
                temp_dir,
            },
        );
    }

    pub fn unregister(&mut self, pid: i32) -> Option<TrackedProcess> {
        let removed = self.processes.remove(&pid);
        if let Some(ref proc_info) = removed {
            if let Some(ref dir) = proc_info.temp_dir {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
        removed
    }

    pub fn active_count(&self) -> usize {
        self.processes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_integration_scripts_contain_osc133_and_osc7() {
        let bash = bash_integration_script();
        assert!(bash.contains("133;D;"));
        assert!(bash.contains("133;A"));
        assert!(bash.contains("133;C"));
        assert!(bash.contains("]7;file://"));

        let zsh = zsh_integration_script();
        assert!(zsh.contains("__onyxsh_precmd"));
        assert!(zsh.contains("__onyxsh_preexec"));
    }

    #[test]
    fn test_process_tracker_lifecycle() {
        let mut tracker = ProcessTracker::new();
        tracker.register(20001, "/bin/bash", None);
        assert_eq!(tracker.active_count(), 1);
        let removed = tracker.unregister(20001);
        assert!(removed.is_some());
        assert_eq!(tracker.active_count(), 0);
    }

    #[test]
    fn test_prepare_local_shell_launch_bash_and_zsh() {
        let bash_launch = prepare_local_shell_launch(Some("/bin/bash"), Some("/tmp"), false, true);
        assert_eq!(bash_launch.argv[0], "/bin/bash");
        assert_eq!(bash_launch.argv[1], "--rcfile");
        let bash_tmp = bash_launch.temp_dir.clone().expect("bash tmp dir");
        assert!(Path::new(&bash_launch.argv[2]).exists());

        let mut tracker = ProcessTracker::new();
        tracker.register(99901, "bash", Some(bash_tmp.clone()));
        assert!(bash_tmp.exists());
        tracker.unregister(99901);
        assert!(!bash_tmp.exists());

        let zsh_launch = prepare_local_shell_launch(Some("/usr/bin/zsh"), Some("/tmp"), true, true);
        assert!(zsh_launch.argv.contains(&"-l".to_string()));
        let zdotdir = zsh_launch.env.get("ZDOTDIR").expect("ZDOTDIR env");
        assert!(Path::new(zdotdir).join(".zshrc").exists());
        if let Some(dir) = zsh_launch.temp_dir {
            let _ = fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn test_resolve_non_ssh_commands_and_build_session_launch() {
        let mut telnet_sess = SessionItem::default();
        telnet_sess.session_type = "telnet".to_string();
        telnet_sess.host = "router.internal".to_string();
        telnet_sess.port = 2323;
        let telnet_cmd = resolve_non_ssh_command(&telnet_sess, Some(&["telnet"])).unwrap();
        assert_eq!(telnet_cmd, vec!["telnet", "router.internal", "2323"]);

        let mut serial_sess = SessionItem::default();
        serial_sess.session_type = "serial".to_string();
        serial_sess.host = "/dev/ttyUSB0".to_string();
        let serial_cmd = resolve_non_ssh_command(&serial_sess, Some(&["picocom"])).unwrap();
        assert_eq!(serial_cmd, vec!["picocom", "-b", "115200", "/dev/ttyUSB0"]);
        serial_sess.port = 9600;
        let serial_cmd_9600 = resolve_non_ssh_command(&serial_sess, Some(&["tio"])).unwrap();
        assert_eq!(serial_cmd_9600, vec!["tio", "-b", "9600", "/dev/ttyUSB0"]);
        let err = resolve_non_ssh_command(&serial_sess, Some(&[])).unwrap_err();
        assert!(err.contains("picocom") || err.contains("serial"));

        let mut ssh_sess = SessionItem::default();
        ssh_sess.session_type = "ssh".to_string();
        ssh_sess.host = "prod.example.com".to_string();
        ssh_sess.user = "ops".to_string();
        ssh_sess.auth_type = "password".to_string();
        let builder = CommandBuilder {
            force_flatpak: Some(true),
            host_spawn_path: Some(PathBuf::from("/app/bin/host-spawn")),
            flatpak_spawn_available: Some(false),
        };
        let launched = build_session_launch(
            Some(&ssh_sess),
            None,
            Some("secret123".to_string()),
            &AppSettings::default(),
            Some(&builder),
        )
        .unwrap();
        assert_eq!(launched.argv[0], "/app/bin/host-spawn");
        assert!(launched.argv.contains(&"sshpass".to_string()));
        assert_eq!(launched.env.get("SSHPASS").map(String::as_str), Some("secret123"));
    }
}
