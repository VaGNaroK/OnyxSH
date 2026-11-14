//! Quick Look preview dialog and integrated in-place editor for OnyxSH File Manager.
//!
//! Ports `src/onyxsh/filemanager/quick_look.py` to native Rust (GTK4 + Libadwaita).
//!
//! Provides:
//! - File inspection & text/code viewer with monospace typography and line metrics.
//! - In-place text editor (toggled via Edit button or Ctrl+E).
//! - Normal Save (`Ctrl+S`) and Superuser / Root Save (`Ctrl+Shift+S`) via `pkexec tee`.
//! - HeaderBar action controls matching the OnyxSH specification:
//!   - Edit mode toggle button (`document-edit-symbolic`)
//!   - Suggested Save button (`document-save-symbolic`)
//!   - Superuser Save button (`security-high-symbolic`)
//!   - More Options popover menu (`view-more-symbolic`):
//!     - "Abrir com aplicativo externo"
//!     - "Explicar com IA"
//!     - "Calcular / Verificar Hash..."
//!     - "Copiar Conteúdo"
//!     - "Recarregar do Disco"
//! - Bottom status bar with `{lines} linhas • {size} • {mime} • UTF-8` and `"Modo Edição"` badge pill.
//! - Dirty state detection (`●` marker in title and discard confirmation on close/navigate).

use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::i18n::{tr, tr_fmt};
use onyxsh_core::platform::CommandBuilder;
use onyxsh_filemanager::backend::{FileSystemBackend, SftpFsBackend};
use onyxsh_filemanager::quick_look::{
    classify_log_line_severity, format_hex_dump_page, highlight_code_spans, load_local_preview,
    load_preview_from_bytes, select_quick_look_entry, LogSeverity, PreviewKind,
};
use onyxsh_filemanager::FileEntry;
use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const HEX_PAGE_SIZE: usize = 4096;
const REMOTE_TEXT_MAX_BYTES: usize = 2 * 1024 * 1024;
const REMOTE_IMAGE_MAX_BYTES: usize = 16 * 1024 * 1024;
const REMOTE_BINARY_MAX_BYTES: usize = 512 * 1024;

/// Backend target for Quick Look preview & edit operations.
#[derive(Clone)]
pub enum QuickLookBackend {
    Local,
    Remote(SftpFsBackend),
}

static REMOTE_TEMP_SEQ: AtomicU64 = AtomicU64::new(1);
static TRACKED_REMOTE_TEMP_FILES: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();

fn tracked_temp_store() -> &'static Mutex<Vec<PathBuf>> {
    TRACKED_REMOTE_TEMP_FILES.get_or_init(|| Mutex::new(Vec::new()))
}

fn remote_temp_cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("onyxsh-remote")
}

fn create_remote_temp_file(filename: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let dir = remote_temp_cache_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe_name: String = filename
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' })
        .collect();
    let seq = REMOTE_TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let path = dir.join(format!("{}_{}_{}", std::process::id(), seq, safe_name));
    fs::write(&path, bytes).map_err(|e| e.to_string())?;
    if let Ok(mut guard) = tracked_temp_store().lock() {
        guard.push(path.clone());
    }
    Ok(path)
}

/// Removes all tracked remote temporary cache files created by Quick Look sessions.
pub fn cleanup_remote_temp_files() {
    if let Ok(mut guard) = tracked_temp_store().lock() {
        for p in guard.drain(..) {
            let _ = fs::remove_file(&p);
        }
    }
    let dir = remote_temp_cache_dir();
    if dir.exists() {
        let _ = fs::remove_dir(&dir);
    }
}

/// Converts a UTF-8 byte offset within `line` into a GTK character column index.
pub fn byte_offset_to_char_offset(line: &str, byte_idx: usize) -> i32 {
    let mut clamped = byte_idx.min(line.len());
    while clamped > 0 && !line.is_char_boundary(clamped) {
        clamped -= 1;
    }
    line[..clamped].chars().count() as i32
}

/// Returns true if a file path appears to be a structured/system log file.
pub fn is_log_file_path(path: &Path) -> bool {
    let lower = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    lower.ends_with(".log")
        || lower.contains(".log.")
        || matches!(lower.as_str(), "syslog" | "messages" | "dmesg" | "kern.log" | "auth.log")
}

/// Clamps an interactive image zoom factor within safe display bounds (`0.1`..=`8.0`).
pub fn clamp_image_zoom(scale: f64) -> f64 {
    if !scale.is_finite() {
        1.0
    } else {
        scale.clamp(0.1, 8.0)
    }
}

/// Computes `(total_pages, start_offset, end_offset)` for paginated Hex Dump rendering.
pub fn compute_hex_page_bounds(total_bytes: u64, page: usize, page_size: usize) -> (usize, usize, usize) {
    let ps = page_size.max(1) as u64;
    let total_pages = if total_bytes == 0 {
        1
    } else {
        ((total_bytes - 1) / ps + 1) as usize
    };
    let clamped_page = page.min(total_pages.saturating_sub(1));
    let start = (clamped_page as u64).saturating_mul(ps);
    let end = start.saturating_add(ps).min(total_bytes);
    (total_pages, start as usize, end as usize)
}

fn read_local_hex_slice(path: &Path, offset: usize, len: usize) -> std::io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset as u64))?;
    let mut buf = vec![0u8; len];
    let n = file.read(&mut buf)?;
    buf.truncate(n);
    Ok(buf)
}

fn apply_buffer_highlighting(
    buffer: &gtk4::TextBuffer,
    path: &Path,
    enabled: bool,
    dark_theme: bool,
) {
    let (start_all, end_all) = (buffer.start_iter(), buffer.end_iter());
    buffer.remove_all_tags(&start_all, &end_all);
    if !enabled {
        return;
    }

    let text = buffer.text(&start_all, &end_all, true).to_string();
    if text.is_empty() || text.len() > 512 * 1024 {
        return;
    }

    let tag_table = buffer.tag_table();
    let get_or_create_tag = |name: &str, fg: &str, bold: bool, italic: bool| -> gtk4::TextTag {
        if let Some(existing) = tag_table.lookup(name) {
            return existing;
        }
        let tag = gtk4::TextTag::new(Some(name));
        tag.set_foreground(Some(fg));
        if bold {
            tag.set_weight(700);
        }
        if italic {
            tag.set_style(gtk4::pango::Style::Italic);
        }
        tag_table.add(&tag);
        tag
    };

    if is_log_file_path(path) {
        for (line_idx, line) in text.split_inclusive('\n').enumerate() {
            let sev = classify_log_line_severity(line);
            let (tag_opt, fg, bold) = match sev {
                LogSeverity::Error => (
                    Some("onyx-log-err"),
                    if dark_theme { "#ff6b6b" } else { "#c92a2a" },
                    true,
                ),
                LogSeverity::Warning => (
                    Some("onyx-log-warn"),
                    if dark_theme { "#ffd166" } else { "#d97706" },
                    true,
                ),
                LogSeverity::Info => (
                    Some("onyx-log-info"),
                    if dark_theme { "#6bc5ff" } else { "#1971c2" },
                    false,
                ),
                LogSeverity::Normal => (None, "", false),
            };
            if let Some(tag_name) = tag_opt {
                let tag = get_or_create_tag(tag_name, fg, bold, false);
                if let Some(s_it) = buffer.iter_at_line(line_idx as i32) {
                    let line_chars = line.trim_end_matches(['\r', '\n']).chars().count() as i32;
                    if let Some(e_it) = buffer.iter_at_line_offset(line_idx as i32, line_chars) {
                        buffer.apply_tag(&tag, &s_it, &e_it);
                    }
                }
            }
        }
        return;
    }

    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let spans = highlight_code_spans(path, &text, dark_theme);
    for span in spans {
        let Some(line_str) = lines.get(span.line_index) else {
            continue;
        };
        let line_clean = line_str.trim_end_matches(['\r', '\n']);
        let line_char_len = line_clean.chars().count() as i32;
        let c0 = byte_offset_to_char_offset(line_clean, span.start_byte).min(line_char_len);
        let c1 = byte_offset_to_char_offset(line_clean, span.end_byte).min(line_char_len);
        if c1 <= c0 {
            continue;
        }
        let key = format!("onyx-syn:{}:{}:{}", span.fg_hex, span.bold, span.italic);
        let tag = get_or_create_tag(&key, &span.fg_hex, span.bold, span.italic);
        if let (Some(s_it), Some(e_it)) = (
            buffer.iter_at_line_offset(span.line_index as i32, c0),
            buffer.iter_at_line_offset(span.line_index as i32, c1),
        ) {
            buffer.apply_tag(&tag, &s_it, &e_it);
        }
    }
}

/// Guesses standard MIME type string from filename extension.
pub fn guess_mime_type(filename: &str) -> &'static str {
    let lower = filename.to_ascii_lowercase();
    let ext = Path::new(&lower)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    match ext {
        "py" => "text/x-python",
        "rs" => "text/rust",
        "sh" | "bash" | "zsh" => "text/x-shellscript",
        "js" | "mjs" | "cjs" => "text/javascript",
        "ts" => "text/typescript",
        "json" => "application/json",
        "yaml" | "yml" => "text/yaml",
        "toml" => "text/toml",
        "md" | "markdown" => "text/markdown",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "xml" | "svg" => "text/xml",
        "c" | "h" => "text/x-c",
        "cpp" | "cxx" | "hpp" => "text/x-c++",
        "java" => "text/x-java",
        "go" => "text/x-go",
        "sql" => "text/x-sql",
        "php" => "text/x-php",
        "rb" => "text/x-ruby",
        "lua" => "text/x-lua",
        "log" => "text/x-log",
        "txt" | "conf" | "cfg" | "ini" | "env" => "text/plain",
        _ => "text/plain",
    }
}

fn build_host_command(raw_args: &[&str]) -> std::process::Command {
    let owned: Vec<String> = raw_args.iter().map(|s| (*s).to_string()).collect();
    let wrapped = CommandBuilder::new().wrap_host_command(&owned, None, None);
    let mut cmd = std::process::Command::new(&wrapped[0]);
    if wrapped.len() > 1 {
        cmd.args(&wrapped[1..]);
    }
    cmd
}

