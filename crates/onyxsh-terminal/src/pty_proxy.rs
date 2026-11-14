//! POSIX PTY Stream Proxy for real-time OSC 133 / OSC 7 / OSC 52 parsing,
//! alternate-screen detection, and non-blocking `HighlightEngine` output colorization.

use crate::highlighter::{highlight_config_revision, HighlightConfig, HighlightEngine};
use crate::osc52::parse_osc52_sequences;
use crate::osc7::create_display_path;
use crate::spawner::PreparedLaunch;
use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};

/// Semantic and control events extracted live from the child PTY output stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscStreamEvent {
    PromptStart,
    CommandStart,
    CommandExecuted { command: Option<String> },
    CommandFinished {
        exit_code: i32,
        command: Option<String>,
        output: Option<String>,
    },
    CwdChanged { uri: String, display_path: String },
    ClipboardSet { target: String, text: String },
    AltScreenChanged(bool),
    ChildExited(i32),
}

const MAX_CAPTURED_COMMAND_BYTES: usize = 256 * 1024;

/// Chunk-safe state machine that intercepts OSC 133/7/52, tracks alternate screen state,
/// and applies `HighlightEngine` colorization to completed lines when safe.
pub struct PtyStreamProcessor {
    engine: HighlightEngine,
    last_highlight_rev: u64,
    active_command: Option<String>,
    command_in_flight: bool,
    captured_output: Vec<u8>,
    alt_screen: bool,
    enable_osc52: bool,
    osc_buf: Vec<u8>,
    in_osc: bool,
    esc_pending: bool,
    csi_buf: Vec<u8>,
    in_csi: bool,
    line_buf: Vec<u8>,
}

impl PtyStreamProcessor {
    pub fn new(enable_osc52: bool) -> Self {
        Self {
            engine: HighlightEngine::from_config(&HighlightConfig::load_default()),
            last_highlight_rev: highlight_config_revision(),
            active_command: None,
            command_in_flight: false,
            captured_output: Vec::new(),
            alt_screen: false,
            enable_osc52,
            osc_buf: Vec::new(),
            in_osc: false,
            esc_pending: false,
            csi_buf: Vec::new(),
            in_csi: false,
            line_buf: Vec::new(),
        }
    }

    pub fn with_engine(engine: HighlightEngine, enable_osc52: bool) -> Self {
        Self {
            engine,
            last_highlight_rev: highlight_config_revision(),
            active_command: None,
            command_in_flight: false,
            captured_output: Vec::new(),
            alt_screen: false,
            enable_osc52,
            osc_buf: Vec::new(),
            in_osc: false,
            esc_pending: false,
            csi_buf: Vec::new(),
            in_csi: false,
            line_buf: Vec::new(),
        }
    }

    pub fn is_alt_screen(&self) -> bool {
        self.alt_screen
    }

    pub fn set_active_command(&mut self, cmd: Option<String>) {
        self.active_command = cmd.filter(|s| !s.trim().is_empty());
    }

    pub fn maybe_reload_engine(&mut self) {
        let rev = highlight_config_revision();
        if rev != self.last_highlight_rev {
            self.engine = HighlightEngine::from_config(&HighlightConfig::load_default());
            self.last_highlight_rev = rev;
        }
    }

