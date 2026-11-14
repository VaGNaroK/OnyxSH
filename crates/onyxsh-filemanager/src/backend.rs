//! FileSystem Backends: `LocalFsBackend`, `SftpFsBackend`, and Full RFC 1350 `UdpTftpServer`
//! (`src/onyxsh/filemanager/local_backend.py`, `sftp_backend.py`, `tftp_server.py`).

use crate::tftp::{
    encode_ack_packet, encode_data_packet, encode_error_packet, parse_request_packet,
    resolve_child_safe, ERR_ACCESS, ERR_EXISTS, ERR_ILLEGAL_OP, ERR_NOT_FOUND, ERR_UNKNOWN_ID,
    OP_ACK, OP_DATA, OP_ERROR, OP_RRQ, OP_WRQ, TFTP_BLOCK_SIZE,
};
use crate::FileEntry;
use onyxsh_core::platform::CommandBuilder;
use onyxsh_core::sessions::SessionItem;
use onyxsh_core::ssh_command::{
    build_remote_command, BuiltRemoteCommand, RemoteCommandOptions, SshToolKind,
};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Common interface implemented by Local and SFTP file manager backends.
pub trait FileSystemBackend {
    fn list_dir(&self, path: &Path) -> io::Result<Vec<FileEntry>>;
    fn mkdir(&self, path: &Path) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path) -> io::Result<()>;
    fn chmod(&self, path: &Path, mode: u16) -> io::Result<()>;
    fn read_bytes(&self, path: &Path, max_bytes: Option<usize>) -> io::Result<Vec<u8>>;
    fn write_bytes(&self, path: &Path, data: &[u8]) -> io::Result<()>;
    fn free_space(&self, path: &Path) -> io::Result<(u64, u64)>;
}

/// Escapes an arbitrary string safely as a single-quoted POSIX shell token.
pub fn shell_single_quote(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    out.push('\'');
    for ch in input.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[cfg(unix)]
pub fn resolve_posix_owner(uid: u32, gid: u32) -> String {
    let username = unsafe {
        let pw = libc::getpwuid(uid);
        if !pw.is_null() && !(*pw).pw_name.is_null() {
            std::ffi::CStr::from_ptr((*pw).pw_name)
                .to_string_lossy()
                .into_owned()
        } else {
            uid.to_string()
        }
    };
    let groupname = unsafe {
        let gr = libc::getgrgid(gid);
        if !gr.is_null() && !(*gr).gr_name.is_null() {
            std::ffi::CStr::from_ptr((*gr).gr_name)
                .to_string_lossy()
                .into_owned()
        } else {
            gid.to_string()
        }
    };
    format!("{username}:{groupname}")
}

#[cfg(not(unix))]
pub fn resolve_posix_owner(_uid: u32, _gid: u32) -> String {
    "user:group".to_string()
}

/// Native local filesystem backend (`src/onyxsh/filemanager/local_backend.py`).
#[derive(Debug, Default, Clone)]
pub struct LocalFsBackend;

impl FileSystemBackend for LocalFsBackend {
    fn list_dir(&self, path: &Path) -> io::Result<Vec<FileEntry>> {
        let mut entries = Vec::new();
        for dir_entry in fs::read_dir(path)? {
            let dir_entry = dir_entry?;
            let entry_path = dir_entry.path();
            let file_type = dir_entry.file_type()?;
            let is_symlink = file_type.is_symlink();
            let symlink_target = if is_symlink {
                fs::read_link(&entry_path)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            } else {
                None
            };
            // Fall back to symlink metadata if symlink target is dangling
            let meta = fs::metadata(&entry_path).or_else(|_| fs::symlink_metadata(&entry_path))?;
            let is_dir = meta.is_dir();
            let size = meta.len();
            let modified_epoch = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let permissions_octal = (meta.permissions().mode() & 0o777) as u16;

            #[cfg(unix)]
            let owner = {
                use std::os::unix::fs::MetadataExt;
                resolve_posix_owner(meta.uid(), meta.gid())
            };
            #[cfg(not(unix))]
            let owner = "user:group".to_string();

            entries.push(FileEntry {
                name: dir_entry.file_name().to_string_lossy().into_owned(),
                path: entry_path.to_string_lossy().into_owned(),
                is_dir,
                is_symlink,
                size,
                modified_epoch,
                permissions_octal,
                owner,
                symlink_target,
            });
        }
        Ok(entries)
    }

    fn mkdir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        let meta = fs::symlink_metadata(path)?;
        if meta.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    }

    fn chmod(&self, path: &Path, mode: u16) -> io::Result<()> {
        fs::set_permissions(path, fs::Permissions::from_mode(u32::from(mode & 0o7777)))
    }

    fn read_bytes(&self, path: &Path, max_bytes: Option<usize>) -> io::Result<Vec<u8>> {
        match max_bytes {
            Some(limit) => {
                let f = File::open(path)?;
                let mut buf = Vec::with_capacity(limit.min(64 * 1024));
                f.take(limit as u64).read_to_end(&mut buf)?;
                Ok(buf)
            }
            None => fs::read(path),
        }
    }

    fn write_bytes(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        fs::write(path, data)
    }

    fn free_space(&self, path: &Path) -> io::Result<(u64, u64)> {
        #[cfg(unix)]
        {
            use std::ffi::CString;
            use std::os::unix::ffi::OsStrExt;
            let c_path = CString::new(path.as_os_str().as_bytes())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
            let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
            let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
            if rc != 0 {
                return Err(io::Error::last_os_error());
            }
            let frsize = if stat.f_frsize > 0 {
                stat.f_frsize as u64
            } else {
                stat.f_bsize as u64
            };
            let avail = (stat.f_bavail as u64).saturating_mul(frsize);
            let total = (stat.f_blocks as u64).saturating_mul(frsize);
            Ok((avail, total))
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            Ok((0, 0))
        }
    }
}