enum RootSaveOutcome {
    Saved,
    UserCancelled,
    NeedSudoFallback(String),
    Error(String),
}

fn write_stdin_and_wait(mut cmd: std::process::Command, stdin_data: &[u8]) -> std::io::Result<std::process::Output> {
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_data)?;
    }
    child.wait_with_output()
}

/// Attempts elevated root file write via `pkexec tee`, signaling when `sudo -S` fallback should be used.
fn save_file_as_root_pkexec(path: &Path, content: &str) -> RootSaveOutcome {
    let path_str = path.to_string_lossy().to_string();
    let cmd = build_host_command(&["pkexec", "tee", &path_str]);
    match write_stdin_and_wait(cmd, content.as_bytes()) {
        Ok(out) if out.status.success() => RootSaveOutcome::Saved,
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let lower = stderr.to_ascii_lowercase();
            if code == 126 && lower.contains("dismiss") {
                return RootSaveOutcome::UserCancelled;
            }
            if code == 126 || code == 127 || lower.contains("authentication agent") || lower.contains("polkit") || stderr.is_empty() {
                return RootSaveOutcome::NeedSudoFallback(stderr);
            }
            RootSaveOutcome::Error(stderr)
        }
        Err(e) => RootSaveOutcome::NeedSudoFallback(e.to_string()),
    }
}

fn try_sudo_with_password(path: &Path, content: &str, password: Option<&str>) -> Result<(), String> {
    let path_str = path.to_string_lossy().to_string();
    if let Some(pw) = password {
        let mut pw_line = format!("{pw}\n");
        let val_cmd = build_host_command(&["sudo", "-S", "-k", "-p", "", "-v"]);
        let val_out = write_stdin_and_wait(val_cmd, pw_line.as_bytes())
            .map_err(|e| tr_fmt("Falha ao executar sudo: {err}", &[("err", &e.to_string())]))?;
        unsafe {
            pw_line.as_bytes_mut().fill(0);
        }
        if !val_out.status.success() {
            let stderr = String::from_utf8_lossy(&val_out.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                tr("Senha sudo incorreta ou não autorizada.")
            } else {
                stderr
            });
        }
    }
    let write_cmd = build_host_command(&["sudo", "-n", "tee", &path_str]);
    let write_out = write_stdin_and_wait(write_cmd, content.as_bytes())
        .map_err(|e| tr_fmt("Falha ao gravar via sudo: {err}", &[("err", &e.to_string())]))?;
    if write_out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&write_out.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            tr("sudo recusou a gravação do arquivo.")
        } else {
            stderr
        })
    }
}

fn prompt_sudo_password_and_save(
    parent: &adw::Window,
    path: PathBuf,
    content: String,
    fallback_reason: String,
    on_done: Rc<dyn Fn(Result<(), String>)>,
) {
    let dlg = adw::Window::builder()
        .title(&tr("Autenticação Sudo Necessária"))
        .transient_for(parent)
        .modal(true)
        .default_width(420)
        .build();
    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    let hdr = adw::HeaderBar::new();
    root.append(&hdr);

    let body = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    body.set_margin_start(18);
    body.set_margin_end(18);
    body.set_margin_top(10);
    body.set_margin_bottom(16);

    let desc = gtk4::Label::new(Some(&tr_fmt(
        "Digite sua senha sudo para gravar '{path}' com privilégios de administrador.",
        &[("path", &path.display().to_string())],
    )));
    desc.set_wrap(true);
    desc.set_halign(gtk4::Align::Start);
    body.append(&desc);

    if !fallback_reason.is_empty() {
        let note = gtk4::Label::new(Some(&tr_fmt(
            "Fallback via sudo -S (pkexec indisponível: {reason})",
            &[("reason", &fallback_reason)],
        )));
        note.add_css_class("caption");
        note.add_css_class("dim-label");
        note.set_wrap(true);
        note.set_halign(gtk4::Align::Start);
        body.append(&note);
    }

    let pw_entry = gtk4::PasswordEntry::builder()
        .show_peek_icon(true)
        .hexpand(true)
        .build();
    body.append(&pw_entry);

    let btn_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_row.set_halign(gtk4::Align::End);
    let btn_cancel = gtk4::Button::with_label(&tr("Cancelar"));
    let btn_ok = gtk4::Button::with_label(&tr("Autenticar e Salvar"));
    btn_ok.add_css_class("suggested-action");
    btn_row.append(&btn_cancel);
    btn_row.append(&btn_ok);
    body.append(&btn_row);
    root.append(&body);
    dlg.set_content(Some(&root));

    {
        let dlg_c = dlg.clone();
        btn_cancel.connect_clicked(move |_| dlg_c.close());
    }
    let trigger_save: Rc<dyn Fn()> = {
        let dlg_c = dlg.clone();
        let pw_c = pw_entry.clone();
        let btn_c = btn_ok.clone();
        let path_c = path.clone();
        let content_c = content.clone();
        let done_c = Rc::clone(&on_done);
        Rc::new(move || {
            let pw = pw_c.text().to_string();
            if pw.is_empty() {
                return;
            }
            btn_c.set_sensitive(false);
            let p_bg = path_c.clone();
            let c_bg = content_c.clone();
            let dlg_done = dlg_c.clone();
            let btn_done = btn_c.clone();
            let cb_done = Rc::clone(&done_c);
            let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
            std::thread::spawn(move || {
                let res = try_sudo_with_password(&p_bg, &c_bg, Some(&pw));
                let _ = tx.send(res);
            });
            glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
                Ok(res) => {
                    btn_done.set_sensitive(true);
                    match res {
                        Ok(()) => {
                            dlg_done.close();
                            cb_done(Ok(()));
                        }
                        Err(e) => cb_done(Err(e)),
                    }
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    btn_done.set_sensitive(true);
                    glib::ControlFlow::Break
                }
            });
        })
    };
    {
        let trig = Rc::clone(&trigger_save);
        btn_ok.connect_clicked(move |_| trig());
    }
    {
        let trig = Rc::clone(&trigger_save);
        pw_entry.connect_activate(move |_| trig());
    }
    dlg.present();
    pw_entry.grab_focus();
}

/// Opens the modern Quick Look modal preview and in-place editor dialog (Local backend).
pub fn show_quick_look_dialog(
    parent: &adw::ApplicationWindow,
    entries: Vec<FileEntry>,
    initial_index: usize,
    feed_terminal_cb: Option<Rc<dyn Fn(&str)>>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
    refresh_fn: Option<Rc<dyn Fn()>>,
) {
    show_quick_look_dialog_with_backend(
        parent,
        entries,
        initial_index,
        QuickLookBackend::Local,
        true,
        feed_terminal_cb,
        ai_prompt_cb,
        refresh_fn,
    );
}