    /// Feeds raw bytes from child PTY master and returns processed bytes to write to VTE.
    /// Any incomplete line without trailing `\n` remains buffered briefly until `flush_pending()` runs.
    pub fn process_chunk(&mut self, input: &[u8], events: &mut Vec<OscStreamEvent>) -> Vec<u8> {
        self.maybe_reload_engine();
        let mut out = Vec::with_capacity(input.len() + 32);

        for &b in input {
            if self.in_osc {
                if self.esc_pending {
                    self.esc_pending = false;
                    if b == b'\\' {
                        // ST terminator (\x1b\\)
                        let payload = std::mem::take(&mut self.osc_buf);
                        self.in_osc = false;
                        self.handle_osc(&payload, b"\x1b\\", events, &mut out);
                        continue;
                    } else {
                        self.osc_buf.push(0x1b);
                        self.osc_buf.push(b);
                    }
                } else if b == 0x07 {
                    let payload = std::mem::take(&mut self.osc_buf);
                    self.in_osc = false;
                    self.handle_osc(&payload, b"\x07", events, &mut out);
                } else if b == 0x1b {
                    self.esc_pending = true;
                } else if self.osc_buf.len() < 128 * 1024 {
                    self.osc_buf.push(b);
                } else {
                    // Safety cap exceeded: abort OSC buffering cleanly
                    self.in_osc = false;
                    self.osc_buf.clear();
                }
                continue;
            }

            if self.in_csi {
                self.csi_buf.push(b);
                if (0x40..=0x7e).contains(&b) || self.csi_buf.len() > 64 {
                    let seq = std::mem::take(&mut self.csi_buf);
                    self.in_csi = false;
                    self.handle_csi(&seq, events);
                    self.line_buf.extend_from_slice(&seq);
                }
                continue;
            }

            if self.esc_pending {
                self.esc_pending = false;
                if b == b']' {
                    self.in_osc = true;
                    self.osc_buf.clear();
                    continue;
                } else if b == b'[' {
                    self.in_csi = true;
                    self.csi_buf.clear();
                    self.csi_buf.push(0x1b);
                    self.csi_buf.push(b'[');
                    continue;
                } else {
                    self.line_buf.push(0x1b);
                    self.line_buf.push(b);
                    continue;
                }
            }

            if b == 0x1b {
                self.esc_pending = true;
                continue;
            }

            if self.command_in_flight
                && !self.alt_screen
                && self.captured_output.len() < MAX_CAPTURED_COMMAND_BYTES
                && (b >= 0x20 || matches!(b, b'\n' | b'\r' | b'\t'))
            {
                self.captured_output.push(b);
            }

            self.line_buf.push(b);
            if b == b'\n' {
                self.flush_line(&mut out, true);
            }
        }

        // In alternate screen (vim/htop/less) or when high throughput isn't waiting on newline
        if self.alt_screen {
            self.flush_line(&mut out, false);
        }

        out
    }

    /// Flushes any pending incomplete line (e.g., interactive echo or shell prompt) immediately.
    pub fn flush_pending(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        if self.esc_pending {
            self.esc_pending = false;
            self.line_buf.push(0x1b);
        }
        self.flush_line(&mut out, false);
        out
    }

    fn flush_line(&mut self, out: &mut Vec<u8>, allow_highlight: bool) {
        if self.line_buf.is_empty() {
            return;
        }
        let raw = std::mem::take(&mut self.line_buf);
        if !allow_highlight || self.alt_screen || !self.engine.output_enabled {
            out.extend_from_slice(&raw);
            return;
        }

        if let Ok(line_str) = std::str::from_utf8(&raw) {
            // Preserve trailing \r\n cleanly while highlighting text body
            let (body, suffix) = if let Some(s) = line_str.strip_suffix("\r\n") {
                (s, "\r\n")
            } else if let Some(s) = line_str.strip_suffix('\n') {
                (s, "\n")
            } else {
                (line_str, "")
            };
            let highlighted = self
                .engine
                .highlight_line(body, self.active_command.as_deref());
            out.extend_from_slice(highlighted.as_bytes());
            out.extend_from_slice(suffix.as_bytes());
        } else {
            out.extend_from_slice(&raw);
        }
    }

    fn handle_csi(&mut self, seq: &[u8], events: &mut Vec<OscStreamEvent>) {
        let next_alt = match seq {
            b"\x1b[?1049h" | b"\x1b[?47h" | b"\x1b[?1047h" => Some(true),
            b"\x1b[?1049l" | b"\x1b[?47l" | b"\x1b[?1047l" => Some(false),
            _ => None,
        };
        if let Some(active) = next_alt {
            if self.alt_screen != active {
                self.alt_screen = active;
                events.push(OscStreamEvent::AltScreenChanged(active));
            }
        }
    }