/// Parses POSIX `df -kP <path>` stdout into `(available_bytes, total_bytes)`.
pub fn parse_df_k_output(stdout: &str) -> Option<(u64, u64)> {
    // Skip header lines and accept wrapped two-line filesystem output.
    let mut tokens: Vec<&str> = Vec::new();
    for line in stdout.lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        tokens.extend(trimmed.split_whitespace());
    }
    if tokens.len() < 4 {
        return None;
    }
    // POSIX `df -kP` columns: Filesystem 1024-blocks Used Available Capacity Mounted-on
    let total_kb = tokens[1].parse::<u64>().ok()?;
    let avail_kb = tokens[3].parse::<u64>().ok()?;
    Some((avail_kb.saturating_mul(1024), total_kb.saturating_mul(1024)))
}

/// Parses a standard POSIX `ls -l` / `ls -la --time-style=+%s` / SFTP `ls -l` line into a [`FileEntry`].
pub fn parse_sftp_ls_line(parent_dir: &str, line: &str) -> Option<FileEntry> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("total ") {
        return None;
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 8 {
        return None;
    }
    let perm_str = parts[0];
    let first_char = perm_str.chars().next()?;
    let is_dir = first_char == 'd';
    let is_symlink = first_char == 'l';
    let size = parts[4].parse::<u64>().unwrap_or(0);

    // Detect `ls -la --time-style=+%s` (token 5 is numeric epoch, name starts at token 6)
    // vs traditional POSIX `ls -l` (`Mon Day Time/Year`, name starts at token 8).
    let (modified_epoch, name_start_idx) = if let Ok(epoch) = parts[5].parse::<u64>() {
        (epoch, 6usize)
    } else if parts.len() >= 9 {
        (0u64, 8usize)
    } else {
        return None;
    };
    if parts.len() <= name_start_idx {
        return None;
    }

    let raw_name = parts[name_start_idx..].join(" ");
    let (name, symlink_target) = if is_symlink {
        if let Some((lhs, rhs)) = raw_name.split_once(" -> ") {
            (lhs.trim().to_string(), Some(rhs.trim().to_string()))
        } else {
            (raw_name, None)
        }
    } else {
        (raw_name, None)
    };

    if name.is_empty() || name == "." || name == ".." {
        return None;
    }

    let mut octal = 0u16;
    let chars: Vec<char> = perm_str.chars().skip(1).take(9).collect();
    if chars.len() == 9 {
        for (idx, c) in chars.iter().enumerate() {
            if *c != '-' {
                octal |= 1 << (8 - idx);
            }
        }
    }

    let clean_parent = if parent_dir == "/" {
        "/"
    } else {
        parent_dir.trim_end_matches('/')
    };
    let full_path = if clean_parent == "/" {
        format!("/{name}")
    } else {
        format!("{clean_parent}/{name}")
    };

    let owner = format!("{}:{}", parts[2], parts[3]);

    Some(FileEntry {
        name,
        path: full_path,
        is_dir,
        is_symlink,
        size,
        modified_epoch,
        permissions_octal: octal,
        owner,
        symlink_target,
    })
}

/// SSH/SFTP filesystem backend backed by native OpenSSH CLI (`ssh` / `scp` / `ControlMaster` / `sshpass`).
#[derive(Debug, Clone)]
pub struct SftpFsBackend {
    pub session: SessionItem,
    pub resolved_password: Option<String>,
}

impl SftpFsBackend {
    pub fn new(session: SessionItem, resolved_password: Option<String>) -> Self {
        Self {
            session,
            resolved_password,
        }
    }

    pub fn display_target(&self) -> String {
        self.session.connection_string()
    }