/// Opens the modern Quick Look modal preview and in-place editor dialog for a Local or SFTP backend.
#[allow(clippy::too_many_arguments)]
pub fn show_quick_look_dialog_with_backend(
    parent: &adw::ApplicationWindow,
    entries: Vec<FileEntry>,
    initial_index: usize,
    backend: QuickLookBackend,
    clear_remote_on_exit: bool,
    feed_terminal_cb: Option<Rc<dyn Fn(&str)>>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
    refresh_fn: Option<Rc<dyn Fn()>>,
) {
    if entries.is_empty() {
        return;
    }

    let current_idx = Rc::new(RefCell::new(initial_index.min(entries.len() - 1)));
    let entries_rc = Rc::new(entries);

    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Quick Look"))
        .default_width(840)
        .default_height(620)
        .build();

    let toolbar_view = adw::ToolbarView::new();
    let header_bar = adw::HeaderBar::new();

    // ── 1. HeaderBar Start: Navigation Controls (< Prev, Next >, counter) ────
    let nav_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    nav_box.add_css_class("linked");
    let btn_prev = gtk4::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text(&tr("Arquivo Anterior (Seta Cima / k)"))
        .build();
    let btn_next = gtk4::Button::builder()
        .icon_name("go-next-symbolic")
        .tooltip_text(&tr("Próximo Arquivo (Seta Baixo / j)"))
        .build();
    nav_box.append(&btn_prev);
    nav_box.append(&btn_next);
    header_bar.pack_start(&nav_box);

    let pos_lbl = gtk4::Label::new(None);
    pos_lbl.add_css_class("dim-label");
    pos_lbl.add_css_class("caption");
    pos_lbl.add_css_class("monospace");
    pos_lbl.set_margin_start(8);
    header_bar.pack_start(&pos_lbl);

    // ── 2. HeaderBar Center: Title & Subtitle ─────────────────────────────────
    let title_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    title_box.set_valign(gtk4::Align::Center);

    let title_lbl = gtk4::Label::builder()
        .css_classes(["title-4"])
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .xalign(0.5)
        .build();

    let subtitle_lbl = gtk4::Label::builder()
        .css_classes(["caption", "dim-label"])
        .ellipsize(gtk4::pango::EllipsizeMode::Middle)
        .xalign(0.5)
        .build();

    title_box.append(&title_lbl);
    title_box.append(&subtitle_lbl);
    header_bar.set_title_widget(Some(&title_box));

    // ── 3. HeaderBar End: Edit, Save, Root Save, More Options Popover ────────
    let popover = gtk4::Popover::new();
    popover.add_css_class("onyxsh-popover");

    let popover_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    popover_vbox.set_margin_start(6);
    popover_vbox.set_margin_end(6);
    popover_vbox.set_margin_top(6);
    popover_vbox.set_margin_bottom(6);
    popover.set_child(Some(&popover_vbox));

    let menu_btn = gtk4::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(&tr("More Options"))
        .popover(&popover)
        .build();
    menu_btn.add_css_class("flat");

    let save_sudo_btn = gtk4::Button::builder()
        .icon_name("security-high-symbolic")
        .tooltip_text(&tr("Salvar com privilégios de Administrador / Root (Ctrl+Shift+S)"))
        .visible(false)
        .build();
    save_sudo_btn.add_css_class("flat");

    let save_btn = gtk4::Button::builder()
        .icon_name("document-save-symbolic")
        .tooltip_text(&tr("Save changes (Ctrl+S)"))
        .sensitive(false)
        .visible(false)
        .build();
    save_btn.add_css_class("suggested-action");

    let edit_toggle_btn = gtk4::ToggleButton::builder()
        .icon_name("document-edit-symbolic")
        .tooltip_text(&tr("Alternar Modo Edição (Ctrl+E)"))
        .build();
    edit_toggle_btn.add_css_class("flat");

    let syntax_toggle_btn = gtk4::ToggleButton::builder()
        .icon_name("applications-engineering-symbolic")
        .tooltip_text(&tr("Realce de Sintaxe / Severidade de Logs"))
        .active(true)
        .visible(false)
        .build();
    syntax_toggle_btn.add_css_class("flat");

    // Packing into headerbar end (order renders: [syntax] [edit] [save] [save_sudo] [menu] [close])
    header_bar.pack_end(&menu_btn);
    header_bar.pack_end(&save_sudo_btn);
    header_bar.pack_end(&save_btn);
    header_bar.pack_end(&edit_toggle_btn);
    header_bar.pack_end(&syntax_toggle_btn);

    toolbar_view.add_top_bar(&header_bar);

    let toast_overlay = adw::ToastOverlay::new();

    // ── 4. Main Body: Stack (Text View, Image, Binary, Directory) ───────────
    let body_stack = gtk4::Stack::new();
    body_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);

    // Page: Text View Container
    let text_container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

    let readonly_banner = adw::Banner::builder()
        .title(&tr("Este arquivo é somente leitura. Edições podem ser salvas com privilégios de Administrador (Root)."))
        .revealed(false)
        .build();
    text_container.append(&readonly_banner);

    let text_scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();

    let text_view = gtk4::TextView::builder()
        .editable(false)
        .cursor_visible(true)
        .monospace(true)
        .left_margin(16)
        .right_margin(16)
        .top_margin(12)
        .bottom_margin(12)
        .build();
    text_view.add_css_class("quick-look-text-view");
    text_scroller.set_child(Some(&text_view));
    text_container.append(&text_scroller);

    // Bottom info bar (lines, size, mime, encoding, and edit badge)
    let bottom_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    bottom_bar.set_margin_start(16);
    bottom_bar.set_margin_end(16);
    bottom_bar.set_margin_top(6);
    bottom_bar.set_margin_bottom(6);

    let text_info_label = gtk4::Label::builder()
        .hexpand(true)
        .xalign(0.0)
        .css_classes(["caption", "dim-label"])
        .build();

    let mode_badge = gtk4::Label::builder()
        .label(&tr("Preview Mode"))
        .css_classes(["badge-pill", "caption", "dim-label"])
        .build();

    bottom_bar.append(&text_info_label);
    bottom_bar.append(&mode_badge);
    text_container.append(&bottom_bar);

    body_stack.add_named(&text_container, Some("text"));

    // Page: Image View with Interactive Zoom & Pan
    let image_container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let image_scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();
    let picture = gtk4::Picture::builder()
        .can_shrink(true)
        .halign(gtk4::Align::Center)
        .valign(gtk4::Align::Center)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    image_scroller.set_child(Some(&picture));
    image_container.append(&image_scroller);

    let zoom_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    zoom_bar.set_margin_start(12);
    zoom_bar.set_margin_end(12);
    zoom_bar.set_margin_top(6);
    zoom_bar.set_margin_bottom(6);
    let btn_zoom_out = gtk4::Button::from_icon_name("zoom-out-symbolic");
    btn_zoom_out.add_css_class("flat");
    btn_zoom_out.set_tooltip_text(Some(&tr("Diminuir zoom")));
    let btn_zoom_in = gtk4::Button::from_icon_name("zoom-in-symbolic");
    btn_zoom_in.add_css_class("flat");
    btn_zoom_in.set_tooltip_text(Some(&tr("Aumentar zoom")));
    let btn_zoom_fit = gtk4::Button::with_label(&tr("Ajustar"));
    btn_zoom_fit.add_css_class("flat");
    let btn_zoom_100 = gtk4::Button::with_label("100%");
    btn_zoom_100.add_css_class("flat");
    let zoom_pct_lbl = gtk4::Label::new(Some(&tr("Ajustar")));
    zoom_pct_lbl.add_css_class("caption");
    zoom_pct_lbl.add_css_class("monospace");
    zoom_pct_lbl.set_hexpand(true);
    zoom_pct_lbl.set_xalign(1.0);
    zoom_bar.append(&btn_zoom_out);
    zoom_bar.append(&btn_zoom_in);
    zoom_bar.append(&btn_zoom_fit);
    zoom_bar.append(&btn_zoom_100);
    zoom_bar.append(&zoom_pct_lbl);
    image_container.append(&zoom_bar);
    body_stack.add_named(&image_container, Some("image"));

    // Page: Binary / Directory / Metadata Card + Paginated Hex Dump
    let binary_container = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    binary_container.set_margin_top(14);
    binary_container.set_margin_bottom(12);
    binary_container.set_margin_start(14);
    binary_container.set_margin_end(14);

    let binary_card = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    binary_card.set_valign(gtk4::Align::Start);
    binary_card.set_halign(gtk4::Align::Center);
    binary_container.append(&binary_card);

    let hex_toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    hex_toolbar.set_visible(false);
    let btn_hex_prev = gtk4::Button::with_label(&tr("< Anterior"));
    btn_hex_prev.add_css_class("flat");
    let hex_page_lbl = gtk4::Label::new(None);
    hex_page_lbl.add_css_class("caption");
    hex_page_lbl.add_css_class("monospace");
    hex_page_lbl.set_hexpand(true);
    hex_page_lbl.set_halign(gtk4::Align::Center);
    let btn_hex_next = gtk4::Button::with_label(&tr("Próxima >"));
    btn_hex_next.add_css_class("flat");
    hex_toolbar.append(&btn_hex_prev);
    hex_toolbar.append(&hex_page_lbl);
    hex_toolbar.append(&btn_hex_next);
    binary_container.append(&hex_toolbar);

    let hex_scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .visible(false)
        .build();
    let hex_view = gtk4::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .monospace(true)
        .left_margin(12)
        .right_margin(12)
        .top_margin(10)
        .bottom_margin(10)
        .build();
    hex_view.add_css_class("quick-look-text-view");
    hex_scroller.set_child(Some(&hex_view));
    binary_container.append(&hex_scroller);

    body_stack.add_named(&binary_container, Some("binary"));

    toast_overlay.set_child(Some(&body_stack));
    toolbar_view.set_content(Some(&toast_overlay));
    dlg.set_content(Some(&toolbar_view));

    // ── 5. State Tracking ───────────────────────────────────────────────────
    let is_editing_rc = Rc::new(RefCell::new(false));
    let is_dirty_rc = Rc::new(RefCell::new(false));
    let original_content_rc = Rc::new(RefCell::new(String::new()));
    let is_loading_buffer = Rc::new(RefCell::new(false));
    let render_gen = Rc::new(Cell::new(0u64));
    let backend_rc = Rc::new(backend);
    let dialog_temp_files: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    let cached_remote_text_files: Rc<RefCell<std::collections::HashMap<usize, PathBuf>>> =
        Rc::new(RefCell::new(std::collections::HashMap::new()));
    let zoom_scale_rc: Rc<Cell<Option<f64>>> = Rc::new(Cell::new(None));
    let natural_img_size_rc: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));
    let hex_state_rc: Rc<RefCell<(u64, usize, Option<PathBuf>, Option<Vec<u8>>)>> =
        Rc::new(RefCell::new((0, 0, None, None)));

    // Helper: update window title and subtitle
    let update_title_and_header = {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let is_dirty = Rc::clone(&is_dirty_rc);
        let t_lbl = title_lbl.clone();
        let s_lbl = subtitle_lbl.clone();
        let dlg_w = dlg.clone();

        Rc::new(move || {
            let idx = *idx_rc.borrow();
            if let Some(entry) = entries.get(idx) {
                let dirty_str = if *is_dirty.borrow() { " ●" } else { "" };
                t_lbl.set_text(&format!("{}{dirty_str}", entry.name));
                s_lbl.set_text(&format!(
                    "{} • {}",
                    entry.formatted_size_clean(),
                    entry.path
                ));
                dlg_w.set_title(Some(&format!("Quick Look — {}{dirty_str}", entry.name)));
            }
        })
    };

    // Helper: display toast message
    let show_toast = {
        let overlay = toast_overlay.clone();
        Rc::new(move |msg: &str| {
            overlay.add_toast(adw::Toast::new(msg));
        })
    };

    // Helper: Discard Changes Confirmation Dialog
    let show_discard_dialog = {
        let dlg_parent = dlg.clone();
        let dirty_ref = Rc::clone(&is_dirty_rc);
        let orig_ref = Rc::clone(&original_content_rc);
        let tv_buf = text_view.buffer();
        let update_hdr = Rc::clone(&update_title_and_header);
        let save_b = save_btn.clone();

        Rc::new(move |on_discard: Rc<dyn Fn()>| {
            let confirm_dlg = adw::MessageDialog::builder()
                .transient_for(&dlg_parent)
                .heading(&tr("Discard Changes?"))
                .body(&tr(
                    "Você possui alterações não salvas neste arquivo que serão perdidas. Deseja realmente descartá-las?",
                ))
                .close_response("cancel")
                .build();
            confirm_dlg.add_response("cancel", &tr("Cancelar"));
            confirm_dlg.add_response("discard", &tr("Discard"));
            confirm_dlg.set_response_appearance("discard", adw::ResponseAppearance::Destructive);

            let dirty_c = Rc::clone(&dirty_ref);
            let orig_c = Rc::clone(&orig_ref);
            let buf_c = tv_buf.clone();
            let up_c = Rc::clone(&update_hdr);
            let sb_c = save_b.clone();
            let discard_cb = Rc::clone(&on_discard);

            confirm_dlg.connect_response(None, move |_, response| {
                if response == "discard" {
                    *dirty_c.borrow_mut() = false;
                    let orig = orig_c.borrow().clone();
                    buf_c.set_text(&orig);
                    sb_c.set_sensitive(false);
                    up_c();
                    discard_cb();
                }
            });
            confirm_dlg.present();
        })
    };

    // Helper: Perform file save (Normal Local, Remote SFTP, or Root via pkexec / sudo -S)
    let perform_save = {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let dirty_ref = Rc::clone(&is_dirty_rc);
        let orig_ref = Rc::clone(&original_content_rc);
        let tv = text_view.clone();
        let sb = save_btn.clone();
        let up_hdr = Rc::clone(&update_title_and_header);
        let toast = Rc::clone(&show_toast);
        let dlg_parent = dlg.clone();
        let refresh_cb = refresh_fn.clone();
        let backend_c = Rc::clone(&backend_rc);

        Rc::new(move |as_root: bool, on_success: Option<Rc<dyn Fn()>>| {
            let idx = *idx_rc.borrow();
            let Some(entry) = entries.get(idx) else {
                return;
            };
            let path_buf = PathBuf::from(&entry.path);
            let entry_name = entry.name.clone();

            let buffer = tv.buffer();
            let (start, end) = (buffer.start_iter(), buffer.end_iter());
            let current_text = buffer.text(&start, &end, true).to_string();

            let mark_saved_success: Rc<dyn Fn(String, &str)> = {
                let d_ref = Rc::clone(&dirty_ref);
                let o_ref = Rc::clone(&orig_ref);
                let sb_c = sb.clone();
                let up_c = Rc::clone(&up_hdr);
                let rf_c = refresh_cb.clone();
                let toast_c = Rc::clone(&toast);
                let on_ok = on_success.clone();
                Rc::new(move |saved_txt: String, msg: &str| {
                    *d_ref.borrow_mut() = false;
                    *o_ref.borrow_mut() = saved_txt;
                    sb_c.set_sensitive(false);
                    up_c();
                    if let Some(ref rf) = rf_c {
                        rf();
                    }
                    toast_c(msg);
                    if let Some(ref cb) = on_ok {
                        cb();
                    }
                })
            };

            if let QuickLookBackend::Remote(sftp) = backend_c.as_ref() {
                if as_root {
                    toast(&tr("Salvamento como Root local não se aplica a arquivos SFTP remotos."));
                    return;
                }
                let sftp_bg = sftp.clone();
                let path_bg = path_buf.clone();
                let text_bg = current_text.clone();
                let mark_ok = Rc::clone(&mark_saved_success);
                let parent_w = dlg_parent.clone();
                let sb_done = sb.clone();
                sb.set_sensitive(false);
                let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
                let text_thread = text_bg.clone();
                std::thread::spawn(move || {
                    let res = sftp_bg
                        .write_bytes(&path_bg, text_thread.as_bytes())
                        .map_err(|e| e.to_string());
                    let _ = tx.send(res);
                });
                glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
                    Ok(Ok(())) => {
                        mark_ok(text_bg.clone(), &tr("Arquivo remoto salvo via SFTP!"));
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        sb_done.set_sensitive(true);
                        let err_dlg = adw::MessageDialog::builder()
                            .transient_for(&parent_w)
                            .heading(&tr("Erro ao Salvar Arquivo Remoto"))
                            .body(&e)
                            .close_response("ok")
                            .build();
                        err_dlg.add_response("ok", &tr("OK"));
                        err_dlg.present();
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        sb_done.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                });
                return;
            }

            let run_root_save_async: Rc<dyn Fn(PathBuf, String)> = {
                let parent_w = dlg_parent.clone();
                let mark_ok = Rc::clone(&mark_saved_success);
                Rc::new(move |root_path: PathBuf, root_txt: String| {
                    let p_bg = root_path.clone();
                    let t_bg = root_txt.clone();
                    let parent_idle = parent_w.clone();
                    let mark_idle = Rc::clone(&mark_ok);
                    let (tx, rx) = std::sync::mpsc::sync_channel::<RootSaveOutcome>(1);
                    let p_thread = p_bg.clone();
                    let t_thread = t_bg.clone();
                    std::thread::spawn(move || {
                        let outcome = match save_file_as_root_pkexec(&p_thread, &t_thread) {
                            RootSaveOutcome::NeedSudoFallback(reason) => {
                                if try_sudo_with_password(&p_thread, &t_thread, None).is_ok() {
                                    RootSaveOutcome::Saved
                                } else {
                                    RootSaveOutcome::NeedSudoFallback(reason)
                                }
                            }
                            other => other,
                        };
                        let _ = tx.send(outcome);
                    });
                    glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
                        Ok(outcome) => {
                            match outcome {
                                RootSaveOutcome::Saved => {
                                    mark_idle(
                                        t_bg.clone(),
                                        &tr("Arquivo salvo com privilégios de Administrador (Root)!"),
                                    );
                                }
                                RootSaveOutcome::UserCancelled => {}
                                RootSaveOutcome::NeedSudoFallback(reason) => {
                                    let mark_sudo = Rc::clone(&mark_idle);
                                    let parent_err = parent_idle.clone();
                                    let txt_sudo = t_bg.clone();
                                    prompt_sudo_password_and_save(
                                        &parent_idle,
                                        p_bg.clone(),
                                        t_bg.clone(),
                                        reason,
                                        Rc::new(move |res| match res {
                                            Ok(()) => mark_sudo(
                                                txt_sudo.clone(),
                                                &tr("Arquivo salvo com privilégios de Administrador (sudo)!"),
                                            ),
                                            Err(err_msg) => {
                                                let ed = adw::MessageDialog::builder()
                                                    .transient_for(&parent_err)
                                                    .heading(&tr("Falha ao Salvar como Root"))
                                                    .body(&err_msg)
                                                    .close_response("ok")
                                                    .build();
                                                ed.add_response("ok", &tr("OK"));
                                                ed.present();
                                            }
                                        }),
                                    );
                                }
                                RootSaveOutcome::Error(err_msg) => {
                                    let err_dlg = adw::MessageDialog::builder()
                                        .transient_for(&parent_idle)
                                        .heading(&tr("Falha ao Salvar como Root"))
                                        .body(&err_msg)
                                        .close_response("ok")
                                        .build();
                                    err_dlg.add_response("ok", &tr("OK"));
                                    err_dlg.present();
                                }
                            }
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                    });
                })
            };

            if as_root {
                run_root_save_async(path_buf, current_text);
            } else {
                match fs::write(&path_buf, current_text.as_bytes()) {
                    Ok(()) => {
                        mark_saved_success(current_text, &tr("Arquivo salvo com sucesso!"));
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::PermissionDenied {
                            let perm_dlg = adw::MessageDialog::builder()
                                .transient_for(&dlg_parent)
                                .heading(&tr("Permissão Negada"))
                                .body(&tr_fmt(
                                    "Você não possui permissão para modificar '{name}'. Deseja salvar utilizando privilégios de Administrador (Root)?",
                                    &[("name", &entry_name)],
                                ))
                                .close_response("cancel")
                                .build();
                            perm_dlg.add_response("cancel", &tr("Cancelar"));
                            perm_dlg.add_response("root_save", &tr("Save as Root"));
                            perm_dlg.set_response_appearance("root_save", adw::ResponseAppearance::Suggested);

                            let root_save_path = path_buf;
                            let root_text = current_text;
                            let runner = Rc::clone(&run_root_save_async);

                            perm_dlg.connect_response(None, move |_, response| {
                                if response == "root_save" {
                                    runner(root_save_path.clone(), root_text.clone());
                                }
                            });
                            perm_dlg.present();
                        } else {
                            let err_dlg = adw::MessageDialog::builder()
                                .transient_for(&dlg_parent)
                                .heading(&tr("Erro ao Salvar Arquivo"))
                                .body(&tr_fmt(
                                    "Não foi possível gravar as alterações: {err}",
                                    &[("err", &e.to_string())],
                                ))
                                .close_response("ok")
                                .build();
                            err_dlg.add_response("ok", &tr("OK"));
                            err_dlg.present();
                        }
                    }
                }
            }
        })
    };

    // Helper: Popover actions builder
    let populate_more_options_menu = {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let vbox = popover_vbox.clone();
        let pop = popover.clone();
        let tv = text_view.clone();
        let toast = Rc::clone(&show_toast);
        let dlg_parent = dlg.clone();
        let terminal_feed = feed_terminal_cb.clone();
        let ai_cb = ai_prompt_cb.clone();
        let dirty_ref = Rc::clone(&is_dirty_rc);
        let orig_ref = Rc::clone(&original_content_rc);
        let is_loading_c = Rc::clone(&is_loading_buffer);
        let disc_dlg = Rc::clone(&show_discard_dialog);
        let backend_c = Rc::clone(&backend_rc);
        let cached_remote = Rc::clone(&cached_remote_text_files);
        let temp_files_c = Rc::clone(&dialog_temp_files);
        let syn_btn_c = syntax_toggle_btn.clone();

        Rc::new(move || {
            // Remove previous popover children
            while let Some(child) = vbox.first_child() {
                vbox.remove(&child);
            }

            let idx = *idx_rc.borrow();
            let Some(entry) = entries.get(idx) else {
                return;
            };
            let path_buf = Path::new(&entry.path).to_path_buf();
            let entry_name = entry.name.clone();
            let size_clean = entry.formatted_size_clean();

            // 1. Abrir com aplicativo externo
            {
                let p = path_buf.clone();
                let name_c = entry_name.clone();
                let pop_c = pop.clone();
                let bk_c = Rc::clone(&backend_c);
                let cache_map = Rc::clone(&cached_remote);
                let tmp_list = Rc::clone(&temp_files_c);
                let tv_c = tv.clone();
                let toast_c = Rc::clone(&toast);
                let btn = gtk4::Button::builder().has_frame(false).build();
                let hb = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                hb.set_margin_start(8);
                hb.set_margin_end(8);
                hb.set_margin_top(4);
                hb.set_margin_bottom(4);
                hb.append(&gtk4::Image::from_icon_name("document-open-symbolic"));
                let l = gtk4::Label::new(Some(&tr("Abrir com aplicativo externo")));
                l.set_xalign(0.0);
                hb.append(&l);
                btn.set_child(Some(&hb));
                btn.connect_clicked(move |_| {
                    pop_c.popdown();
                    let target_path = match bk_c.as_ref() {
                        QuickLookBackend::Local => Some(p.clone()),
                        QuickLookBackend::Remote(_) => {
                            if let Some(cached) = cache_map.borrow().get(&idx).cloned() {
                                Some(cached)
                            } else {
                                let buf = tv_c.buffer();
                                let (s, e) = (buf.start_iter(), buf.end_iter());
                                let txt = buf.text(&s, &e, true).to_string();
                                match create_remote_temp_file(&name_c, txt.as_bytes()) {
                                    Ok(tmp_p) => {
                                        cache_map.borrow_mut().insert(idx, tmp_p.clone());
                                        tmp_list.borrow_mut().push(tmp_p.clone());
                                        Some(tmp_p)
                                    }
                                    Err(err) => {
                                        toast_c(&err);
                                        None
                                    }
                                }
                            }
                        }
                    };
                    if let Some(open_p) = target_path {
                        let uri = format!("file://{}", open_p.display());
                        let ok =
                            gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE)
                                .is_ok();
                        if !ok {
                            let _ = std::process::Command::new("xdg-open").arg(&open_p).spawn();
                        }
                    }
                });
                vbox.append(&btn);
            }

            // 2. Explicar com IA
            {
                let pop_c = pop.clone();
                let tv_c = tv.clone();
                let ai_opt = ai_cb.clone();
                let toast_c = Rc::clone(&toast);
                let name_c = entry_name.clone();
                let dlg_c = dlg_parent.clone();

                let btn = gtk4::Button::builder().has_frame(false).build();
                let hb = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                hb.set_margin_start(8);
                hb.set_margin_end(8);
                hb.set_margin_top(4);
                hb.set_margin_bottom(4);
                hb.append(&gtk4::Image::from_icon_name("system-search-symbolic"));
                let l = gtk4::Label::new(Some(&tr("Explain with AI")));
                l.set_xalign(0.0);
                hb.append(&l);
                btn.set_child(Some(&hb));

                btn.connect_clicked(move |_| {
                    pop_c.popdown();
                    if let Some(ref ai) = ai_opt {
                        let buffer = tv_c.buffer();
                        let (s, e) = (buffer.start_iter(), buffer.end_iter());
                        let txt = buffer.text(&s, &e, true);
                        let sample: String = txt.lines().take(80).collect::<Vec<_>>().join("\n");
                        let prompt = tr_fmt(
                            "Por favor, analise e explique o código/conteúdo do arquivo '{name}', sua finalidade, estrutura e pontos importantes:\n```\n{code}\n```",
                            &[("name", &name_c), ("code", &sample)],
                        );
                        ai(&prompt);
                        toast_c(&tr("Análise solicitada ao Assistente de IA."));
                        dlg_c.close();
                    } else {
                        toast_c(&tr("Assistente de IA não está conectado."));
                    }
                });
                vbox.append(&btn);
            }

            // 3. Calcular / Verificar Hash...
            {
                let pop_c = pop.clone();
                let parent_win = dlg_parent.clone();
                let p = path_buf.clone();
                let name_c = entry_name.clone();
                let sz_c = size_clean.clone();
                let feed_opt = terminal_feed.clone();

                let btn = gtk4::Button::builder().has_frame(false).build();
                let hb = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                hb.set_margin_start(8);
                hb.set_margin_end(8);
                hb.set_margin_top(4);
                hb.set_margin_bottom(4);
                hb.append(&gtk4::Image::from_icon_name("accessories-calculator-symbolic"));
                let l = gtk4::Label::new(Some(&tr("Calculate / Verify Hash...")));
                l.set_xalign(0.0);
                hb.append(&l);
                btn.set_child(Some(&hb));

                btn.connect_clicked(move |_| {
                    pop_c.popdown();
                    let dummy_cb: Rc<dyn Fn(&str)> = Rc::new(|_: &str| {});
                    let cb_ref = feed_opt.as_ref().unwrap_or(&dummy_cb);
                    crate::checksum_dialog::show_checksum_dialog(
                        &parent_win,
                        &p,
                        &name_c,
                        &sz_c,
                        cb_ref,
                    );
                });
                vbox.append(&btn);
            }

            // 4. Copiar Conteúdo
            {
                let pop_c = pop.clone();
                let tv_c = tv.clone();
                let toast_c = Rc::clone(&toast);

                let btn = gtk4::Button::builder().has_frame(false).build();
                let hb = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                hb.set_margin_start(8);
                hb.set_margin_end(8);
                hb.set_margin_top(4);
                hb.set_margin_bottom(4);
                hb.append(&gtk4::Image::from_icon_name("edit-copy-symbolic"));
                let l = gtk4::Label::new(Some(&tr("Copy Content")));
                l.set_xalign(0.0);
                hb.append(&l);
                btn.set_child(Some(&hb));

                btn.connect_clicked(move |_| {
                    pop_c.popdown();
                    let buffer = tv_c.buffer();
                    let (s, e) = (buffer.start_iter(), buffer.end_iter());
                    let txt = buffer.text(&s, &e, true);
                    if let Some(display) = gdk::Display::default() {
                        display.clipboard().set_text(&txt);
                        toast_c(&tr("Conteúdo copiado para a área de transferência."));
                    }
                });
                vbox.append(&btn);
            }

            // 5. Recarregar do Disco / Servidor Remoto
            {
                let pop_c = pop.clone();
                let p = path_buf.clone();
                let tv_c = tv.clone();
                let is_d = Rc::clone(&dirty_ref);
                let orig_c = Rc::clone(&orig_ref);
                let is_load = Rc::clone(&is_loading_c);
                let disc_dlg = Rc::clone(&disc_dlg);
                let toast_c = Rc::clone(&toast);
                let bk_c = Rc::clone(&backend_c);
                let syn_c = syn_btn_c.clone();

                let btn = gtk4::Button::builder().has_frame(false).build();
                let hb = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                hb.set_margin_start(8);
                hb.set_margin_end(8);
                hb.set_margin_top(4);
                hb.set_margin_bottom(4);
                hb.append(&gtk4::Image::from_icon_name("view-refresh-symbolic"));
                let l = gtk4::Label::new(Some(&tr("Reload from Disk")));
                l.set_xalign(0.0);
                hb.append(&l);
                btn.set_child(Some(&hb));

                btn.connect_clicked(move |_| {
                    pop_c.popdown();
                    let p2 = p.clone();
                    let tv2 = tv_c.clone();
                    let t2 = Rc::clone(&toast_c);
                    let bk2 = Rc::clone(&bk_c);
                    let orig2 = Rc::clone(&orig_c);
                    let load2 = Rc::clone(&is_load);
                    let syn2 = syn_c.clone();

                    let reload_action = Rc::new(move || {
                        let apply_fresh = {
                            let tv3 = tv2.clone();
                            let p3 = p2.clone();
                            let orig3 = Rc::clone(&orig2);
                            let load3 = Rc::clone(&load2);
                            let syn3 = syn2.clone();
                            move |fresh: String| {
                                *orig3.borrow_mut() = fresh.clone();
                                *load3.borrow_mut() = true;
                                tv3.buffer().set_text(&fresh);
                                *load3.borrow_mut() = false;
                                let dark = adw::StyleManager::default().is_dark();
                                apply_buffer_highlighting(&tv3.buffer(), &p3, syn3.is_active(), dark);
                            }
                        };
                        match bk2.as_ref() {
                            QuickLookBackend::Local => {
                                if let Ok(fresh) = fs::read_to_string(&p2) {
                                    apply_fresh(fresh);
                                    t2(&tr("Arquivo recarregado do disco."));
                                }
                            }
                            QuickLookBackend::Remote(sftp) => {
                                let sftp_bg = sftp.clone();
                                let p_bg = p2.clone();
                                let t3 = Rc::clone(&t2);
                                let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, std::io::Error>>(1);
                                std::thread::spawn(move || {
                                    let res = sftp_bg.read_bytes(&p_bg, Some(REMOTE_TEXT_MAX_BYTES));
                                    let _ = tx.send(res);
                                });
                                glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
                                    Ok(Ok(bytes)) => {
                                        let fresh = String::from_utf8_lossy(&bytes).to_string();
                                        apply_fresh(fresh);
                                        t3(&tr("Arquivo recarregado via SFTP."));
                                        glib::ControlFlow::Break
                                    }
                                    Ok(Err(e)) => {
                                        t3(&e.to_string());
                                        glib::ControlFlow::Break
                                    }
                                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                                });
                            }
                        }
                    });

                    if *is_d.borrow() {
                        disc_dlg(reload_action);
                    } else {
                        reload_action();
                    }
                });
                vbox.append(&btn);
            }
        })
    };

    // Helper: Apply current image zoom state (`None` = Fit to window)
    let apply_zoom_ui: Rc<dyn Fn()> = {
        let pic = picture.clone();
        let zoom_rc = Rc::clone(&zoom_scale_rc);
        let nat_rc = Rc::clone(&natural_img_size_rc);
        let pct_lbl = zoom_pct_lbl.clone();
        Rc::new(move || match zoom_rc.get() {
            None => {
                pic.set_can_shrink(true);
                pic.set_size_request(-1, -1);
                let (w, h) = nat_rc.get();
                if w > 0 && h > 0 {
                    pct_lbl.set_text(&format!("{} ({w}×{h})", tr("Ajustar")));
                } else {
                    pct_lbl.set_text(&tr("Ajustar"));
                }
            }
            Some(scale) => {
                let clamped = clamp_image_zoom(scale);
                zoom_rc.set(Some(clamped));
                let (nw, nh) = nat_rc.get();
                let (w, h) = if nw > 0 && nh > 0 {
                    ((f64::from(nw) * clamped).round() as i32, (f64::from(nh) * clamped).round() as i32)
                } else {
                    ((600.0 * clamped).round() as i32, (420.0 * clamped).round() as i32)
                };
                pic.set_can_shrink(false);
                pic.set_size_request(w.max(16), h.max(16));
                pct_lbl.set_text(&format!("{}%", (clamped * 100.0).round() as i32));
            }
        })
    };

    // Wire Image Zoom & Pan controls
    {
        let z_rc = Rc::clone(&zoom_scale_rc);
        let apply_z = Rc::clone(&apply_zoom_ui);
        btn_zoom_in.connect_clicked(move |_| {
            let next = clamp_image_zoom(z_rc.get().unwrap_or(1.0) * 1.25);
            z_rc.set(Some(next));
            apply_z();
        });
    }
    {
        let z_rc = Rc::clone(&zoom_scale_rc);
        let apply_z = Rc::clone(&apply_zoom_ui);
        btn_zoom_out.connect_clicked(move |_| {
            let next = clamp_image_zoom(z_rc.get().unwrap_or(1.0) / 1.25);
            z_rc.set(Some(next));
            apply_z();
        });
    }
    {
        let z_rc = Rc::clone(&zoom_scale_rc);
        let apply_z = Rc::clone(&apply_zoom_ui);
        btn_zoom_fit.connect_clicked(move |_| {
            z_rc.set(None);
            apply_z();
        });
    }
    {
        let z_rc = Rc::clone(&zoom_scale_rc);
        let apply_z = Rc::clone(&apply_zoom_ui);
        btn_zoom_100.connect_clicked(move |_| {
            z_rc.set(Some(1.0));
            apply_z();
        });
    }
    {
        let z_rc = Rc::clone(&zoom_scale_rc);
        let apply_z = Rc::clone(&apply_zoom_ui);
        let base_zoom = Rc::new(Cell::new(1.0f64));
        let zoom_gesture = gtk4::GestureZoom::new();
        let b_begin = Rc::clone(&base_zoom);
        let z_begin = Rc::clone(&z_rc);
        zoom_gesture.connect_begin(move |_, _| {
            b_begin.set(z_begin.get().unwrap_or(1.0));
        });
        zoom_gesture.connect_scale_changed(move |_, scale| {
            z_rc.set(Some(clamp_image_zoom(base_zoom.get() * scale)));
            apply_z();
        });
        image_scroller.add_controller(zoom_gesture);
    }
    {
        let z_rc = Rc::clone(&zoom_scale_rc);
        let apply_z = Rc::clone(&apply_zoom_ui);
        let scroll_ctrl = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
        scroll_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
        scroll_ctrl.connect_scroll(move |ctrl, _dx, dy| {
            let state = ctrl.current_event_state();
            if state.contains(gdk::ModifierType::CONTROL_MASK) {
                let factor = if dy < 0.0 { 1.15 } else { 1.0 / 1.15 };
                let cur = z_rc.get().unwrap_or(1.0);
                z_rc.set(Some(clamp_image_zoom(cur * factor)));
                apply_z();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        image_scroller.add_controller(scroll_ctrl);
    }
    {
        let drag = gtk4::GestureDrag::new();
        let scroller_c = image_scroller.clone();
        let start_hv = Rc::new(Cell::new((0.0f64, 0.0f64)));
        let s_beg = Rc::clone(&start_hv);
        let sc_beg = scroller_c.clone();
        drag.connect_drag_begin(move |_, _, _| {
            s_beg.set((sc_beg.hadjustment().value(), sc_beg.vadjustment().value()));
        });
        drag.connect_drag_update(move |_, off_x, off_y| {
            let (hx, vy) = start_hv.get();
            scroller_c.hadjustment().set_value(hx - off_x);
            scroller_c.vadjustment().set_value(vy - off_y);
        });
        image_scroller.add_controller(drag);
    }

    // Helper: Render current Hex Dump page (`HEX_PAGE_SIZE` bytes per page)
    let render_hex_page: Rc<dyn Fn()> = {
        let state_c = Rc::clone(&hex_state_rc);
        let hv = hex_view.clone();
        let lbl = hex_page_lbl.clone();
        let b_prev = btn_hex_prev.clone();
        let b_next = btn_hex_next.clone();
        Rc::new(move || {
            let mut st = state_c.borrow_mut();
            let effective_total = if let Some(ref rbytes) = st.3 {
                rbytes.len() as u64
            } else {
                st.0
            };
            let (total_pages, start, end) = compute_hex_page_bounds(effective_total, st.1, HEX_PAGE_SIZE);
            st.1 = st.1.min(total_pages.saturating_sub(1));
            b_prev.set_sensitive(st.1 > 0);
            b_next.set_sensitive(st.1 + 1 < total_pages);

            let slice = if let Some(ref rbytes) = st.3 {
                rbytes.get(start..end).unwrap_or(&[]).to_vec()
            } else if let Some(ref lp) = st.2 {
                read_local_hex_slice(lp, start, end.saturating_sub(start)).unwrap_or_default()
            } else {
                Vec::new()
            };

            let dump = if slice.is_empty() {
                tr("(Arquivo vazio — 0 bytes)")
            } else {
                format_hex_dump_page(&slice, start, 16)
            };
            hv.buffer().set_text(&dump);
            lbl.set_text(&format!(
                "{} {}/{} • 0x{:08X}–0x{:08X}",
                tr("Página"),
                st.1 + 1,
                total_pages,
                start,
                end
            ));
        })
    };
    {
        let st_c = Rc::clone(&hex_state_rc);
        let r_hex = Rc::clone(&render_hex_page);
        btn_hex_prev.connect_clicked(move |_| {
            {
                let mut st = st_c.borrow_mut();
                st.1 = st.1.saturating_sub(1);
            }
            r_hex();
        });
    }
    {
        let st_c = Rc::clone(&hex_state_rc);
        let r_hex = Rc::clone(&render_hex_page);
        btn_hex_next.connect_clicked(move |_| {
            {
                let mut st = st_c.borrow_mut();
                st.1 = st.1.saturating_add(1);
            }
            r_hex();
        });
    }

    // Helper: Render active entry into UI (supports synchronous Local & async Remote SFTP)
    let render_entry: Rc<dyn Fn()> = {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let pos_l = pos_lbl.clone();
        let up_hdr = Rc::clone(&update_title_and_header);
        let stack = body_stack.clone();
        let pic = picture.clone();
        let tv = text_view.clone();
        let bin_card = binary_card.clone();
        let hex_tb = hex_toolbar.clone();
        let hex_sc = hex_scroller.clone();
        let orig_c = Rc::clone(&original_content_rc);
        let dirty_c = Rc::clone(&is_dirty_rc);
        let is_loading = Rc::clone(&is_loading_buffer);
        let info_lbl = text_info_label.clone();
        let badge_lbl = mode_badge.clone();
        let edit_btn = edit_toggle_btn.clone();
        let syn_btn = syntax_toggle_btn.clone();
        let save_b = save_btn.clone();
        let save_s = save_sudo_btn.clone();
        let ro_banner = readonly_banner.clone();
        let is_ed = Rc::clone(&is_editing_rc);
        let build_menu = Rc::clone(&populate_more_options_menu);
        let r_gen = Rc::clone(&render_gen);
        let bk_c = Rc::clone(&backend_rc);
        let temp_files_c = Rc::clone(&dialog_temp_files);
        let cache_map = Rc::clone(&cached_remote_text_files);
        let zoom_rc = Rc::clone(&zoom_scale_rc);
        let nat_rc = Rc::clone(&natural_img_size_rc);
        let apply_z = Rc::clone(&apply_zoom_ui);
        let hex_st = Rc::clone(&hex_state_rc);
        let r_hex = Rc::clone(&render_hex_page);

        Rc::new(move || {
            let idx = *idx_rc.borrow();
            let Some(entry) = entries.get(idx).cloned() else {
                return;
            };
            let my_gen = r_gen.get().wrapping_add(1);
            r_gen.set(my_gen);

            pos_l.set_text(&format!("{}/{}", idx + 1, entries.len()));
            *dirty_c.borrow_mut() = false;
            save_b.set_sensitive(false);
            up_hdr();
            build_menu();

            let path = Path::new(&entry.path).to_path_buf();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let is_img = matches!(
                ext.as_str(),
                "png" | "jpg" | "jpeg" | "svg" | "webp" | "ico" | "bmp" | "gif"
            );

            if entry.is_dir {
                edit_btn.set_visible(false);
                syn_btn.set_visible(false);
                save_b.set_visible(false);
                save_s.set_visible(false);
                hex_tb.set_visible(false);
                hex_sc.set_visible(false);
                while let Some(child) = bin_card.first_child() {
                    bin_card.remove(&child);
                }
                let icon = gtk4::Image::from_icon_name("folder-symbolic");
                icon.set_pixel_size(64);
                bin_card.append(&icon);
                let info = gtk4::Label::new(Some(&tr_fmt(
                    "Diretório: {name}\nCaminho: {path}\nPermissões: {perms}",
                    &[
                        ("name", &entry.name),
                        ("path", &entry.path),
                        ("perms", &entry.formatted_permissions()),
                    ],
                )));
                info.set_justify(gtk4::Justification::Center);
                bin_card.append(&info);
                stack.set_visible_child_name("binary");
                return;
            }

            let show_loaded = {
                let edit_btn = edit_btn.clone();
                let syn_btn = syn_btn.clone();
                let save_b = save_b.clone();
                let save_s = save_s.clone();
                let pic = pic.clone();
                let tv = tv.clone();
                let bin_card = bin_card.clone();
                let hex_tb = hex_tb.clone();
                let hex_sc = hex_sc.clone();
                let orig_c = Rc::clone(&orig_c);
                let is_loading = Rc::clone(&is_loading);
                let info_lbl = info_lbl.clone();
                let badge_lbl = badge_lbl.clone();
                let ro_banner = ro_banner.clone();
                let is_ed = Rc::clone(&is_ed);
                let stack = stack.clone();
                let zoom_rc = Rc::clone(&zoom_rc);
                let nat_rc = Rc::clone(&nat_rc);
                let apply_z = Rc::clone(&apply_z);
                let hex_st = Rc::clone(&hex_st);
                let r_hex = Rc::clone(&r_hex);
                let temp_files_c = Rc::clone(&temp_files_c);
                let cache_map = Rc::clone(&cache_map);

                move |entry: &FileEntry,
                      kind: PreviewKind,
                      text_opt: Option<String>,
                      remote_bytes: Option<Vec<u8>>,
                      local_file_path: Option<PathBuf>,
                      is_readonly: bool| {
                    match kind {
                        PreviewKind::Image => {
                            edit_btn.set_visible(false);
                            syn_btn.set_visible(false);
                            save_b.set_visible(false);
                            save_s.set_visible(false);
                            hex_tb.set_visible(false);
                            hex_sc.set_visible(false);
                            let img_path = if let Some(lp) = local_file_path {
                                Some(lp)
                            } else if let Some(bytes) = remote_bytes {
                                match create_remote_temp_file(&entry.name, &bytes) {
                                    Ok(tp) => {
                                        temp_files_c.borrow_mut().push(tp.clone());
                                        cache_map.borrow_mut().insert(idx, tp.clone());
                                        Some(tp)
                                    }
                                    Err(_) => None,
                                }
                            } else {
                                None
                            };
                            if let Some(ip) = img_path {
                                if let Ok(tex) = gdk::Texture::from_file(&gio::File::for_path(&ip)) {
                                    nat_rc.set((tex.width(), tex.height()));
                                } else {
                                    nat_rc.set((0, 0));
                                }
                                pic.set_filename(Some(&ip));
                            } else {
                                nat_rc.set((0, 0));
                                pic.set_filename(None::<&Path>);
                            }
                            zoom_rc.set(None);
                            apply_z();
                            stack.set_visible_child_name("image");
                        }
                        PreviewKind::Text => {
                            edit_btn.set_visible(true);
                            syn_btn.set_visible(true);
                            hex_tb.set_visible(false);
                            hex_sc.set_visible(false);
                            let editing = *is_ed.borrow();
                            save_b.set_visible(editing);
                            save_s.set_visible(editing);

                            let raw_text = text_opt.unwrap_or_default();
                            *orig_c.borrow_mut() = raw_text.clone();
                            *is_loading.borrow_mut() = true;
                            tv.buffer().set_text(&raw_text);
                            *is_loading.borrow_mut() = false;

                            let dark = adw::StyleManager::default().is_dark();
                            apply_buffer_highlighting(
                                &tv.buffer(),
                                Path::new(&entry.path),
                                syn_btn.is_active(),
                                dark,
                            );

                            ro_banner.set_revealed(is_readonly);
                            let lines_count = raw_text.lines().count();
                            let mime = guess_mime_type(&entry.name);
                            info_lbl.set_text(&tr_fmt(
                                "{lines} linhas • {size} • {mime} • UTF-8",
                                &[
                                    ("lines", &lines_count.to_string()),
                                    ("size", &entry.formatted_size_clean()),
                                    ("mime", mime),
                                ],
                            ));
                            if editing {
                                tv.set_editable(true);
                                badge_lbl.set_text(&tr("Edit Mode"));
                                badge_lbl.remove_css_class("dim-label");
                                badge_lbl.add_css_class("badge-exec");
                            } else {
                                tv.set_editable(false);
                                badge_lbl.set_text(&tr("Preview Mode"));
                                badge_lbl.remove_css_class("badge-exec");
                                badge_lbl.add_css_class("dim-label");
                            }
                            stack.set_visible_child_name("text");
                        }
                        _ => {
                            edit_btn.set_visible(false);
                            syn_btn.set_visible(false);
                            save_b.set_visible(false);
                            save_s.set_visible(false);
                            while let Some(child) = bin_card.first_child() {
                                bin_card.remove(&child);
                            }
                            let hdr_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
                            let icon = gtk4::Image::from_icon_name("application-x-executable-symbolic");
                            icon.set_pixel_size(40);
                            hdr_box.append(&icon);
                            let info = gtk4::Label::new(Some(&tr_fmt(
                                "Arquivo Binário: {name} • {size} • {perms}",
                                &[
                                    ("name", &entry.name),
                                    ("size", &entry.formatted_size_clean()),
                                    ("perms", &entry.formatted_permissions()),
                                ],
                            )));
                            info.set_wrap(true);
                            hdr_box.append(&info);
                            bin_card.append(&hdr_box);

                            *hex_st.borrow_mut() = (
                                entry.size,
                                0,
                                local_file_path,
                                remote_bytes,
                            );
                            hex_tb.set_visible(true);
                            hex_sc.set_visible(true);
                            r_hex();
                            stack.set_visible_child_name("binary");
                        }
                    }
                }
            };

            match bk_c.as_ref() {
                QuickLookBackend::Local => {
                    if is_img {
                        show_loaded(&entry, PreviewKind::Image, None, None, Some(path), false);
                        return;
                    }
                    match load_local_preview(&path) {
                        Ok(preview) => match preview.kind {
                            PreviewKind::Text => {
                                let raw = fs::read_to_string(&path).unwrap_or_else(|_| {
                                    preview.text_content.clone().unwrap_or_default()
                                });
                                let readonly = fs::OpenOptions::new().write(true).open(&path).is_err();
                                show_loaded(&entry, PreviewKind::Text, Some(raw), None, Some(path), readonly);
                            }
                            other => {
                                show_loaded(&entry, other, preview.text_content, None, Some(path), false);
                            }
                        },
                        Err(_) => {
                            edit_btn.set_visible(false);
                            syn_btn.set_visible(false);
                            save_b.set_visible(false);
                            save_s.set_visible(false);
                            hex_tb.set_visible(false);
                            hex_sc.set_visible(false);
                            while let Some(child) = bin_card.first_child() {
                                bin_card.remove(&child);
                            }
                            let info = gtk4::Label::new(Some(&tr_fmt(
                                "Não foi possível carregar o item: {name}",
                                &[("name", &entry.name)],
                            )));
                            bin_card.append(&info);
                            stack.set_visible_child_name("binary");
                        }
                    }
                }
                QuickLookBackend::Remote(sftp) => {
                    edit_btn.set_visible(false);
                    syn_btn.set_visible(false);
                    save_b.set_visible(false);
                    save_s.set_visible(false);
                    hex_tb.set_visible(false);
                    hex_sc.set_visible(false);
                    while let Some(child) = bin_card.first_child() {
                        bin_card.remove(&child);
                    }
                    let spinner = gtk4::Spinner::builder().spinning(true).width_request(32).height_request(32).build();
                    let loading_lbl = gtk4::Label::new(Some(&tr_fmt(
                        "Carregando '{name}' via SFTP...",
                        &[("name", &entry.name)],
                    )));
                    bin_card.append(&spinner);
                    bin_card.append(&loading_lbl);
                    stack.set_visible_child_name("binary");

                    let sftp_bg = sftp.clone();
                    let path_bg = path.clone();
                    let entry_bg = entry.clone();
                    let max_bytes = if is_img {
                        REMOTE_IMAGE_MAX_BYTES
                    } else if matches!(ext.as_str(), "bin" | "iso" | "exe" | "elf" | "so" | "o" | "zip" | "tar" | "gz") {
                        REMOTE_BINARY_MAX_BYTES
                    } else {
                        REMOTE_TEXT_MAX_BYTES
                    };
                    let gen_ref = Rc::clone(&r_gen);
                    let bin_c_err = bin_card.clone();
                    let st_err = stack.clone();
                    let (tx, rx) = std::sync::mpsc::sync_channel::<Result<Vec<u8>, std::io::Error>>(1);
                    std::thread::spawn(move || {
                        let res = sftp_bg.read_bytes(&path_bg, Some(max_bytes));
                        let _ = tx.send(res);
                    });
                    glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
                        Ok(res) => {
                            if gen_ref.get() != my_gen {
                                return glib::ControlFlow::Break;
                            }
                            match res {
                                Ok(bytes) => {
                                    let preview = load_preview_from_bytes(
                                        &entry_bg.name,
                                        entry_bg.size.max(bytes.len() as u64),
                                        &bytes,
                                    );
                                    if preview.kind == PreviewKind::Text {
                                        let raw = String::from_utf8_lossy(&bytes).to_string();
                                        show_loaded(&entry_bg, PreviewKind::Text, Some(raw), Some(bytes), None, false);
                                    } else {
                                        show_loaded(&entry_bg, preview.kind, preview.text_content, Some(bytes), None, false);
                                    }
                                }
                                Err(e) => {
                                    while let Some(child) = bin_c_err.first_child() {
                                        bin_c_err.remove(&child);
                                    }
                                    let info = gtk4::Label::new(Some(&tr_fmt(
                                        "Falha SFTP ao abrir '{name}': {err}",
                                        &[("name", &entry_bg.name), ("err", &e.to_string())],
                                    )));
                                    info.set_wrap(true);
                                    bin_c_err.append(&info);
                                    st_err.set_visible_child_name("binary");
                                }
                            }
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                    });
                }
            }
        })
    };

    // ── 6. Wire Buffer Change (Dirty State & Syntax Debounce) ────────────────
    let syntax_debounce_src: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    {
        let tv_buf = text_view.buffer();
        let orig_c = Rc::clone(&original_content_rc);
        let dirty_c = Rc::clone(&is_dirty_rc);
        let is_loading = Rc::clone(&is_loading_buffer);
        let sb = save_btn.clone();
        let up_hdr = Rc::clone(&update_title_and_header);
        let syn_btn_c = syntax_toggle_btn.clone();
        let entries_c = Rc::clone(&entries_rc);
        let idx_c = Rc::clone(&current_idx);
        let deb_c = Rc::clone(&syntax_debounce_src);

        tv_buf.connect_changed(move |buf| {
            if *is_loading.borrow() {
                return;
            }
            let (s, e) = (buf.start_iter(), buf.end_iter());
            let current = buf.text(&s, &e, true).to_string();
            let is_dirty = current != *orig_c.borrow();
            if is_dirty != *dirty_c.borrow() {
                *dirty_c.borrow_mut() = is_dirty;
                sb.set_sensitive(is_dirty);
                up_hdr();
            }
            if syn_btn_c.is_active() && current.len() <= 200 * 1024 {
                if let Some(old) = deb_c.borrow_mut().take() {
                    old.remove();
                }
                let buf_w = buf.clone();
                let entries_w = Rc::clone(&entries_c);
                let idx_w = Rc::clone(&idx_c);
                let syn_w = syn_btn_c.clone();
                let deb_w = Rc::clone(&deb_c);
                let sid = glib::timeout_add_local_once(Duration::from_millis(250), move || {
                    *deb_w.borrow_mut() = None;
                    if !syn_w.is_active() {
                        return;
                    }
                    let idx = *idx_w.borrow();
                    if let Some(ent) = entries_w.get(idx) {
                        let dark = adw::StyleManager::default().is_dark();
                        apply_buffer_highlighting(&buf_w, Path::new(&ent.path), true, dark);
                    }
                });
                *deb_c.borrow_mut() = Some(sid);
            }
        });
    }
    {
        let tv_c = text_view.clone();
        let entries_c = Rc::clone(&entries_rc);
        let idx_c = Rc::clone(&current_idx);
        syntax_toggle_btn.connect_toggled(move |btn| {
            let idx = *idx_c.borrow();
            if let Some(ent) = entries_c.get(idx) {
                let dark = adw::StyleManager::default().is_dark();
                apply_buffer_highlighting(
                    &tv_c.buffer(),
                    Path::new(&ent.path),
                    btn.is_active(),
                    dark,
                );
            }
        });
    }

    // ── 7. Wire Edit Toggle Button ───────────────────────────────────────────
    {
        let is_ed = Rc::clone(&is_editing_rc);
        let is_d = Rc::clone(&is_dirty_rc);
        let tv = text_view.clone();
        let sb = save_btn.clone();
        let ss = save_sudo_btn.clone();
        let badge_lbl = mode_badge.clone();
        let disc_dlg = Rc::clone(&show_discard_dialog);
        let btn_toggle = edit_toggle_btn.clone();

        edit_toggle_btn.connect_toggled(move |btn| {
            let active = btn.is_active();
            if active {
                *is_ed.borrow_mut() = true;
                tv.set_editable(true);
                sb.set_visible(true);
                ss.set_visible(true);
                badge_lbl.set_text(&tr("Edit Mode"));
                badge_lbl.remove_css_class("dim-label");
                badge_lbl.add_css_class("badge-exec");
                tv.grab_focus();
            } else {
                if *is_d.borrow() {
                    let tv2 = tv.clone();
                    let sb2 = sb.clone();
                    let ss2 = ss.clone();
                    let b_lbl = badge_lbl.clone();
                    let is_ed2 = Rc::clone(&is_ed);
                    let btn_tog = btn_toggle.clone();

                    disc_dlg(Rc::new(move || {
                        *is_ed2.borrow_mut() = false;
                        tv2.set_editable(false);
                        sb2.set_visible(false);
                        ss2.set_visible(false);
                        b_lbl.set_text(&tr("Preview Mode"));
                        b_lbl.remove_css_class("badge-exec");
                        b_lbl.add_css_class("dim-label");
                        btn_tog.set_active(false);
                    }));
                } else {
                    *is_ed.borrow_mut() = false;
                    tv.set_editable(false);
                    sb.set_visible(false);
                    ss.set_visible(false);
                    badge_lbl.set_text(&tr("Preview Mode"));
                    badge_lbl.remove_css_class("badge-exec");
                    badge_lbl.add_css_class("dim-label");
                }
            }
        });
    }

    // ── 8. Wire Save Buttons ─────────────────────────────────────────────────
    {
        let saver = Rc::clone(&perform_save);
        save_btn.connect_clicked(move |_| {
            saver(false, None);
        });
    }
    {
        let saver = Rc::clone(&perform_save);
        save_sudo_btn.connect_clicked(move |_| {
            saver(true, None);
        });
    }

    // ── 9. Wire Navigation Buttons ───────────────────────────────────────────
    {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let render = Rc::clone(&render_entry);
        let is_d = Rc::clone(&is_dirty_rc);
        let disc_dlg = Rc::clone(&show_discard_dialog);

        btn_prev.connect_clicked(move |_| {
            let entries_c = Rc::clone(&entries);
            let idx_c = Rc::clone(&idx_rc);
            let render_c = Rc::clone(&render);

            let do_nav = Rc::new(move || {
                let curr = *idx_c.borrow();
                if let Some((next_idx, _)) = select_quick_look_entry(&entries_c, curr, false) {
                    *idx_c.borrow_mut() = next_idx;
                    render_c();
                }
            });

            if *is_d.borrow() {
                disc_dlg(do_nav);
            } else {
                do_nav();
            }
        });
    }
    {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let render = Rc::clone(&render_entry);
        let is_d = Rc::clone(&is_dirty_rc);
        let disc_dlg = Rc::clone(&show_discard_dialog);

        btn_next.connect_clicked(move |_| {
            let entries_c = Rc::clone(&entries);
            let idx_c = Rc::clone(&idx_rc);
            let render_c = Rc::clone(&render);

            let do_nav = Rc::new(move || {
                let curr = *idx_c.borrow();
                if let Some((next_idx, _)) = select_quick_look_entry(&entries_c, curr, true) {
                    *idx_c.borrow_mut() = next_idx;
                    render_c();
                }
            });

            if *is_d.borrow() {
                disc_dlg(do_nav);
            } else {
                do_nav();
            }
        });
    }

    // ── 10. Wire Close Interception (Unsaved changes check) ───────────────────
    {
        let is_d = Rc::clone(&is_dirty_rc);
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let saver = Rc::clone(&perform_save);
        let dlg_w = dlg.clone();

        let temp_files_close = Rc::clone(&dialog_temp_files);
        let deb_close = Rc::clone(&syntax_debounce_src);
        dlg.connect_close_request(move |_| {
            if *is_d.borrow() {
                let idx = *idx_rc.borrow();
                let fname = entries
                    .get(idx)
                    .map(|e| e.name.clone())
                    .unwrap_or_else(|| "arquivo".to_string());

                let confirm = adw::MessageDialog::builder()
                    .transient_for(&dlg_w)
                    .heading(&tr("Unsaved Changes"))
                    .body(&tr_fmt(
                        "Você possui alterações não salvas em '{fname}'. O que deseja fazer antes de fechar?",
                        &[("fname", &fname)],
                    ))
                    .close_response("cancel")
                    .build();

                confirm.add_response("cancel", &tr("Cancelar"));
                confirm.add_response("discard", &tr("Discard"));
                confirm.add_response("save", &tr("Salvar & Fechar"));
                confirm.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
                confirm.set_response_appearance("save", adw::ResponseAppearance::Suggested);

                let is_d_c = Rc::clone(&is_d);
                let w_c = dlg_w.clone();
                let saver_c = Rc::clone(&saver);

                confirm.connect_response(None, move |_, resp| {
                    if resp == "discard" {
                        *is_d_c.borrow_mut() = false;
                        w_c.close();
                    } else if resp == "save" {
                        let w_close = w_c.clone();
                        saver_c(false, Some(Rc::new(move || {
                            w_close.close();
                        })));
                    }
                });

                confirm.present();
                glib::Propagation::Stop
            } else {
                if let Some(sid) = deb_close.borrow_mut().take() {
                    sid.remove();
                }
                if clear_remote_on_exit {
                    for p in temp_files_close.borrow_mut().drain(..) {
                        let _ = fs::remove_file(p);
                    }
                }
                glib::Propagation::Proceed
            }
        });
    }

    // ── 11. Keyboard Shortcuts (Ctrl+S, Ctrl+Shift+S, Ctrl+E, Esc, Space) ────
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        let is_ed = Rc::clone(&is_editing_rc);
        let edit_toggle = edit_toggle_btn.clone();
        let saver = Rc::clone(&perform_save);
        let dlg_close = dlg.clone();

        key_ctrl.connect_key_pressed(move |_, key, _, state| {
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            let shift = state.contains(gdk::ModifierType::SHIFT_MASK);

            // Ctrl+S: Salvar
            if ctrl && (key == gdk::Key::s || key == gdk::Key::S) {
                saver(shift, None);
                return glib::Propagation::Stop;
            }

            // Ctrl+E: Alternar Modo Edição
            if ctrl && (key == gdk::Key::e || key == gdk::Key::E) {
                edit_toggle.set_active(!edit_toggle.is_active());
                return glib::Propagation::Stop;
            }

            // Escape: Fechar janela
            if key == gdk::Key::Escape {
                dlg_close.close();
                return glib::Propagation::Stop;
            }

            // Espaço: Fecha somente se em modo visualização (não editando)
            if key == gdk::Key::space && !*is_ed.borrow() {
                dlg_close.close();
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });

        dlg.add_controller(key_ctrl);
    }

    // Initial render
    render_entry();
    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guess_mime_type() {
        assert_eq!(guess_mime_type("desafio2.py"), "text/x-python");
        assert_eq!(guess_mime_type("main.rs"), "text/rust");
        assert_eq!(guess_mime_type("script.sh"), "text/x-shellscript");
        assert_eq!(guess_mime_type("config.json"), "application/json");
        assert_eq!(guess_mime_type("Cargo.toml"), "text/toml");
        assert_eq!(guess_mime_type("notes.md"), "text/markdown");
        assert_eq!(guess_mime_type("app.log"), "text/x-log");
        assert_eq!(guess_mime_type("unknown.xyz"), "text/plain");
    }

    #[test]
    fn test_file_line_count_metric() {
        let text = "dia = input('Dia: ')\nmes = input('Mês: ')\nano = input('Ano: ')\nprint('ok')";
        assert_eq!(text.lines().count(), 4);
    }

    #[test]
    fn test_byte_offset_to_char_offset_utf8() {
        let line = "ação = 'olá'";
        // "aç" -> 'a' (1 byte) + 'ç' (2 bytes) = 3 bytes -> char index 2
        assert_eq!(byte_offset_to_char_offset(line, 3), 2);
        // Past end safely clamps
        assert_eq!(byte_offset_to_char_offset(line, 999), line.chars().count() as i32);
    }

    #[test]
    fn test_is_log_file_and_zoom_and_hex_bounds() {
        assert!(is_log_file_path(Path::new("/var/log/syslog")));
        assert!(is_log_file_path(Path::new("server.log")));
        assert!(is_log_file_path(Path::new("app.log.1")));
        assert!(!is_log_file_path(Path::new("main.rs")));

        assert!((clamp_image_zoom(0.01) - 0.1).abs() < 1e-6);
        assert!((clamp_image_zoom(25.0) - 8.0).abs() < 1e-6);
        assert!((clamp_image_zoom(f64::NAN) - 1.0).abs() < 1e-6);

        assert_eq!(compute_hex_page_bounds(0, 0, 4096), (1, 0, 0));
        assert_eq!(compute_hex_page_bounds(5000, 0, 4096), (2, 0, 4096));
        assert_eq!(compute_hex_page_bounds(5000, 1, 4096), (2, 4096, 5000));
        assert_eq!(compute_hex_page_bounds(5000, 99, 4096), (2, 4096, 5000));
    }
}