    fn handle_osc(
        &mut self,
        payload: &[u8],
        terminator: &[u8],
        events: &mut Vec<OscStreamEvent>,
        out: &mut Vec<u8>,
    ) {
        let text = String::from_utf8_lossy(payload);
        if let Some(rest) = text.strip_prefix("133;") {
            if rest.starts_with('A') {
                self.active_command = None;
                self.command_in_flight = false;
                self.captured_output.clear();
                events.push(OscStreamEvent::PromptStart);
            } else if rest.starts_with('B') {
                events.push(OscStreamEvent::CommandStart);
            } else if let Some(cmd_part) = rest.strip_prefix('C') {
                let cmd = cmd_part
                    .strip_prefix(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
                self.active_command = cmd.clone();
                self.command_in_flight = true;
                self.captured_output.clear();
                events.push(OscStreamEvent::CommandExecuted { command: cmd });
            } else if let Some(d_part) = rest.strip_prefix('D') {
                let body = d_part.strip_prefix(';').unwrap_or("");
                let (code_str, d_cmd_opt) = match body.split_once(';') {
                    Some((c, c_rest)) => (
                        c,
                        Some(c_rest.trim().to_string()).filter(|s| !s.is_empty()),
                    ),
                    None => (body, None),
                };
                let exit_code = code_str.trim().parse::<i32>().ok().unwrap_or(0);
                let resolved_cmd = self.active_command.take().or(d_cmd_opt);
                let resolved_out = if self.command_in_flight {
                    self.command_in_flight = false;
                    let raw = std::mem::take(&mut self.captured_output);
                    let normalized = String::from_utf8_lossy(&raw)
                        .replace("\r\n", "\n")
                        .replace('\r', "\n");
                    Some(normalized.trim_end_matches(&['\n', '\r'][..]).to_string())
                } else {
                    self.captured_output.clear();
                    None
                };
                events.push(OscStreamEvent::CommandFinished {
                    exit_code,
                    command: resolved_cmd,
                    output: resolved_out,
                });
            }
            // Strip OSC 133 from line_buf flush or emit straight into out so order isn't tangled
            self.flush_line(out, false);
            out.extend_from_slice(b"\x1b]");
            out.extend_from_slice(payload);
            out.extend_from_slice(terminator);
            return;
        }

        if let Some(uri) = text.strip_prefix("7;") {
            let home = onyxsh_core::platform::home_dir();
            let home_str = home.to_string_lossy();
            let path_part = uri
                .strip_prefix("file://")
                .and_then(|s| s.find('/').map(|idx| &s[idx..]))
                .unwrap_or(uri);
            let display = create_display_path(path_part, &home_str);
            events.push(OscStreamEvent::CwdChanged {
                uri: uri.to_string(),
                display_path: display,
            });
            self.flush_line(out, false);
            out.extend_from_slice(b"\x1b]");
            out.extend_from_slice(payload);
            out.extend_from_slice(terminator);
            return;
        }

        if let Some(rest) = text.strip_prefix("52;") {
            // Enforce Zero-Trust OSC 52 filtering: reject '?' queries and strip from VTE stream
            if self.enable_osc52 {
                let probe = format!("\x1b]52;{rest}\x07");
                for (target, decoded) in
                    parse_osc52_sequences(probe.as_bytes(), crate::osc52::MAX_PAYLOAD_BYTES)
                {
                    events.push(OscStreamEvent::ClipboardSet {
                        target,
                        text: decoded,
                    });
                }
            }
            return;
        }

        // Unknown OSC (e.g. window title OSC 0/2): flush buffered line before passing straight through
        self.flush_line(out, false);
        out.extend_from_slice(b"\x1b]");
        out.extend_from_slice(payload);
        out.extend_from_slice(terminator);
    }
}

/// Control handle for a live proxied PTY session.
pub struct ProxiedPtyHandle {
    pub child_pid: i32,
    stop_flag: Arc<AtomicBool>,
    cmd_tx: mpsc::Sender<Option<String>>,
    worker: Option<JoinHandle<()>>,
}

impl ProxiedPtyHandle {
    /// Updates the active command name used to select among the 51 command highlight profiles.
    pub fn set_active_command(&self, cmd: Option<String>) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn shutdown(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ProxiedPtyHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        // Detach without blocking GTK main thread on drop if child still finishing
    }
}

fn open_pty_pair(raw_slave: bool) -> io::Result<(OwnedFd, OwnedFd)> {
    let mut master: RawFd = -1;
    let mut slave: RawFd = -1;
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    let master_fd = unsafe { OwnedFd::from_raw_fd(master) };
    let slave_fd = unsafe { OwnedFd::from_raw_fd(slave) };

    unsafe {
        libc::fcntl(master_fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
        libc::fcntl(slave_fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC);
        if raw_slave {
            let mut termios: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(slave_fd.as_raw_fd(), &mut termios) == 0 {
                libc::cfmakeraw(&mut termios);
                libc::tcsetattr(slave_fd.as_raw_fd(), libc::TCSANOW, &termios);
            }
        }
    }

    Ok((master_fd, slave_fd))
}

/// Spawns a child process under a dual-PTY proxy, returning `(vte_master_fd, handle)`.
/// The caller owns `vte_master_fd` (passed to `vte4::Pty::foreign_sync`) and receives all
/// [`OscStreamEvent`]s asynchronously over `event_tx`.
pub fn spawn_proxied_pty(
    launch: &PreparedLaunch,
    enable_osc52: bool,
    event_tx: mpsc::Sender<OscStreamEvent>,
) -> io::Result<(OwnedFd, ProxiedPtyHandle)> {
    if launch.argv.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Empty argv for PTY launch",
        ));
    }

    let (vte_master, vte_slave) = open_pty_pair(true)?;
    let (child_master, child_slave) = open_pty_pair(false)?;

    let argv_cstrings: Vec<CString> = launch
        .argv
        .iter()
        .map(|s| CString::new(s.as_str()).unwrap_or_else(|_| CString::new("").unwrap()))
        .collect();
    let cwd_cstring = CString::new(launch.working_directory.as_str())
        .unwrap_or_else(|_| CString::new("/").unwrap());

    let child_slave_raw = child_slave.as_raw_fd();
    let child_pid = unsafe { libc::fork() };
    if child_pid < 0 {
        return Err(io::Error::last_os_error());
    }

    if child_pid == 0 {
        // Child process
        drop(vte_master);
        drop(vte_slave);
        drop(child_master);
        unsafe {
            libc::setsid();
            libc::ioctl(child_slave_raw, libc::TIOCSCTTY as _, 0);
            libc::dup2(child_slave_raw, libc::STDIN_FILENO);
            libc::dup2(child_slave_raw, libc::STDOUT_FILENO);
            libc::dup2(child_slave_raw, libc::STDERR_FILENO);
            if child_slave_raw > 2 {
                libc::close(child_slave_raw);
            }
            for (k, v) in &launch.env {
                if let (Ok(ck), Ok(cv)) = (CString::new(k.as_str()), CString::new(v.as_str())) {
                    libc::setenv(ck.as_ptr(), cv.as_ptr(), 1);
                }
            }
            libc::chdir(cwd_cstring.as_ptr());

            let mut ptrs: Vec<*const libc::c_char> =
                argv_cstrings.iter().map(|c| c.as_ptr()).collect();
            ptrs.push(std::ptr::null());
            libc::execvp(ptrs[0], ptrs.as_ptr());
            libc::_exit(127);
        }
    }

    // Parent process
    drop(child_slave);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = Arc::clone(&stop_flag);
    let (cmd_tx, cmd_rx) = mpsc::channel::<Option<String>>();

    let worker = thread::Builder::new()
        .name(format!("onyx-pty-{child_pid}"))
        .spawn(move || {
            run_pty_bridge_loop(
                child_pid,
                child_master,
                vte_slave,
                enable_osc52,
                stop_clone,
                cmd_rx,
                event_tx,
            );
        })?;

    Ok((
        vte_master,
        ProxiedPtyHandle {
            child_pid,
            stop_flag,
            cmd_tx,
            worker: Some(worker),
        },
    ))
}