    pub fn build_ssh_command(&self, remote_cmd: &str) -> BuiltRemoteCommand {
        let has_pw = self
            .resolved_password
            .as_ref()
            .is_some_and(|p| !p.is_empty());
        let opts = RemoteCommandOptions {
            keepalive_interval: 15,
            keepalive_count_max: 3,
            control_persist_secs: 600,
            use_control_master: true,
            batch_mode: !has_pw,
            connect_timeout_secs: 12,
            resolved_password: self.resolved_password.clone(),
            remote_command: Some(remote_cmd.to_string()),
        };
        build_remote_command(
            SshToolKind::Ssh,
            &self.session,
            &opts,
            Some(&CommandBuilder::new()),
        )
    }

    pub fn build_scp_transfer_command(&self, recursive: bool, src: &str, dst: &str) -> BuiltRemoteCommand {
        let has_pw = self
            .resolved_password
            .as_ref()
            .is_some_and(|p| !p.is_empty());
        let opts = RemoteCommandOptions {
            keepalive_interval: 15,
            keepalive_count_max: 3,
            control_persist_secs: 600,
            use_control_master: true,
            batch_mode: !has_pw,
            connect_timeout_secs: 15,
            resolved_password: self.resolved_password.clone(),
            remote_command: None,
        };
        let raw = build_remote_command(SshToolKind::Scp, &self.session, &opts, None);
        let mut cmd = raw.argv;
        // Inject `-r` right after `scp` when copying directories
        if recursive {
            if let Some(pos) = cmd.iter().position(|a| a == "scp") {
                cmd.insert(pos + 1, "-r".to_string());
            } else {
                cmd.push("-r".to_string());
            }
        }
        cmd.push(src.to_string());
        cmd.push(dst.to_string());

        let env_opt = (!raw.env.is_empty()).then_some(&raw.env);
        let final_argv = CommandBuilder::new().wrap_host_command(&cmd, None, env_opt);
        BuiltRemoteCommand {
            argv: final_argv,
            env: raw.env,
            control_socket_path: raw.control_socket_path,
        }
    }

    pub fn remote_scp_spec(&self, remote_path: &Path) -> String {
        let target = if self.session.user.trim().is_empty() {
            self.session.host.trim().to_string()
        } else {
            format!("{}@{}", self.session.user.trim(), self.session.host.trim())
        };
        format!("{target}:{}", remote_path.to_string_lossy())
    }

    fn run_ssh(&self, remote_cmd: &str, stdin_bytes: Option<&[u8]>) -> io::Result<Vec<u8>> {
        let built = self.build_ssh_command(remote_cmd);
        let (bin, args) = built
            .argv
            .split_first()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Empty SSH command"))?;
        let mut cmd = Command::new(bin);
        cmd.args(args);
        cmd.envs(&built.env);
        if stdin_bytes.is_some() {
            cmd.stdin(Stdio::piped());
        } else {
            cmd.stdin(Stdio::null());
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        if let Some(bytes) = stdin_bytes {
            if let Some(mut child_in) = child.stdin.take() {
                child_in.write_all(bytes)?;
            }
        }
        let out = child.wait_with_output()?;
        if !out.status.success() {
            let err_txt = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(io::Error::other(if err_txt.is_empty() {
                format!("SSH exited with status {}", out.status)
            } else {
                err_txt
            }));
        }
        Ok(out.stdout)
    }

    /// Queries the remote user's initial working directory (`pwd`).
    pub fn resolve_home_dir(&self) -> io::Result<PathBuf> {
        let stdout = self.run_ssh("pwd", None)?;
        let raw = String::from_utf8_lossy(&stdout).trim().to_string();
        if raw.is_empty() {
            Ok(PathBuf::from("/"))
        } else {
            Ok(PathBuf::from(raw))
        }
    }

    /// Uploads a local file or directory to `remote_dst` using OpenSSH `scp`.
    pub fn upload_path(&self, local_src: &Path, remote_dst: &Path) -> io::Result<()> {
        let is_dir = fs::metadata(local_src).is_ok_and(|m| m.is_dir());
        let src_str = local_src.to_string_lossy().into_owned();
        let dst_str = self.remote_scp_spec(remote_dst);
        let built = self.build_scp_transfer_command(is_dir, &src_str, &dst_str);
        Self::exec_built(&built)
    }

    /// Downloads a remote file or directory (`is_dir`) to `local_dst` using OpenSSH `scp`.
    pub fn download_path(&self, remote_src: &Path, local_dst: &Path, is_dir: bool) -> io::Result<()> {
        let src_str = self.remote_scp_spec(remote_src);
        let dst_str = local_dst.to_string_lossy().into_owned();
        let built = self.build_scp_transfer_command(is_dir, &src_str, &dst_str);
        Self::exec_built(&built)
    }

    fn exec_built(built: &BuiltRemoteCommand) -> io::Result<()> {
        let (bin, args) = built
            .argv
            .split_first()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Empty SCP command"))?;
        let out = Command::new(bin)
            .args(args)
            .envs(&built.env)
            .stdin(Stdio::null())
            .output()?;
        if !out.status.success() {
            let err_txt = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(io::Error::other(if err_txt.is_empty() {
                format!("SCP failed with status {}", out.status)
            } else {
                err_txt
            }));
        }
        Ok(())
    }
}

impl FileSystemBackend for SftpFsBackend {
    fn list_dir(&self, path: &Path) -> io::Result<Vec<FileEntry>> {
        let q = shell_single_quote(&path.to_string_lossy());
        // Prefer GNU `--time-style=+%s`, fallback seamlessly to POSIX `ls -la` on BusyBox/BSD.
        let remote_cmd =
            format!("LC_ALL=C ls -la --time-style=+%s {q} 2>/dev/null || LC_ALL=C ls -la {q}");
        let stdout = self.run_ssh(&remote_cmd, None)?;
        let text = String::from_utf8_lossy(&stdout);
        let parent_str = path.to_string_lossy();
        let mut entries = Vec::new();
        for line in text.lines() {
            if let Some(entry) = parse_sftp_ls_line(&parent_str, line) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    fn mkdir(&self, path: &Path) -> io::Result<()> {
        let q = shell_single_quote(&path.to_string_lossy());
        self.run_ssh(&format!("mkdir -p -- {q}"), None)?;
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        let q_from = shell_single_quote(&from.to_string_lossy());
        let q_to = shell_single_quote(&to.to_string_lossy());
        self.run_ssh(&format!("mv -- {q_from} {q_to}"), None)?;
        Ok(())
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        let raw = path.to_string_lossy();
        if raw.trim() == "/" || raw.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Refusing to remove root path",
            ));
        }
        let q = shell_single_quote(&raw);
        self.run_ssh(&format!("rm -rf -- {q}"), None)?;
        Ok(())
    }

    fn chmod(&self, path: &Path, mode: u16) -> io::Result<()> {
        let q = shell_single_quote(&path.to_string_lossy());
        let cmd = format!("chmod {:04o} -- {q}", mode & 0o7777);
        self.run_ssh(&cmd, None)?;
        Ok(())
    }

    fn read_bytes(&self, path: &Path, max_bytes: Option<usize>) -> io::Result<Vec<u8>> {
        let q = shell_single_quote(&path.to_string_lossy());
        let cmd = match max_bytes {
            Some(limit) => format!("head -c {limit} -- {q} 2>/dev/null || cat -- {q}"),
            None => format!("cat -- {q}"),
        };
        let mut out = self.run_ssh(&cmd, None)?;
        if let Some(limit) = max_bytes {
            if out.len() > limit {
                out.truncate(limit);
            }
        }
        Ok(out)
    }

    fn write_bytes(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        let q = shell_single_quote(&path.to_string_lossy());
        self.run_ssh(&format!("cat > {q}"), Some(data))?;
        Ok(())
    }

    fn free_space(&self, path: &Path) -> io::Result<(u64, u64)> {
        let q = shell_single_quote(&path.to_string_lossy());
        let stdout = self.run_ssh(&format!("LC_ALL=C df -kP -- {q}"), None)?;
        let text = String::from_utf8_lossy(&stdout);
        parse_df_k_output(&text)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Unparseable df output"))
    }
}

/// Configuration for [`UdpTftpServer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TftpServerConfig {
    pub bind_addr: String,
    pub port: u16,
    pub upload_dir: PathBuf,
    pub download_dir: PathBuf,
    pub timeout_ms: u64,
    pub max_retries: usize,
}

impl Default for TftpServerConfig {
    fn default() -> Self {
        let home = onyxsh_core::platform::home_dir();
        Self {
            bind_addr: "0.0.0.0".to_string(),
            port: 6969,
            upload_dir: home.clone(),
            download_dir: home,
            timeout_ms: 2000,
            max_retries: 3,
        }
    }
}

/// Live transfer audit event emitted by [`UdpTftpServer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TftpTransferEvent {
    pub timestamp: String,
    pub client: String,
    pub operation: String,
    pub filename: String,
    pub bytes: usize,
    pub success: bool,
    pub detail: String,
}

fn format_now_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02} UTC")
}

/// Threaded RFC 1350 UDP TFTP Server supporting multi-block RRQ and WRQ transfers.
pub struct UdpTftpServer {
    config: TftpServerConfig,
    bound_addr: SocketAddr,
    stop_flag: Arc<AtomicBool>,
    dispatcher: Mutex<Option<JoinHandle<()>>>,
}