fn sync_pty_winsize(src_fd: RawFd, dst_fd: RawFd, prev: &mut (u16, u16)) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(src_fd, libc::TIOCGWINSZ as _, &mut ws) == 0
            && (ws.ws_row > 0 && ws.ws_col > 0)
            && (ws.ws_row != prev.0 || ws.ws_col != prev.1)
        {
            *prev = (ws.ws_row, ws.ws_col);
            libc::ioctl(dst_fd, libc::TIOCSWINSZ as _, &ws);
        }
    }
}

fn write_all_fd(fd: RawFd, data: &[u8]) -> bool {
    let mut offset = 0;
    while offset < data.len() {
        let n = unsafe {
            libc::write(
                fd,
                data[offset..].as_ptr() as *const libc::c_void,
                data.len() - offset,
            )
        };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if err.kind() == io::ErrorKind::WouldBlock {
                std::thread::sleep(std::time::Duration::from_millis(1));
                continue;
            }
            return false;
        }
        offset += n as usize;
    }
    true
}

fn run_pty_bridge_loop(
    child_pid: i32,
    child_master: OwnedFd,
    vte_slave: OwnedFd,
    enable_osc52: bool,
    stop_flag: Arc<AtomicBool>,
    cmd_rx: mpsc::Receiver<Option<String>>,
    event_tx: mpsc::Sender<OscStreamEvent>,
) {
    let cm_raw = child_master.as_raw_fd();
    let vs_raw = vte_slave.as_raw_fd();
    unsafe {
        libc::fcntl(cm_raw, libc::F_SETFL, libc::O_NONBLOCK);
        libc::fcntl(vs_raw, libc::F_SETFL, libc::O_NONBLOCK);
    }

    let mut proc_state = PtyStreamProcessor::new(enable_osc52);
    let mut buf = [0u8; 8192];
    let mut last_winsize = (0u16, 0u16);
    let mut events = Vec::new();
    let mut child_alive = true;

    while !stop_flag.load(Ordering::Relaxed) {
        while let Ok(cmd) = cmd_rx.try_recv() {
            proc_state.set_active_command(cmd);
        }
        sync_pty_winsize(vs_raw, cm_raw, &mut last_winsize);

        let mut fds = [
            libc::pollfd {
                fd: cm_raw,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: vs_raw,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 2, 15) };
        if ready < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }

        if ready == 0 {
            // 15ms timeout: flush any pending prompt or interactive fragment un-buffered
            let flushed = proc_state.flush_pending();
            if !flushed.is_empty() && !write_all_fd(vs_raw, &flushed) {
                break;
            }
            // Check non-blocking child exit status
            let mut status: libc::c_int = 0;
            let waited = unsafe { libc::waitpid(child_pid, &mut status, libc::WNOHANG) };
            if waited == child_pid {
                child_alive = false;
                let code = if libc::WIFEXITED(status) {
                    libc::WEXITSTATUS(status)
                } else {
                    1
                };
                let _ = event_tx.send(OscStreamEvent::ChildExited(code));
                break;
            }
            continue;
        }

        // 1. Child stdout/stderr -> Processor -> VTE slave
        if (fds[0].revents & libc::POLLIN) != 0 {
            loop {
                let n = unsafe { libc::read(cm_raw, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n > 0 {
                    events.clear();
                    let processed = proc_state.process_chunk(&buf[..n as usize], &mut events);
                    for ev in events.drain(..) {
                        let _ = event_tx.send(ev);
                    }
                    if !processed.is_empty() && !write_all_fd(vs_raw, &processed) {
                        stop_flag.store(true, Ordering::Relaxed);
                        break;
                    }
                } else if n == 0 {
                    break;
                } else {
                    let err = io::Error::last_os_error();
                    if err.kind() != io::ErrorKind::WouldBlock && err.kind() != io::ErrorKind::Interrupted {
                        break;
                    }
                    break;
                }
            }
        }

        // 2. VTE keyboard/input -> Child master
        if (fds[1].revents & libc::POLLIN) != 0 {
            loop {
                let n = unsafe { libc::read(vs_raw, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n > 0 {
                    if !write_all_fd(cm_raw, &buf[..n as usize]) {
                        stop_flag.store(true, Ordering::Relaxed);
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        if (fds[0].revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL)) != 0
            || (fds[1].revents & (libc::POLLERR | libc::POLLNVAL)) != 0
        {
            let flushed = proc_state.flush_pending();
            if !flushed.is_empty() {
                let _ = write_all_fd(vs_raw, &flushed);
            }
            break;
        }
    }

    if child_alive {
        let mut status: libc::c_int = 0;
        let mut waited = 0;
        for _ in 0..15 {
            waited = unsafe { libc::waitpid(child_pid, &mut status, libc::WNOHANG) };
            if waited != 0 || stop_flag.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if waited == child_pid {
            let code = if libc::WIFEXITED(status) {
                libc::WEXITSTATUS(status)
            } else {
                1
            };
            let _ = event_tx.send(OscStreamEvent::ChildExited(code));
        } else {
            let _ = event_tx.send(OscStreamEvent::ChildExited(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    #[test]
    fn test_chunk_split_osc_parsing_and_alt_screen_bypass() {
        let mut proc = PtyStreamProcessor::new(true);
        let mut events = Vec::new();

        // Split OSC 133;A and OSC 7 across chunks
        let out1 = proc.process_chunk(b"\x1b]133;A\x07\x1b]7;file://host", &mut events);
        let out2 = proc.process_chunk(b"/var/log\x07\x1b]133;C;git status\x07", &mut events);
        let out3 = proc.process_chunk(b"@@ -1,2 +1,3 @@\r\n", &mut events);
        let mut total_out = Vec::new();
        total_out.extend(out1);
        total_out.extend(out2);
        total_out.extend(out3);

        assert!(events.contains(&OscStreamEvent::PromptStart));
        assert!(events.iter().any(|e| matches!(e, OscStreamEvent::CwdChanged { display_path, .. } if display_path == "/var/log")));
        assert!(events.iter().any(|e| matches!(e, OscStreamEvent::CommandExecuted { command: Some(c) } if c == "git status")));
        let out_str = String::from_utf8_lossy(&total_out);
        assert!(out_str.contains("\x1b["));

        // OSC 52 clipboard intercept strips payload from VTE and decodes safely
        events.clear();
        let b64 = base64::engine::general_purpose::STANDARD.encode("hello-osc52");
        let osc52 = format!("\x1b]52;c;{b64}\x07\x1b]52;c;?\x07");
        let stripped = proc.process_chunk(osc52.as_bytes(), &mut events);
        assert!(!String::from_utf8_lossy(&stripped).contains("]52;"));
        assert_eq!(
            events,
            vec![OscStreamEvent::ClipboardSet {
                target: "c".to_string(),
                text: "hello-osc52".to_string()
            }]
        );

        // Alternate screen bypasses highlighting & sends AltScreenChanged
        events.clear();
        let alt_in = proc.process_chunk(b"\x1b[?1049hERROR inside vim\n", &mut events);
        assert!(proc.is_alt_screen());
        assert!(events.contains(&OscStreamEvent::AltScreenChanged(true)));
        assert_eq!(String::from_utf8_lossy(&alt_in), "\x1b[?1049hERROR inside vim\n");

        events.clear();
        let _ = proc.process_chunk(b"\x1b[?1049l\x1b]133;D;0\x07", &mut events);
        assert!(!proc.is_alt_screen());
        assert!(events.iter().any(|e| matches!(
            e,
            OscStreamEvent::CommandFinished { exit_code: 0, .. }
        )));

        // Fast command with ANSI coloring between 133;C and 133;D;0;uname -a captures exact output
        events.clear();
        let _ = proc.process_chunk(
            b"\x1b]133;A\x07user@host:~$ uname -a\r\n\x1b]133;C\x07\x1b[32mLinux host 7.0\x1b[0m\r\n\x1b]133;D;0;uname -a\x07\x1b]133;A\x07user@host:~$ ",
            &mut events,
        );
        assert!(events.contains(&OscStreamEvent::CommandFinished {
            exit_code: 0,
            command: Some("uname -a".to_string()),
            output: Some("Linux host 7.0".to_string()),
        }));

        // Empty Enter emits 133;D without 133;C -> output is None so stale command doesn't overwrite
        events.clear();
        let _ = proc.process_chunk(b"\r\n\x1b]133;D;0;uname -a\x07\x1b]133;A\x07", &mut events);
        assert!(events.contains(&OscStreamEvent::CommandFinished {
            exit_code: 0,
            command: Some("uname -a".to_string()),
            output: None,
        }));
    }
}