impl UdpTftpServer {
    /// Starts the threaded RFC 1350 server dispatcher immediately on `config.bind_addr:config.port`.
    pub fn start(
        config: TftpServerConfig,
        on_event: Option<Arc<dyn Fn(TftpTransferEvent) + Send + Sync>>,
    ) -> io::Result<Self> {
        if !config.upload_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Diretório de Upload inválido: {}", config.upload_dir.display()),
            ));
        }
        if !config.download_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Diretório de Download inválido: {}",
                    config.download_dir.display()
                ),
            ));
        }

        let listen_addr = format!("{}:{}", config.bind_addr, config.port);
        let socket = UdpSocket::bind(&listen_addr)?;
        socket.set_read_timeout(Some(Duration::from_millis(250)))?;
        let bound_addr = socket.local_addr()?;

        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_c = Arc::clone(&stop_flag);
        let cfg_c = config.clone();

        let handle = thread::Builder::new()
            .name("OnyxShTftpDispatcher".to_string())
            .spawn(move || {
                let mut buf = [0u8; 2048];
                while !stop_c.load(Ordering::SeqCst) {
                    match socket.recv_from(&mut buf) {
                        Ok((len, peer)) => {
                            let packet = buf[..len].to_vec();
                            let cfg_worker = cfg_c.clone();
                            let stop_worker = Arc::clone(&stop_c);
                            let cb_worker = on_event.clone();
                            let _ = thread::Builder::new()
                                .name(format!("OnyxShTftpClient-{peer}"))
                                .spawn(move || {
                                    Self::handle_client_session(
                                        &packet,
                                        peer,
                                        &cfg_worker,
                                        &stop_worker,
                                        cb_worker.as_ref(),
                                    );
                                });
                        }
                        Err(e)
                            if e.kind() == io::ErrorKind::WouldBlock
                                || e.kind() == io::ErrorKind::TimedOut =>
                        {
                            continue;
                        }
                        Err(_) => {
                            if stop_c.load(Ordering::SeqCst) {
                                break;
                            }
                        }
                    }
                }
            })?;

        Ok(Self {
            config,
            bound_addr,
            stop_flag,
            dispatcher: Mutex::new(Some(handle)),
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.bound_addr)
    }

    pub fn config(&self) -> &TftpServerConfig {
        &self.config
    }

    pub fn is_running(&self) -> bool {
        !self.stop_flag.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.dispatcher.lock() {
            if let Some(h) = guard.take() {
                let _ = h.join();
            }
        }
    }

    fn emit_event(
        cb: Option<&Arc<dyn Fn(TftpTransferEvent) + Send + Sync>>,
        peer: SocketAddr,
        operation: &str,
        filename: &str,
        bytes: usize,
        success: bool,
        detail: &str,
    ) {
        if let Some(f) = cb {
            f(TftpTransferEvent {
                timestamp: format_now_hms(),
                client: peer.to_string(),
                operation: operation.to_string(),
                filename: filename.to_string(),
                bytes,
                success,
                detail: detail.to_string(),
            });
        }
    }

    fn handle_client_session(
        packet: &[u8],
        peer: SocketAddr,
        cfg: &TftpServerConfig,
        stop_flag: &AtomicBool,
        on_event: Option<&Arc<dyn Fn(TftpTransferEvent) + Send + Sync>>,
    ) {
        let ephemeral_bind = if peer.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
        let Ok(sock) = UdpSocket::bind(ephemeral_bind) else {
            return;
        };
        let _ = sock.set_read_timeout(Some(Duration::from_millis(cfg.timeout_ms.max(200))));

        let Ok(req) = parse_request_packet(packet) else {
            let err = encode_error_packet(ERR_ILLEGAL_OP, "Illegal TFTP operation");
            let _ = sock.send_to(&err, peer);
            Self::emit_event(on_event, peer, "REQ", "-", 0, false, "Operação TFTP inválida");
            return;
        };

        if req.opcode == OP_RRQ {
            Self::serve_rrq(&sock, peer, &req.filename, cfg, stop_flag, on_event);
        } else if req.opcode == OP_WRQ {
            Self::serve_wrq(&sock, peer, &req.filename, cfg, stop_flag, on_event);
        } else {
            let err = encode_error_packet(ERR_ILLEGAL_OP, "Illegal TFTP operation");
            let _ = sock.send_to(&err, peer);
        }
    }

    fn serve_rrq(
        sock: &UdpSocket,
        peer: SocketAddr,
        filename: &str,
        cfg: &TftpServerConfig,
        stop_flag: &AtomicBool,
        on_event: Option<&Arc<dyn Fn(TftpTransferEvent) + Send + Sync>>,
    ) {
        let Some(target) = resolve_child_safe(&cfg.upload_dir, filename) else {
            let _ = sock.send_to(&encode_error_packet(ERR_ACCESS, "Access violation"), peer);
            Self::emit_event(on_event, peer, "RRQ", filename, 0, false, "Path traversal bloqueado");
            return;
        };
        if !target.is_file() {
            let _ = sock.send_to(&encode_error_packet(ERR_NOT_FOUND, "File not found"), peer);
            Self::emit_event(on_event, peer, "RRQ", filename, 0, false, "Arquivo não encontrado");
            return;
        }
        let Ok(mut file) = File::open(&target) else {
            let _ = sock.send_to(&encode_error_packet(ERR_ACCESS, "Access denied"), peer);
            Self::emit_event(on_event, peer, "RRQ", filename, 0, false, "Erro ao abrir arquivo");
            return;
        };

        let mut block: u16 = 1;
        let mut total_sent = 0usize;
        let mut chunk_buf = [0u8; TFTP_BLOCK_SIZE];
        let mut ack_buf = [0u8; 64];

        while !stop_flag.load(Ordering::SeqCst) {
            let Ok(n) = file.read(&mut chunk_buf) else {
                let _ = sock.send_to(&encode_error_packet(ERR_ACCESS, "Read failure"), peer);
                Self::emit_event(on_event, peer, "RRQ", filename, total_sent, false, "Falha de leitura");
                return;
            };
            let data_pkt = encode_data_packet(block, &chunk_buf[..n]);
            let mut acked = false;
            for _attempt in 0..cfg.max_retries.max(1) {
                if sock.send_to(&data_pkt, peer).is_err() {
                    break;
                }
                while !stop_flag.load(Ordering::SeqCst) {
                    match sock.recv_from(&mut ack_buf) {
                        Ok((len, sender)) => {
                            if sender != peer {
                                let _ = sock.send_to(
                                    &encode_error_packet(ERR_UNKNOWN_ID, "Unknown transfer ID"),
                                    sender,
                                );
                                continue;
                            }
                            if len >= 4 {
                                let op = u16::from_be_bytes([ack_buf[0], ack_buf[1]]);
                                let blk = u16::from_be_bytes([ack_buf[2], ack_buf[3]]);
                                if op == OP_ACK && blk == block {
                                    acked = true;
                                    break;
                                }
                                if op == OP_ERROR {
                                    Self::emit_event(
                                        on_event,
                                        peer,
                                        "RRQ",
                                        filename,
                                        total_sent,
                                        false,
                                        "Cliente cancelou",
                                    );
                                    return;
                                }
                            }
                        }
                        Err(e)
                            if e.kind() == io::ErrorKind::WouldBlock
                                || e.kind() == io::ErrorKind::TimedOut =>
                        {
                            break;
                        }
                        Err(_) => break,
                    }
                }
                if acked {
                    break;
                }
            }

            if !acked {
                Self::emit_event(
                    on_event,
                    peer,
                    "RRQ",
                    filename,
                    total_sent,
                    false,
                    "Timeout aguardando ACK",
                );
                return;
            }
            total_sent += n;
            block = block.wrapping_add(1);
            if n < TFTP_BLOCK_SIZE {
                Self::emit_event(
                    on_event,
                    peer,
                    "RRQ",
                    filename,
                    total_sent,
                    true,
                    "Enviado (RRQ)",
                );
                return;
            }
        }
    }

    fn serve_wrq(
        sock: &UdpSocket,
        peer: SocketAddr,
        filename: &str,
        cfg: &TftpServerConfig,
        stop_flag: &AtomicBool,
        on_event: Option<&Arc<dyn Fn(TftpTransferEvent) + Send + Sync>>,
    ) {
        let Some(target) = resolve_child_safe(&cfg.download_dir, filename) else {
            let _ = sock.send_to(&encode_error_packet(ERR_ACCESS, "Access violation"), peer);
            Self::emit_event(on_event, peer, "WRQ", filename, 0, false, "Path traversal bloqueado");
            return;
        };
        if target.exists() {
            let _ = sock.send_to(&encode_error_packet(ERR_EXISTS, "File already exists"), peer);
            Self::emit_event(on_event, peer, "WRQ", filename, 0, false, "Arquivo já existe");
            return;
        }
        if let Some(parent) = target.parent() {
            if fs::create_dir_all(parent).is_err() {
                let _ = sock.send_to(&encode_error_packet(ERR_ACCESS, "Cannot create directory"), peer);
                return;
            }
        }
        let Ok(mut file) = OpenOptions::new().write(true).create_new(true).open(&target) else {
            let _ = sock.send_to(&encode_error_packet(ERR_ACCESS, "Cannot create file"), peer);
            Self::emit_event(on_event, peer, "WRQ", filename, 0, false, "Sem permissão de escrita");
            return;
        };

        if sock.send_to(&encode_ack_packet(0), peer).is_err() {
            drop(file);
            let _ = fs::remove_file(&target);
            return;
        }

        let mut expected_block: u16 = 1;
        let mut total_received = 0usize;
        let mut data_buf = vec![0u8; TFTP_BLOCK_SIZE + 16];
        let mut timeouts = 0usize;

        while !stop_flag.load(Ordering::SeqCst) {
            match sock.recv_from(&mut data_buf) {
                Ok((len, sender)) => {
                    timeouts = 0;
                    if sender != peer {
                        let _ = sock.send_to(
                            &encode_error_packet(ERR_UNKNOWN_ID, "Unknown transfer ID"),
                            sender,
                        );
                        continue;
                    }
                    if len < 4 {
                        let _ = sock.send_to(&encode_error_packet(ERR_ILLEGAL_OP, "Malformed DATA"), peer);
                        break;
                    }
                    let op = u16::from_be_bytes([data_buf[0], data_buf[1]]);
                    let block = u16::from_be_bytes([data_buf[2], data_buf[3]]);
                    if op != OP_DATA {
                        let _ = sock.send_to(&encode_error_packet(ERR_ILLEGAL_OP, "Expected DATA"), peer);
                        break;
                    }
                    if block != expected_block {
                        let _ = sock.send_to(&encode_ack_packet(expected_block.wrapping_sub(1)), peer);
                        continue;
                    }
                    let payload = &data_buf[4..len];
                    if file.write_all(payload).is_err() {
                        let _ = sock.send_to(&encode_error_packet(ERR_ACCESS, "Write failure"), peer);
                        break;
                    }
                    total_received += payload.len();
                    let _ = sock.send_to(&encode_ack_packet(block), peer);
                    expected_block = expected_block.wrapping_add(1);
                    if payload.len() < TFTP_BLOCK_SIZE {
                        let _ = file.flush();
                        Self::emit_event(
                            on_event,
                            peer,
                            "WRQ",
                            filename,
                            total_received,
                            true,
                            "Recebido (WRQ)",
                        );
                        return;
                    }
                }
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                {
                    timeouts += 1;
                    if timeouts >= cfg.max_retries.max(1) {
                        break;
                    }
                    let _ = sock.send_to(&encode_ack_packet(expected_block.wrapping_sub(1)), peer);
                }
                Err(_) => break,
            }
        }

        drop(file);
        let _ = fs::remove_file(&target);
        Self::emit_event(
            on_event,
            peer,
            "WRQ",
            filename,
            total_received,
            false,
            "Transferência WRQ interrompida",
        );
    }
}

impl Drop for UdpTftpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_fs_backend_and_sftp_ls_parser() {
        let dir = std::env::temp_dir().join(format!("onyxsh_fm_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let backend = LocalFsBackend;

        let sub = dir.join("configs");
        backend.mkdir(&sub).unwrap();
        fs::write(dir.join("router.conf"), b"hostname core-sw1\n").unwrap();

        let entries = backend.list_dir(&dir).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(backend.free_space(&dir).unwrap().1 > 0);
        let _ = fs::remove_dir_all(&dir);

        let parsed = parse_sftp_ls_line(
            "/var/log",
            "-rw-r--r-- 1 root root 4096 Nov 4 12:00 syslog",
        )
        .expect("parsed ls line");
        assert_eq!(parsed.name, "syslog");
        assert_eq!(parsed.path, "/var/log/syslog");
        assert_eq!(parsed.permissions_octal, 0o644);
        assert_eq!(parsed.symlink_target, None);

        // Symlink + unix epoch (`ls -la --time-style=+%s`)
        let link = parse_sftp_ls_line(
            "/etc",
            "lrwxrwxrwx 1 root root 18 1731500000 current.conf -> /opt/app/my conf.cfg",
        )
        .expect("parsed symlink line");
        assert!(link.is_symlink);
        assert_eq!(link.name, "current.conf");
        assert_eq!(link.path, "/etc/current.conf");
        assert_eq!(link.modified_epoch, 1731500000);
        assert_eq!(link.symlink_target.as_deref(), Some("/opt/app/my conf.cfg"));

        let df = parse_df_k_output(
            "Filesystem     1024-blocks      Used Available Capacity Mounted on\n/dev/sda1        104857600  20971520  83886080      20% /\n",
        );
        assert_eq!(df, Some((83886080 * 1024, 104857600 * 1024)));
    }

    #[test]
    fn test_sftp_backend_command_builder_and_quoting() {
        assert_eq!(shell_single_quote("/tmp/a b/c'd"), "'/tmp/a b/c'\\''d'");

        let mut sess = SessionItem::default();
        sess.session_type = "ssh".to_string();
        sess.host = "10.0.0.50".to_string();
        sess.user = "netadmin".to_string();
        sess.port = 2222;
        sess.auth_type = "password".to_string();

        let sftp = SftpFsBackend::new(sess, Some("s3cr3t".to_string()));
        let ssh_cmd = sftp.build_ssh_command("df -kP '/var/log'");
        assert!(ssh_cmd.argv.iter().any(|a| a == "sshpass"));
        assert!(ssh_cmd.argv.iter().any(|a| a == "2222"));
        assert!(ssh_cmd.argv.iter().any(|a| a.contains("ControlMaster=auto")));
        assert_eq!(ssh_cmd.env.get("SSHPASS").map(String::as_str), Some("s3cr3t"));

        let scp_cmd = sftp.build_scp_transfer_command(true, "/local/dir", "netadmin@10.0.0.50:/remote/dir");
        assert!(scp_cmd.argv.iter().any(|a| a == "-r"));
        assert!(scp_cmd.argv.iter().any(|a| a == "netadmin@10.0.0.50:/remote/dir"));
    }

    #[test]
    fn test_live_udp_tftp_server_multiblock_rrq_and_wrq() {
        let dir = std::env::temp_dir().join(format!("onyxsh_tftp_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let up_dir = dir.join("up");
        let dn_dir = dir.join("dn");
        fs::create_dir_all(&up_dir).unwrap();
        fs::create_dir_all(&dn_dir).unwrap();

        // Multi-block file (1200 bytes -> 3 packets: 512, 512, 176)
        let payload: Vec<u8> = (0..1200u32).map(|i| (i % 251) as u8).collect();
        fs::write(up_dir.join("firmware.bin"), &payload).unwrap();

        let events: Arc<Mutex<Vec<TftpTransferEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_c = Arc::clone(&events);

        let cfg = TftpServerConfig {
            bind_addr: "127.0.0.1".to_string(),
            port: 0,
            upload_dir: up_dir.clone(),
            download_dir: dn_dir.clone(),
            timeout_ms: 800,
            max_retries: 2,
        };
        let server = UdpTftpServer::start(
            cfg,
            Some(Arc::new(move |ev| {
                events_c.lock().unwrap().push(ev);
            })),
        )
        .unwrap();
        let server_addr = server.local_addr().unwrap();

        // 1. Test multi-block RRQ
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut rrq = vec![0x00, 0x01];
        rrq.extend_from_slice(b"firmware.bin\0octet\0");
        client.send_to(&rrq, server_addr).unwrap();

        let mut received = Vec::new();
        let mut expected_blk = 1u16;
        loop {
            let mut resp = [0u8; 600];
            let (n, tid) = client.recv_from(&mut resp).unwrap();
            assert_eq!(u16::from_be_bytes([resp[0], resp[1]]), OP_DATA);
            let blk = u16::from_be_bytes([resp[2], resp[3]]);
            assert_eq!(blk, expected_blk);
            received.extend_from_slice(&resp[4..n]);
            client.send_to(&encode_ack_packet(blk), tid).unwrap();
            expected_blk += 1;
            if n - 4 < TFTP_BLOCK_SIZE {
                break;
            }
        }
        assert_eq!(received, payload);

        // 2. Test multi-block WRQ (700 bytes -> 2 DATA packets: 512, 188)
        let wrq_payload: Vec<u8> = (0..700u32).map(|i| ((i * 3) % 253) as u8).collect();
        let wrq_client = UdpSocket::bind("127.0.0.1:0").unwrap();
        wrq_client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut wrq = vec![0x00, 0x02];
        wrq.extend_from_slice(b"uploaded.cfg\0octet\0");
        wrq_client.send_to(&wrq, server_addr).unwrap();

        let mut ack_pkt = [0u8; 64];
        let (an, wrq_tid) = wrq_client.recv_from(&mut ack_pkt).unwrap();
        assert_eq!(an, 4);
        assert_eq!(u16::from_be_bytes([ack_pkt[0], ack_pkt[1]]), OP_ACK);
        assert_eq!(u16::from_be_bytes([ack_pkt[2], ack_pkt[3]]), 0);

        for (i, chunk) in wrq_payload.chunks(TFTP_BLOCK_SIZE).enumerate() {
            let blk = (i + 1) as u16;
            wrq_client.send_to(&encode_data_packet(blk, chunk), wrq_tid).unwrap();
            let (n2, _) = wrq_client.recv_from(&mut ack_pkt).unwrap();
            assert_eq!(n2, 4);
            assert_eq!(u16::from_be_bytes([ack_pkt[2], ack_pkt[3]]), blk);
        }

        // Wait briefly for worker flush
        thread::sleep(Duration::from_millis(80));
        assert_eq!(fs::read(dn_dir.join("uploaded.cfg")).unwrap(), wrq_payload);
        assert!(events.lock().unwrap().len() >= 2);

        server.stop();
        let _ = fs::remove_dir_all(&dir);
    }
}
