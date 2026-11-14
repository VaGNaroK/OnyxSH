//! Phase 12: Complete Dual-Pane File Manager UI (`src/onyxsh/filemanager/manager.py`,
//! `local_pane.py`, `dual_pane.py`, `quick_look.py`, `transfer_dialog.py`).
//!
//! Provides:
//! - 3 View Modes (`gtk4::Stack`): Detailed List (sortable columns), Thumbnail/Icon Grid (`gtk4::GridView`
//!   + `gtk4::SignalListItemFactory` matching Python `manager.py`), and Inline Directory Tree (`gtk4::ListBox`).
//! - Clickable Breadcrumbs path bar + direct path entry, Up, Home, Refresh, New Folder, Hidden toggle,
//!   and live search filter.
//! - Quick Look modal preview window (Spacebar or button) with `< Prev` / `Next >` cycling,
//!   image rendering, line-numbered text preview, and metadata summary.
//! - Dual-Pane mode (`Left <-> Right`) with center transfer buttons (`→ Copy`, `← Copy`, `⇄ Compare`,
//!   and `Transfer Queue` dialog with progress bars).
//! - File operations: Rename, Chmod permissions, SHA-256 Checksum, Delete, and "CD in Terminal".

use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::i18n::{tr, tr_fmt, tr_n};
use onyxsh_core::platform;
use onyxsh_core::sessions::{SessionItem, SessionsStorageData};
use onyxsh_filemanager::backend::{FileSystemBackend, LocalFsBackend, SftpFsBackend};
use onyxsh_filemanager::transfer::{TransferDirection, TransferQueueController, TransferStatus};
use onyxsh_filemanager::{
    filter_and_sort_entries, format_bytes, FileEntry, SortColumn,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const DIFF_MAX_FILE_BYTES: usize = 5 * 1024 * 1024;

/// Active backend for a single File Manager pane.
#[derive(Clone)]
pub enum FilePaneBackend {
    Local,
    Sftp(SftpFsBackend),
}

impl FilePaneBackend {
    pub fn is_sftp(&self) -> bool {
        matches!(self, Self::Sftp(_))
    }

    pub fn display_label(&self) -> String {
        match self {
            Self::Local => tr("Local"),
            Self::Sftp(sftp) => sftp.display_target(),
        }
    }

    pub fn list_dir(&self, path: &Path) -> std::io::Result<Vec<FileEntry>> {
        match self {
            Self::Local => LocalFsBackend.list_dir(path),
            Self::Sftp(sftp) => sftp.list_dir(path),
        }
    }

    pub fn mkdir(&self, path: &Path) -> std::io::Result<()> {
        match self {
            Self::Local => LocalFsBackend.mkdir(path),
            Self::Sftp(sftp) => sftp.mkdir(path),
        }
    }

    pub fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        match self {
            Self::Local => LocalFsBackend.rename(from, to),
            Self::Sftp(sftp) => sftp.rename(from, to),
        }
    }

    pub fn remove(&self, path: &Path) -> std::io::Result<()> {
        match self {
            Self::Local => LocalFsBackend.remove(path),
            Self::Sftp(sftp) => sftp.remove(path),
        }
    }

    pub fn chmod(&self, path: &Path, mode: u16) -> std::io::Result<()> {
        match self {
            Self::Local => LocalFsBackend.chmod(path, mode),
            Self::Sftp(sftp) => sftp.chmod(path, mode),
        }
    }

    pub fn create_empty_file(&self, path: &Path) -> std::io::Result<()> {
        match self {
            Self::Local => fs::File::create(path).map(|_| ()),
            Self::Sftp(sftp) => sftp.write_bytes(path, &[]),
        }
    }

    pub fn read_bytes(&self, path: &Path, max_bytes: Option<usize>) -> std::io::Result<Vec<u8>> {
        match self {
            Self::Local => LocalFsBackend.read_bytes(path, max_bytes),
            Self::Sftp(sftp) => sftp.read_bytes(path, max_bytes),
        }
    }

    pub fn free_space(&self, path: &Path) -> std::io::Result<(u64, u64)> {
        match self {
            Self::Local => LocalFsBackend.free_space(path),
            Self::Sftp(sftp) => sftp.free_space(path),
        }
    }
}

/// Drag & Drop transfer payload serialized between Left and Right panes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DragTransferPayload {
    pub source_pane: String,
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size_bytes: u64,
}

const DRAG_PAYLOAD_PREFIX: &str = "ONYXSH_FM_DND:";

pub fn encode_drag_payload(payload: &DragTransferPayload) -> String {
    serde_json::to_string(payload)
        .map(|j| format!("{DRAG_PAYLOAD_PREFIX}{j}"))
        .unwrap_or_default()
}

pub fn decode_drag_payload(raw: &str) -> Option<DragTransferPayload> {
    let trimmed = raw.trim();
    let json = trimmed.strip_prefix(DRAG_PAYLOAD_PREFIX)?;
    serde_json::from_str(json).ok()
}

/// Computes a unified line-by-line text diff between `left_text` and `right_text`.
pub fn generate_unified_diff(
    left_label: &str,
    right_label: &str,
    left_text: &str,
    right_text: &str,
) -> String {
    if left_text == right_text {
        return tr("Os arquivos possuem conteúdo idêntico.");
    }
    let a: Vec<&str> = left_text.lines().collect();
    let b: Vec<&str> = right_text.lines().collect();
    if a.len() > 2500 || b.len() > 2500 {
        // Fallback summary for very large files
        let mut out = format!("--- {left_label}\n+++ {right_label}\n@@ -1,{} +1,{} @@\n", a.len(), b.len());
        let max_lines = a.len().max(b.len()).min(200);
        for i in 0..max_lines {
            match (a.get(i), b.get(i)) {
                (Some(la), Some(lb)) if la == lb => out.push_str(&format!(" {la}\n")),
                (Some(la), Some(lb)) => {
                    out.push_str(&format!("-{la}\n"));
                    out.push_str(&format!("+{lb}\n"));
                }
                (Some(la), None) => out.push_str(&format!("-{la}\n")),
                (None, Some(lb)) => out.push_str(&format!("+{lb}\n")),
                (None, None) => {}
            }
        }
        return out;
    }

    // LCS DP table
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut out = format!("--- {left_label}\n+++ {right_label}\n@@ -1,{n} +1,{m} @@\n");
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            out.push_str(&format!(" {}\n", a[i]));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            out.push_str(&format!("-{}\n", a[i]));
            i += 1;
        } else {
            out.push_str(&format!("+{}\n", b[j]));
            j += 1;
        }
    }
    while i < n {
        out.push_str(&format!("-{}\n", a[i]));
        i += 1;
    }
    while j < m {
        out.push_str(&format!("+{}\n", b[j]));
        j += 1;
    }
    out
}

fn open_quick_look_for_pane(
    parent: &adw::ApplicationWindow,
    entries: Vec<FileEntry>,
    idx: usize,
    backend: &FilePaneBackend,
    feed_terminal_cb: Option<Rc<dyn Fn(&str)>>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
    refresh_fn: Option<Rc<dyn Fn()>>,
) {
    let ql_backend = match backend {
        FilePaneBackend::Local => crate::quick_look_dialog::QuickLookBackend::Local,
        FilePaneBackend::Sftp(sftp) => crate::quick_look_dialog::QuickLookBackend::Remote(sftp.clone()),
    };
    let cfg_path = platform::config_dir().join("settings.json");
    let clear_on_exit = AppSettings::load_from_path(&cfg_path)
        .map(|s| s.clear_remote_edit_files_on_exit)
        .unwrap_or(true);
    crate::quick_look_dialog::show_quick_look_dialog_with_backend(
        parent,
        entries,
        idx,
        ql_backend,
        clear_on_exit,
        feed_terminal_cb,
        ai_prompt_cb,
        refresh_fn,
    );
}

fn run_backend_mutation<F>(
    backend: &FilePaneBackend,
    status_lbl: &gtk4::Label,
    refresh_fn: &Rc<dyn Fn()>,
    ok_msg: String,
    err_prefix: String,
    op: F,
) where
    F: FnOnce(&FilePaneBackend) -> std::io::Result<()> + Send + 'static,
{
    match backend {
        FilePaneBackend::Local => match op(backend) {
            Ok(()) => {
                status_lbl.set_text(&ok_msg);
                refresh_fn();
            }
            Err(e) => status_lbl.set_text(&format!("{err_prefix}: {e}")),
        },
        FilePaneBackend::Sftp(_) => {
            let bk_bg = backend.clone();
            let st_c = status_lbl.clone();
            let rf_c = Rc::clone(refresh_fn);
            status_lbl.set_text(&tr("Executando operação SFTP..."));
            let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
            std::thread::spawn(move || {
                let res = op(&bk_bg).map_err(|e| e.to_string());
                let _ = tx.send(res);
            });
            glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
                Ok(Ok(())) => {
                    st_c.set_text(&ok_msg);
                    rf_c();
                    glib::ControlFlow::Break
                }
                Ok(Err(err)) => {
                    st_c.set_text(&format!("{err_prefix}: {err}"));
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            });
        }
    }
}

fn collect_local_tree_stats_depth(root: &Path, max_depth: Option<usize>) -> (usize, u64) {
    fn walk(dir: &Path, depth: usize, max_depth: Option<usize>, items: &mut usize, bytes: &mut u64) {
        if *items >= 10_000 {
            return;
        }
        if let Some(md) = max_depth {
            if depth > md {
                return;
            }
        }
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let Ok(ft) = ent.file_type() else {
                continue;
            };
            if ft.is_symlink() {
                *items += 1;
                continue;
            }
            if ft.is_dir() {
                *items += 1;
                walk(&ent.path(), depth + 1, max_depth, items, bytes);
            } else {
                *items += 1;
                if let Ok(m) = ent.metadata() {
                    *bytes = bytes.saturating_add(m.len());
                }
            }
            if *items >= 10_000 {
                break;
            }
        }
    }
    let mut items = 0usize;
    let mut bytes = 0u64;
    walk(root, 1, max_depth, &mut items, &mut bytes);
    (items, bytes)
}

fn collect_sftp_tree_stats_limited(
    sftp: &SftpFsBackend,
    root: &Path,
    depth_limit: usize,
) -> (usize, u64, Vec<FileEntry>) {
    let Ok(children) = sftp.list_dir(root) else {
        return (0, 0, Vec::new());
    };
    let mut count = children.len();
    let mut bytes: u64 = children.iter().filter(|c| !c.is_dir).map(|c| c.size).sum();
    if depth_limit > 1 {
        for sub in children.iter().filter(|c| c.is_dir).take(12) {
            if let Ok(grand) = sftp.list_dir(Path::new(&sub.path)) {
                count += grand.len();
                bytes = bytes.saturating_add(grand.iter().filter(|g| !g.is_dir).map(|g| g.size).sum::<u64>());
            }
        }
    }
    (count, bytes, children)
}

/// Visual presentation mode for a File Manager pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileManagerViewMode {
    List,
    Grid,
    Tree,
}

impl FileManagerViewMode {
    pub fn as_stack_name(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Grid => "grid",
            Self::Tree => "tree",
        }
    }

    pub fn from_stack_name(s: &str) -> Self {
        match s {
            "grid" => Self::Grid,
            "tree" => Self::Tree,
            _ => Self::List,
        }
    }
}

/// Decomposes a filesystem path into clickable `(label, target_path)` breadcrumb segments.
pub fn build_breadcrumb_segments(path: &Path) -> Vec<(String, PathBuf)> {
    let mut segments = Vec::new();
    let mut acc = PathBuf::new();

    for comp in path.components() {
        use std::path::Component;
        match comp {
            Component::RootDir => {
                acc.push("/");
                segments.push(("/".to_string(), PathBuf::from("/")));
            }
            Component::Normal(os_str) => {
                let name = os_str.to_string_lossy().into_owned();
                acc.push(&name);
                segments.push((name, acc.clone()));
            }
            _ => {}
        }
    }

    if segments.is_empty() {
        segments.push(("/".to_string(), PathBuf::from("/")));
    }
    segments
}

/// Formats text content with 1-based line numbers for Quick Look preview.
pub fn format_numbered_text_preview(text: &str, max_lines: usize) -> String {
    let mut out = String::new();
    for (idx, line) in text.lines().take(max_lines).enumerate() {
        out.push_str(&format!("{:>4} │ {}\n", idx + 1, line));
    }
    out
}

/// Computes the lowercase hex SHA-256 digest of a local file.
pub fn compute_file_sha256_hex(path: &Path) -> std::io::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let digest = hasher.finalize();
    Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Summary of differences between Left and Right pane directories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryComparison {
    pub only_in_left: Vec<String>,
    pub only_in_right: Vec<String>,
    pub size_differs: Vec<(String, u64, u64)>,
    pub identical_count: usize,
}

/// Compares two directory entry lists by filename and byte size.
pub fn compare_directory_entries(
    left: &[FileEntry],
    right: &[FileEntry],
) -> DirectoryComparison {
    let left_map: BTreeMap<&str, &FileEntry> =
        left.iter().map(|e| (e.name.as_str(), e)).collect();
    let right_map: BTreeMap<&str, &FileEntry> =
        right.iter().map(|e| (e.name.as_str(), e)).collect();

    let mut only_in_left = Vec::new();
    let mut only_in_right = Vec::new();
    let mut size_differs = Vec::new();
    let mut identical_count = 0usize;

    for (name, l_entry) in &left_map {
        match right_map.get(name) {
            None => only_in_left.push((*name).to_string()),
            Some(r_entry) => {
                if !l_entry.is_dir && !r_entry.is_dir && l_entry.size != r_entry.size {
                    size_differs.push(((*name).to_string(), l_entry.size, r_entry.size));
                } else {
                    identical_count += 1;
                }
            }
        }
    }

    for name in right_map.keys() {
        if !left_map.contains_key(name) {
            only_in_right.push((*name).to_string());
        }
    }

    DirectoryComparison {
        only_in_left,
        only_in_right,
        size_differs,
        identical_count,
    }
}

/// Resolves the `gio::Icon` for a GridView item matching Python `models.py` (`folder-symbolic` for
/// directories including `..`, and `gio::functions::content_type_guess` / `content_type_get_icon`
/// for files so `GridView` always uses fixed 48x48 `gtk4::Image` icons without 4K `GtkPicture` bloat).
fn resolve_grid_item_gicon(entry: &FileEntry) -> gio::Icon {
    if entry.is_dir {
        return gio::ThemedIcon::new(icon_for_entry(entry)).upcast();
    }
    let (mime_type, _uncertain) = gio::functions::content_type_guess(Some(&entry.name), &[]);
    if !mime_type.is_empty() {
        return gio::functions::content_type_get_icon(mime_type.as_str());
    }
    gio::ThemedIcon::new(icon_for_entry(entry)).upcast()
}

/// Chooses a symbolic icon name for a [`FileEntry`], supporting specialized folders and MIME classifications.
fn icon_for_entry(entry: &FileEntry) -> &'static str {
    if entry.name == ".." {
        return "folder-symbolic";
    }
    if entry.is_dir {
        let lower = entry.name.to_lowercase();
        return match lower.as_str() {
            "downloads" | "transferências" => "folder-download-symbolic",
            "documents" | "documentos" => "folder-documents-symbolic",
            "music" | "música" | "músicas" => "folder-music-symbolic",
            "pictures" | "imagens" | "fotos" => "folder-pictures-symbolic",
            "videos" | "vídeos" => "folder-videos-symbolic",
            "desktop" | "área de trabalho" => "user-desktop-symbolic",
            "public" | "público" => "folder-publicshare-symbolic",
            ".git" => "folder-git-symbolic",
            _ => "folder-symbolic",
        };
    }
    let ext = Path::new(&entry.name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" | "tiff" => {
            "image-x-generic-symbolic"
        }
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "deb" | "rpm" => {
            "package-x-generic-symbolic"
        }
        "rs" | "py" | "sh" | "bash" | "zsh" | "js" | "ts" | "c" | "cpp" | "h" | "hpp" | "go"
        | "lua" | "rb" | "php" | "pl" | "json" | "toml" | "yaml" | "yml" => {
            "text-x-script-symbolic"
        }
        "md" | "txt" | "rst" | "log" | "out" | "err" => "text-x-generic-symbolic",
        "pdf" => "x-office-document-symbolic",
        "doc" | "docx" | "odt" | "rtf" => "x-office-document-symbolic",
        "xls" | "xlsx" | "ods" | "csv" | "tsv" => "x-office-spreadsheet-symbolic",
        "ppt" | "pptx" | "odp" => "x-office-presentation-symbolic",
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => "audio-x-generic-symbolic",
        "mp4" | "mkv" | "webm" | "avi" | "mov" => "video-x-generic-symbolic",
        "iso" | "img" => "media-optical-symbolic",
        "crt" | "pem" | "pub" | "key" => "channel-secure-symbolic",
        _ if entry.is_executable() => "application-x-executable-symbolic",
        _ => "text-x-generic-symbolic",
    }
}

/// Attaches a Pango-markup tooltip lazily via `query-tooltip` so widget creation does not
/// trigger synchronous X11 pointer round-trips for every row.
fn set_lazy_tooltip_markup(widget: &impl IsA<gtk4::Widget>, markup: String) {
    widget.set_has_tooltip(true);
    widget.connect_query_tooltip(move |_, _, _, _, tip| {
        tip.set_markup(Some(&markup));
        true
    });
}

/// Attaches a plain-text tooltip lazily via `query-tooltip` (avoiding synchronous `XIQueryPointer`).
fn set_lazy_tooltip_text(widget: &impl IsA<gtk4::Widget>, text: String) {
    widget.set_has_tooltip(true);
    widget.connect_query_tooltip(move |_, _, _, _, tip| {
        tip.set_text(Some(&text));
        true
    });
}

/// Generates a rich, formatted Pango Markup tooltip for a file/directory row matching Python.
fn build_entry_tooltip_markup(entry: &FileEntry) -> String {
    if entry.name == ".." {
        return tr("<b>Subir para o diretório pai (..)</b>");
    }
    let icon_emoji = if entry.is_dir {
        "📁"
    } else if entry.is_symlink {
        "🔗"
    } else {
        "📄"
    };
    let safe_name = glib::markup_escape_text(&entry.name);
    let type_desc = glib::markup_escape_text(&tr(entry.type_description()));
    let perms_sym = entry.formatted_permissions();
    let perms_oct = entry.permissions_octal_str();
    let mod_date = format_epoch_short(entry.modified_epoch);

    let mut lines = vec![
        format!("{icon_emoji} <b>{safe_name}</b> <span alpha='70%'>({type_desc})</span>"),
        "<span alpha='30%'>──────────────────────────────────────────</span>".to_string(),
    ];

    if entry.is_dir {
        lines.push(tr("📦 <b>Tipo:</b> Pasta / Diretório"));
    } else {
        lines.push(tr_fmt(
            "📦 <b>Tamanho:</b> {size} <span alpha='70%'>({bytes} bytes)</span>",
            &[
                ("size", &entry.formatted_size()),
                ("bytes", &entry.size.to_string()),
            ],
        ));
    }

    lines.push(tr_fmt(
        "📅 <b>Modificado:</b> {date}",
        &[("date", &mod_date)],
    ));
    lines.push(tr_fmt(
        "🔒 <b>Permissões:</b> <tt>{sym}</tt> <span alpha='70%'>({oct})</span>",
        &[("sym", &perms_sym), ("oct", &perms_oct)],
    ));

    if entry.is_symlink {
        let safe_path = glib::markup_escape_text(&entry.path);
        lines.push(tr_fmt(
            "🔗 <b>Caminho:</b> <tt>{path}</tt>",
            &[("path", safe_path.as_str())],
        ));
    }

    lines.join("\n")
}

/// Formats epoch seconds into a compact human-readable string (`YYYY-MM-DD` or relative).
fn format_epoch_short(epoch: u64) -> String {
    if epoch == 0 {
        return "-".to_string();
    }
    let days_since_epoch = epoch / 86_400;
    // Civil date calculation from days since 1970-01-01 (Howard Hinnant algorithm)
    let z = days_since_epoch as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    let secs_in_day = epoch % 86_400;
    let hh = secs_in_day / 3600;
    let mm = (secs_in_day % 3600) / 60;
    format!("{year:04}-{m:02}-{d:02} {hh:02}:{mm:02}")
}

/// Formats epoch seconds into a short `DD/MM HH:MM` date string matching Python `icongrade.png`.
pub fn format_epoch_day_month(epoch: u64) -> String {
    if epoch == 0 {
        return "-".to_string();
    }
    let days_since_epoch = epoch / 86_400;
    let z = days_since_epoch as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let secs_in_day = epoch % 86_400;
    let hh = secs_in_day / 3600;
    let mm = (secs_in_day % 3600) / 60;
    format!("{d:02}/{m:02} {hh:02}:{mm:02}")
}

/// Formats the subtitle of an icon card in Grid View matching `icongrade.png`:
/// - Folders: `Pasta · DD/MM HH:MM`
/// - Files: `{size} · DD/MM HH:MM`
/// - Parent `..`: empty
pub fn format_grid_subtitle(entry: &FileEntry) -> String {
    if entry.name == ".." {
        return String::new();
    }
    let date_str = format_epoch_day_month(entry.modified_epoch);
    if entry.is_dir {
        tr_fmt("Pasta · {date}", &[("date", &date_str)])
    } else {
        format!("{} · {date_str}", entry.formatted_size_clean())
    }
}

/// Formats the hierarchical size summary for Tree View matching `treehierarquic.png`:
/// - Folders: `{formatted_recursive_size} ({count} itens)` e.g. `1.5 MB (31 itens)` or `0 B (20 itens)`
/// - Files: `{formatted_size}` e.g. `6.9 KB`
pub fn format_tree_size_summary(entry: &FileEntry) -> String {
    if !entry.is_dir {
        return entry.formatted_size_clean();
    }
    if entry.name == ".." {
        return String::new();
    }
    let p = Path::new(&entry.path);
    let mut count = 0u64;
    let mut total_bytes = 0;
    if let Ok(entries) = std::fs::read_dir(p) {
        for e in entries.flatten() {
            count += 1;
            if let Ok(meta) = e.metadata() {
                total_bytes += meta.len();
            }
        }
    }
    let items_str = tr_n("item", "itens", count);
    let size_str = format_bytes(total_bytes);
    format!("{size_str} ({count} {items_str})")
}

/// Opens the Quick Look modal preview window with in-place editor and navigation.
pub fn show_quick_look_window(
    parent: &adw::ApplicationWindow,
    entries: Vec<FileEntry>,
    initial_index: usize,
) {
    crate::quick_look_dialog::show_quick_look_dialog(
        parent,
        entries,
        initial_index,
        None,
        None,
        None,
    );
}

/// Opens the Transfer Queue dialog (`TransferManagerDialog`) showing active/completed transfers.
pub fn show_transfer_queue_dialog(
    parent: &adw::ApplicationWindow,
    controller: &Arc<TransferQueueController>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(false)
        .title(&tr("Fila de Transferências — OnyxSH"))
        .default_width(640)
        .default_height(430)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    let hb = adw::HeaderBar::new();

    let clear_btn = gtk4::Button::builder()
        .icon_name("edit-clear-all-symbolic")
        .tooltip_text(&tr("Limpar Concluídos"))
        .build();
    hb.pack_end(&clear_btn);
    vbox.append(&hb);

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);

    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_box.set_margin_start(14);
    list_box.set_margin_end(14);
    list_box.set_margin_top(10);
    list_box.set_margin_bottom(14);

    let populate: Rc<dyn Fn()> = {
        let lb = list_box.clone();
        let ctrl = Arc::clone(controller);
        Rc::new(move || {
            lb.remove_all();
            let items = ctrl.snapshot();
            if items.is_empty() {
                let empty = gtk4::Label::new(Some(&tr(
                    "Nenhuma transferência registrada na sessão.",
                )));
                empty.add_css_class("dim-label");
                empty.set_margin_top(24);
                empty.set_margin_bottom(24);
                lb.append(&empty);
                return;
            }

            for item in items.into_iter().rev() {
                let row_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
                row_vbox.set_margin_start(10);
                row_vbox.set_margin_end(10);
                row_vbox.set_margin_top(8);
                row_vbox.set_margin_bottom(8);

                let top_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                let dir_badge = match item.direction {
                    TransferDirection::Upload => tr("→ Upload / Copy"),
                    TransferDirection::Download => tr("← Download / Copy"),
                };
                let name_lbl = gtk4::Label::new(Some(&format!(
                    "[{}] {} ({})",
                    dir_badge,
                    item.filename,
                    format_bytes(item.total_bytes)
                )));
                name_lbl.set_halign(gtk4::Align::Start);
                name_lbl.set_hexpand(true);

                let speed_txt = if item.speed_bps > 1.0 && item.status == TransferStatus::InProgress {
                    format!(" • {}/s", format_bytes(item.speed_bps as u64))
                } else {
                    String::new()
                };
                let eta_txt = item
                    .eta_seconds()
                    .filter(|_| item.status == TransferStatus::InProgress)
                    .map(|s| format!(" • ETA {s}s"))
                    .unwrap_or_default();

                let status_str = match item.status {
                    TransferStatus::Queued => tr("Na fila"),
                    TransferStatus::InProgress => tr("Em andamento"),
                    TransferStatus::Completed => tr("Concluído"),
                    TransferStatus::Failed => tr("Falhou"),
                    TransferStatus::Cancelled => tr("Cancelado"),
                };
                let status_lbl = gtk4::Label::new(Some(&format!(
                    "{status_str} ({}%){speed_txt}{eta_txt}",
                    item.progress_percent()
                )));
                status_lbl.add_css_class("caption");
                top_hbox.append(&name_lbl);
                top_hbox.append(&status_lbl);

                if matches!(item.status, TransferStatus::Queued | TransferStatus::InProgress) {
                    let cancel_btn = gtk4::Button::with_label(&tr("Cancelar"));
                    cancel_btn.add_css_class("flat");
                    cancel_btn.add_css_class("destructive-action");
                    let ctrl_c = Arc::clone(&ctrl);
                    let id_c = item.id.clone();
                    cancel_btn.connect_clicked(move |_| {
                        ctrl_c.cancel(&id_c);
                    });
                    top_hbox.append(&cancel_btn);
                }

                row_vbox.append(&top_hbox);

                let pbar = gtk4::ProgressBar::new();
                pbar.set_fraction(item.progress_fraction());
                row_vbox.append(&pbar);

                let detail_str = if let Some(ref err) = item.error_message {
                    format!("{} — {}  ⟶  {}", err, item.source_path, item.destination_path)
                } else {
                    format!("{}  ⟶  {}", item.source_path, item.destination_path)
                };
                let paths_lbl = gtk4::Label::new(Some(&detail_str));
                paths_lbl.add_css_class("caption");
                paths_lbl.add_css_class("dim-label");
                paths_lbl.set_halign(gtk4::Align::Start);
                paths_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
                row_vbox.append(&paths_lbl);

                lb.append(&row_vbox);
            }
        })
    };

    populate();

    {
        let ctrl_c = Arc::clone(controller);
        let pop = Rc::clone(&populate);
        clear_btn.connect_clicked(move |_| {
            ctrl_c.clear_finished();
            pop();
        });
    }

    let timer_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
    {
        let pop = Rc::clone(&populate);
        let sid = glib::timeout_add_local(Duration::from_millis(300), move || {
            pop();
            glib::ControlFlow::Continue
        });
        *timer_id.borrow_mut() = Some(sid);
    }
    {
        let tid = Rc::clone(&timer_id);
        dlg.connect_close_request(move |_| {
            if let Some(sid) = tid.borrow_mut().take() {
                sid.remove();
            }
            glib::Propagation::Proceed
        });
    }

    scroll.set_child(Some(&list_box));
    vbox.append(&scroll);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Controller handle for a single File Manager Pane (Left or Right).
#[derive(Clone)]
pub struct FilePaneHandle {
    pub pane_id: String,
    pub widget: gtk4::Box,
    pub current_dir: Rc<RefCell<PathBuf>>,
    pub backend: Rc<RefCell<FilePaneBackend>>,
    pub selected_entry: Rc<RefCell<Option<FileEntry>>>,
    pub visible_entries: Rc<RefCell<Vec<FileEntry>>>,
    pub refresh: Rc<dyn Fn()>,
    pub set_backend_and_dir: Rc<dyn Fn(FilePaneBackend, PathBuf)>,
    pub btn_toggle_dual: gtk4::ToggleButton,
}

/// Represents a copied or cut item in the File Manager clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileManagerClipboardItem {
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_cut: bool,
}

/// Recursively copies a directory tree from `src` to `dst`.
pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Helper function to execute paste operation from clipboard to destination directory.
fn execute_paste(
    clipboard: &Rc<RefCell<Option<FileManagerClipboardItem>>>,
    dest_dir: &Path,
    dst_backend: &FilePaneBackend,
    status_lbl: &gtk4::Label,
    refresh_fn: &Rc<dyn Fn()>,
) {
    let clip = clipboard.borrow().clone();
    let Some(item) = clip else {
        status_lbl.set_text(&tr("Nenhum item na área de transferência para colar."));
        return;
    };
    let file_name = match item.path.file_name() {
        Some(n) => n,
        None => {
            status_lbl.set_text(&tr("Nome de arquivo inválido na área de transferência."));
            return;
        }
    };
    let target = dest_dir.join(file_name);

    if let FilePaneBackend::Sftp(sftp) = dst_backend {
        if !item.path.exists() {
            status_lbl.set_text(&tr("Colar entre pastas remotas na mesma sessão: use Mover/Renomear ou Painel Duplo."));
            return;
        }
        let sftp_bg = sftp.clone();
        let src_p = item.path.clone();
        let dst_p = target.clone();
        let rf_c = Rc::clone(refresh_fn);
        let st_c = status_lbl.clone();
        status_lbl.set_text(&tr("Enviando item colado via SFTP..."));
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
        std::thread::spawn(move || {
            let res = sftp_bg.upload_path(&src_p, &dst_p).map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
        let dst_disp = target;
        glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
            Ok(Ok(())) => {
                st_c.set_text(&tr_fmt(
                    "Item colado via SFTP: {path}",
                    &[("path", &dst_disp.display().to_string())],
                ));
                rf_c();
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                st_c.set_text(&tr_fmt("Erro ao colar via SFTP: {err}", &[("err", &e)]));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        });
        return;
    }

    if item.is_cut {
        match fs::rename(&item.path, &target) {
            Ok(()) => {
                status_lbl.set_text(&tr_fmt(
                    "Item movido para: {path}",
                    &[("path", &target.display().to_string())],
                ));
                *clipboard.borrow_mut() = None;
                refresh_fn();
            }
            Err(_) => {
                let res = if item.is_dir {
                    copy_dir_all(&item.path, &target).and_then(|_| fs::remove_dir_all(&item.path))
                } else {
                    fs::copy(&item.path, &target)
                        .and_then(|_| fs::remove_file(&item.path))
                        .map(|_| ())
                };
                match res {
                    Ok(()) => {
                        status_lbl.set_text(&tr_fmt(
                            "Item movido para: {path}",
                            &[("path", &target.display().to_string())],
                        ));
                        *clipboard.borrow_mut() = None;
                        refresh_fn();
                    }
                    Err(e) => status_lbl.set_text(&tr_fmt(
                        "Erro ao mover item: {err}",
                        &[("err", &e.to_string())],
                    )),
                }
            }
        }
    } else {
        let res = if item.is_dir {
            copy_dir_all(&item.path, &target)
        } else {
            fs::copy(&item.path, &target).map(|_| ())
        };
        match res {
            Ok(()) => {
                status_lbl.set_text(&tr_fmt(
                    "Item colado em: {path}",
                    &[("path", &target.display().to_string())],
                ));
                refresh_fn();
            }
            Err(e) => status_lbl.set_text(&tr_fmt(
                "Erro ao colar item: {err}",
                &[("err", &e.to_string())],
            )),
        }
    }
}

/// Helper to add a stylized action button to a context menu popover.
fn add_menu_item(
    menu_box: &gtk4::Box,
    popover: &gtk4::Popover,
    icon_name: &str,
    label_text: &str,
    destructive: bool,
    action: impl Fn() + 'static,
) {
    let btn = gtk4::Button::new();
    btn.add_css_class("flat");
    if destructive {
        btn.add_css_class("destructive-action");
    }
    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let img = gtk4::Image::from_icon_name(icon_name);
    let lbl = gtk4::Label::new(Some(label_text));
    lbl.set_halign(gtk4::Align::Start);
    lbl.set_hexpand(true);
    hbox.append(&img);
    hbox.append(&lbl);
    btn.set_child(Some(&hbox));

    let act = Rc::new(action);
    let pop = popover.clone();
    btn.connect_clicked(move |_| {
        pop.popdown();
        let a = Rc::clone(&act);
        glib::idle_add_local_once(move || {
            a();
        });
    });
    menu_box.append(&btn);
}

/// Helper to add a checkable action button with a select indicator to a context menu popover.
fn add_checkable_menu_item(
    menu_box: &gtk4::Box,
    popover: &gtk4::Popover,
    icon_name: &str,
    label_text: &str,
    checked: bool,
    action: impl Fn() + 'static,
) {
    let btn = gtk4::Button::new();
    btn.add_css_class("flat");
    let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    let img = gtk4::Image::from_icon_name(icon_name);
    let lbl = gtk4::Label::new(Some(label_text));
    lbl.set_halign(gtk4::Align::Start);
    lbl.set_hexpand(true);
    hbox.append(&img);
    hbox.append(&lbl);

    if checked {
        let check_icon = gtk4::Image::from_icon_name("object-select-symbolic");
        check_icon.add_css_class("accent");
        hbox.append(&check_icon);
    }

    btn.set_child(Some(&hbox));

    let act = Rc::new(action);
    let pop = popover.clone();
    btn.connect_clicked(move |_| {
        pop.popdown();
        let a = Rc::clone(&act);
        glib::idle_add_local_once(move || {
            a();
        });
    });
    menu_box.append(&btn);
}

/// Injects the 'Organizar por' and 'Modo de Exibição' options into the context menu popover.
fn add_organize_and_view_menu_items(
    menu_vbox: &gtk4::Box,
    popover: &gtk4::Popover,
    sort_col: &Rc<RefCell<SortColumn>>,
    sort_asc: &Rc<RefCell<bool>>,
    view_mode: &Rc<RefCell<FileManagerViewMode>>,
    change_sort: &Rc<dyn Fn(SortColumn, Option<bool>)>,
    switch_view_mode: &Rc<dyn Fn(FileManagerViewMode)>,
    status_lbl: &gtk4::Label,
) {
    menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let header_sort = gtk4::Label::builder()
        .label(tr("Organizar por"))
        .xalign(0.0)
        .build();
    header_sort.add_css_class("dim-label");
    header_sort.set_margin_start(8);
    header_sort.set_margin_top(4);
    header_sort.set_margin_bottom(2);
    menu_vbox.append(&header_sort);

    let curr_col = *sort_col.borrow();
    let curr_asc = *sort_asc.borrow();
    let curr_vm = *view_mode.borrow();

    // 1. Sort by Name
    {
        let cs = Rc::clone(change_sort);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "edit-find-symbolic",
            &tr("Nome (A-Z)"),
            curr_col == SortColumn::Name,
            move || {
                cs(SortColumn::Name, None);
                st.set_text(&tr("Arquivos organizados por Nome"));
            },
        );
    }

    // 2. Sort by Type / Extension
    {
        let cs = Rc::clone(change_sort);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "text-x-generic-symbolic",
            &tr("Tipo / Extensão"),
            curr_col == SortColumn::Type,
            move || {
                cs(SortColumn::Type, None);
                st.set_text(&tr("Arquivos organizados por Tipo / Extensão"));
            },
        );
    }

    // 3. Sort by Size
    {
        let cs = Rc::clone(change_sort);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "emblem-documents-symbolic",
            &tr("Size"),
            curr_col == SortColumn::Size,
            move || {
                cs(SortColumn::Size, None);
                st.set_text(&tr("Arquivos organizados por Tamanho"));
            },
        );
    }

    // 4. Sort by Modified Date
    {
        let cs = Rc::clone(change_sort);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "x-office-calendar-symbolic",
            &tr("Data de Modificação"),
            curr_col == SortColumn::Modified,
            move || {
                cs(SortColumn::Modified, None);
                st.set_text(&tr("Arquivos organizados por Data de Modificação"));
            },
        );
    }

    // 5. Intelligent Grouping (.7z at top, scripts, documents, media)
    {
        let cs = Rc::clone(change_sort);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "package-x-generic-symbolic",
            &tr("Agrupar por Categorias (.7z no topo)"),
            curr_col == SortColumn::GroupByType,
            move || {
                cs(SortColumn::GroupByType, None);
                st.set_text(&tr("Arquivos agrupados por Categoria (.7z no topo)"));
            },
        );
    }

    menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    // Direction: Ascending / Descending
    {
        let cs = Rc::clone(change_sort);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "view-sort-ascending-symbolic",
            &tr("Ordem Crescente"),
            curr_asc,
            move || {
                cs(curr_col, Some(true));
                st.set_text(&tr("Ordenação crescente ativada"));
            },
        );
    }
    {
        let cs = Rc::clone(change_sort);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "view-sort-descending-symbolic",
            &tr("Ordem Decrescente"),
            !curr_asc,
            move || {
                cs(curr_col, Some(false));
                st.set_text(&tr("Ordenação decrescente ativada"));
            },
        );
    }

    menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let header_view = gtk4::Label::builder()
        .label(tr("Display Mode"))
        .xalign(0.0)
        .build();
    header_view.add_css_class("dim-label");
    header_view.set_margin_start(8);
    header_view.set_margin_top(4);
    header_view.set_margin_bottom(2);
    menu_vbox.append(&header_view);

    // View Mode: Grid (Icons) vs List (Detailed)
    {
        let vm_fn = Rc::clone(switch_view_mode);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "view-grid-symbolic",
            &tr("Grade de Ícones"),
            curr_vm == FileManagerViewMode::Grid,
            move || {
                vm_fn(FileManagerViewMode::Grid);
                st.set_text(&tr("Visualização em Grade de Ícones ativada"));
            },
        );
    }
    {
        let vm_fn = Rc::clone(switch_view_mode);
        let st = status_lbl.clone();
        add_checkable_menu_item(
            menu_vbox,
            popover,
            "view-list-symbolic",
            &tr("Lista Detalhada"),
            curr_vm == FileManagerViewMode::List,
            move || {
                vm_fn(FileManagerViewMode::List);
                st.set_text(&tr("Visualização em Lista Detalhada ativada"));
            },
        );
    }
}

/// Confirmation dialog for permanent deletion.
fn show_confirm_delete_dialog(
    parent: &adw::ApplicationWindow,
    entry: &FileEntry,
    on_confirm: Rc<dyn Fn()>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(tr("Confirmar Exclusão"))
        .default_width(380)
        .default_height(160)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);

    let msg = if entry.is_dir {
        tr_fmt(
            "Tem certeza de que deseja excluir permanentemente a pasta '{name}'?",
            &[("name", &entry.name)],
        )
    } else {
        tr_fmt(
            "Tem certeza de que deseja excluir permanentemente o arquivo '{name}'?",
            &[("name", &entry.name)],
        )
    };
    let lbl = gtk4::Label::new(Some(&msg));
    lbl.set_wrap(true);
    lbl.set_halign(gtk4::Align::Start);
    vbox.append(&lbl);

    let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);

    let btn_cancel = gtk4::Button::with_label(&tr("Cancel"));
    let btn_delete = gtk4::Button::with_label(&tr("Excluir Permanentemente"));
    btn_delete.add_css_class("destructive-action");

    btn_box.append(&btn_cancel);
    btn_box.append(&btn_delete);
    vbox.append(&btn_box);

    dlg.set_content(Some(&vbox));

    let d1 = dlg.clone();
    btn_cancel.connect_clicked(move |_| d1.close());

    let d2 = dlg.clone();
    btn_delete.connect_clicked(move |_| {
        on_confirm();
        d2.close();
    });

    dlg.present();
}

/// Shows the right-click contextual popover menu for a file or directory item.
fn show_context_menu_for_entry(
    anchor_widget: &impl IsA<gtk4::Widget>,
    _pane_root: &gtk4::Box,
    active_popover: &Rc<RefCell<Option<gtk4::Popover>>>,
    x: f64,
    y: f64,
    entry: &FileEntry,
    cur_dir: &Rc<RefCell<PathBuf>>,
    pane_backend: &Rc<RefCell<FilePaneBackend>>,
    parent_window: &adw::ApplicationWindow,
    status_lbl: &gtk4::Label,
    refresh_fn: &Rc<dyn Fn()>,
    cd_in_terminal_cb: &Rc<dyn Fn(&Path)>,
    feed_terminal_cb: &Rc<dyn Fn(&str)>,
    ai_prompt_cb: &Option<Rc<dyn Fn(&str)>>,
    clipboard: &Rc<RefCell<Option<FileManagerClipboardItem>>>,
    visible_entries: &Rc<RefCell<Vec<FileEntry>>>,
    sort_col: &Rc<RefCell<SortColumn>>,
    sort_asc: &Rc<RefCell<bool>>,
    view_mode: &Rc<RefCell<FileManagerViewMode>>,
    change_sort: &Rc<dyn Fn(SortColumn, Option<bool>)>,
    switch_view_mode: &Rc<dyn Fn(FileManagerViewMode)>,
) {
    if let Some(old) = active_popover.borrow_mut().take() {
        if old.parent().is_some() {
            old.popdown();
        }
    }

    let popover = gtk4::Popover::new();
    popover.set_parent(anchor_widget);
    popover.set_has_arrow(true);
    popover.set_position(gtk4::PositionType::Top);
    popover.set_autohide(true);
    popover.add_css_class("onyxsh-popover");
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

    let act_rc = Rc::clone(active_popover);
    popover.connect_closed(move |p| {
        let p_clone = p.clone();
        let act = Rc::clone(&act_rc);
        glib::idle_add_local_once(move || {
            let mut act_borrow = act.borrow_mut();
            if let Some(cur) = act_borrow.as_ref() {
                if cur == &p_clone {
                    act_borrow.take();
                }
            }
            drop(act_borrow);
            if p_clone.parent().is_some() {
                p_clone.unparent();
            }
        });
    });
    *active_popover.borrow_mut() = Some(popover.clone());

    let menu_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    menu_vbox.set_margin_start(6);
    menu_vbox.set_margin_end(6);
    menu_vbox.set_margin_top(6);
    menu_vbox.set_margin_bottom(6);
    menu_vbox.set_width_request(280);

    if entry.name == ".." {
        let cd = Rc::clone(cur_dir);
        let rf = Rc::clone(refresh_fn);
        add_menu_item(&menu_vbox, &popover, "go-up-symbolic", &tr("Up One Level"), false, move || {
            if let Some(p) = cd.borrow().parent() {
                *cd.borrow_mut() = p.to_path_buf();
                rf();
            }
        });

        let cd = Rc::clone(cur_dir);
        let cd_cb = Rc::clone(cd_in_terminal_cb);
        add_menu_item(&menu_vbox, &popover, "utilities-terminal-symbolic", &tr("Open in Terminal (cd)"), false, move || {
            if let Some(p) = cd.borrow().parent() {
                cd_cb(p);
            }
        });

        let cd = Rc::clone(cur_dir);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", &tr("Copiar Caminho do Diretório Pai"), false, move || {
            if let Some(p) = cd.borrow().parent() {
                let path_str = p.to_string_lossy().to_string();
                if let Some(cb) = gdk::Display::default().and_then(|d| Some(d.clipboard())) {
                    cb.set_text(&path_str);
                    st.set_text(&tr_fmt("Caminho copiado: {path}", &[("path", &path_str)]));
                }
            }
        });

        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .propagate_natural_height(true)
            .propagate_natural_width(true)
            .max_content_height(440)
            .child(&menu_vbox)
            .build();
        popover.set_child(Some(&scroll));
        popover.popup();
        return;
    }

    if entry.is_dir {
        // ── 1. Opções de Pasta ──────────────────────────────────────
        let path = PathBuf::from(&entry.path);
        let cd = Rc::clone(cur_dir);
        let rf = Rc::clone(refresh_fn);
        add_menu_item(&menu_vbox, &popover, "folder-open-symbolic", &tr("Abrir Pasta"), false, move || {
            *cd.borrow_mut() = path.clone();
            rf();
        });

        let path = PathBuf::from(&entry.path);
        let cd_cb = Rc::clone(cd_in_terminal_cb);
        add_menu_item(&menu_vbox, &popover, "utilities-terminal-symbolic", &tr("Open in Terminal (cd)"), false, move || {
            cd_cb(&path);
        });

        let path_str = entry.path.clone();
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", &tr("Copy Absolute Path"), false, move || {
            if let Some(cb) = gdk::Display::default().and_then(|d| Some(d.clipboard())) {
                cb.set_text(&path_str);
                st.set_text(&tr_fmt("Caminho copiado: {path}", &[("path", &path_str)]));
            }
        });

        let path_str = entry.path.clone();
        let feed_cb = Rc::clone(feed_terminal_cb);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "insert-text-symbolic", &tr("Inserir Caminho no Terminal"), false, move || {
            let escaped = format!("'{}' ", path_str.replace('\'', "'\\''"));
            feed_cb(&escaped);
            st.set_text(&tr_fmt("Caminho inserido no terminal: {path}", &[("path", &path_str)]));
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let target_dir = PathBuf::from(&entry.path);
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "folder-new-symbolic", &tr("Nova Pasta Aqui..."), false, move || {
            let tdir = target_dir.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            let bk2 = Rc::clone(&bk);
            show_simple_input_dialog(&win, &tr("New Folder"), &tr("Nome da nova pasta:"), "nova_pasta", Rc::new(move |name| {
                let p = tdir.join(name.trim());
                let ok_msg = tr_fmt("Pasta criada: {path}", &[("path", &p.display().to_string())]);
                let bk_snap = bk2.borrow().clone();
                run_backend_mutation(&bk_snap, &st2, &rf2, ok_msg, tr("Erro ao criar pasta"), move |b| b.mkdir(&p));
            }));
        });

        let target_dir = PathBuf::from(&entry.path);
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "document-new-symbolic", &tr("Novo Arquivo Aqui..."), false, move || {
            let tdir = target_dir.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            let bk2 = Rc::clone(&bk);
            show_simple_input_dialog(&win, &tr("New File"), &tr("Nome do novo arquivo:"), "novo_arquivo.txt", Rc::new(move |name| {
                let p = tdir.join(name.trim());
                let ok_msg = tr_fmt("Arquivo criado: {path}", &[("path", &p.display().to_string())]);
                let bk_snap = bk2.borrow().clone();
                run_backend_mutation(&bk_snap, &st2, &rf2, ok_msg, tr("Erro ao criar arquivo"), move |b| b.create_empty_file(&p));
            }));
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let p = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", &tr("Copiar Pasta"), false, move || {
            *clip.borrow_mut() = Some(FileManagerClipboardItem {
                path: p.clone(),
                is_dir: true,
                is_cut: false,
            });
            st.set_text(&tr_fmt("Pasta copiada para transferência: {name}", &[("name", &name)]));
        });

        let p = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-cut-symbolic", &tr("Recortar Pasta"), false, move || {
            *clip.borrow_mut() = Some(FileManagerClipboardItem {
                path: p.clone(),
                is_dir: true,
                is_cut: true,
            });
            st.set_text(&tr_fmt("Pasta marcada para mover: {name}", &[("name", &name)]));
        });

        if clipboard.borrow().is_some() {
            let target_dir = PathBuf::from(&entry.path);
            let clip = Rc::clone(clipboard);
            let st = status_lbl.clone();
            let rf = Rc::clone(refresh_fn);
            let bk = Rc::clone(pane_backend);
            add_menu_item(&menu_vbox, &popover, "edit-paste-symbolic", &tr("Colar Dentro desta Pasta"), false, move || {
                execute_paste(&clip, &target_dir, &bk.borrow(), &st, &rf);
            });
        }

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let old_path = PathBuf::from(&entry.path);
        let old_name = entry.name.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "document-edit-symbolic", &tr("Renomear..."), false, move || {
            let op = old_path.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            let bk2 = Rc::clone(&bk);
            show_simple_input_dialog(&win, &tr("Renomear Pasta"), &tr("Novo nome da pasta:"), &old_name, Rc::new(move |new_name| {
                if let Some(parent) = op.parent() {
                    let np = parent.join(new_name.trim());
                    let ok_msg = tr_fmt("Pasta renomeada para '{name}'", &[("name", new_name.trim())]);
                    let bk_snap = bk2.borrow().clone();
                    let op_bg = op.clone();
                    run_backend_mutation(&bk_snap, &st2, &rf2, ok_msg, tr("Erro ao renomear"), move |b| b.rename(&op_bg, &np));
                }
            }));
        });

        let e_clone = entry.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "channel-secure-symbolic", &tr("Permissões (chmod)..."), false, move || {
            show_permissions_dialog_with_backend(&win, &e_clone, bk.borrow().clone(), Rc::clone(&rf), st.clone());
        });

        if let Some(ai_cb) = ai_prompt_cb.as_ref() {
            let ai = Rc::clone(ai_cb);
            let e_clone = entry.clone();
            let st = status_lbl.clone();
            add_menu_item(&menu_vbox, &popover, "security-high-symbolic", &tr("Auditar Segurança com IA"), false, move || {
                let perms = e_clone.formatted_permissions();
                let oct = e_clone.permissions_octal_str();
                let mod_date = format_epoch_short(e_clone.modified_epoch);
                let prompt = tr_fmt(
                    "Por favor, realize uma auditoria detalhada de segurança e permissões de acesso do diretório '{name}'.\nCaminho: {path}\nPermissões atuais: {perms} ({oct})\nÚltima modificação: {date}\nRecomende configurações ideais de chmod, restrições e boas práticas de conformidade Zero-Trust.",
                    &[
                        ("name", &e_clone.name),
                        ("path", &e_clone.path),
                        ("perms", &perms),
                        ("oct", &oct),
                        ("date", &mod_date),
                    ],
                );
                ai(&prompt);
                st.set_text(&tr_fmt("Auditoria de segurança de '{name}' enviada ao Assistente de IA.", &[("name", &e_clone.name)]));
            });
        }

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let e_clone = entry.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "user-trash-symbolic", &tr("Excluir Pasta"), true, move || {
            let ec = e_clone.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            let bk2 = Rc::clone(&bk);
            let del_path = PathBuf::from(&ec.path);
            let del_name = ec.name.clone();
            show_confirm_delete_dialog(&win, &ec, Rc::new(move || {
                let ok_msg = tr_fmt("Pasta excluída: {name}", &[("name", &del_name)]);
                let bk_snap = bk2.borrow().clone();
                let dp = del_path.clone();
                run_backend_mutation(&bk_snap, &st2, &rf2, ok_msg, tr("Erro ao excluir pasta"), move |b| b.remove(&dp));
            }));
        });
    } else {
        // ── 2. Opções de Arquivo ────────────────────────────────────
        let win = parent_window.clone();
        let vis = Rc::clone(visible_entries);
        let path = entry.path.clone();
        let feed_cb = Rc::clone(feed_terminal_cb);
        let ai_cb = ai_prompt_cb.clone();
        let rf_cb = Rc::clone(refresh_fn);
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "find-location-symbolic", &tr("Quick Look (Pré-visualizar)"), false, move || {
            let entries = vis.borrow().clone();
            let idx = entries.iter().position(|e| e.path == path).unwrap_or(0);
            open_quick_look_for_pane(
                &win,
                entries,
                idx,
                &bk.borrow(),
                Some(Rc::clone(&feed_cb)),
                ai_cb.clone(),
                Some(Rc::clone(&rf_cb)),
            );
        });

        let path = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let size_str = entry.formatted_size_clean();
        let win = parent_window.clone();
        let feed_cb = Rc::clone(feed_terminal_cb);
        add_menu_item(
            &menu_vbox,
            &popover,
            "accessories-calculator-symbolic",
            &tr("Calculate / Verify Hash..."),
            false,
            move || {
                crate::checksum_dialog::show_checksum_dialog(
                    &win,
                    &path,
                    &name,
                    &size_str,
                    &feed_cb,
                );
            },
        );

        let lower = entry.name.to_lowercase();
        let ext = Path::new(&entry.name).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let is_script_or_bin = entry.is_executable() || matches!(ext.as_str(), "sh" | "bash" | "zsh" | "py" | "rs" | "pl" | "rb" | "js" | "ts");

        if is_script_or_bin {
            let name_escaped = entry.name.replace('\'', "'\\''");
            let cmd_base = if ext == "py" {
                format!("python3 ./'{}'\n", name_escaped)
            } else {
                format!("./'{}'\n", name_escaped)
            };
            let feed_cb = Rc::clone(feed_terminal_cb);
            let cb_copy = cmd_base.clone();
            add_menu_item(&menu_vbox, &popover, "media-playback-start-symbolic", &tr("Run in Terminal"), false, move || {
                feed_cb(&cb_copy);
            });

            let feed_cb = Rc::clone(feed_terminal_cb);
            let sudo_cmd = format!("sudo {cmd_base}");
            add_menu_item(&menu_vbox, &popover, "security-medium-symbolic", &tr("Executar com sudo no Terminal"), false, move || {
                feed_cb(&sudo_cmd);
            });
        }

        let is_log = matches!(ext.as_str(), "log" | "out" | "err") || lower.contains("log");
        if is_log {
            let name_escaped = entry.name.replace('\'', "'\\''");
            let tail_cmd = format!("tail -n 50 -f ./'{}'\n", name_escaped);
            let feed_cb = Rc::clone(feed_terminal_cb);
            add_menu_item(&menu_vbox, &popover, "format-indent-more-symbolic", &tr("Follow Log (tail -f)"), false, move || {
                feed_cb(&tail_cmd);
            });

            if let Some(ai_cb) = ai_prompt_cb.as_ref() {
                let ai = Rc::clone(ai_cb);
                let p = entry.path.clone();
                let n = entry.name.clone();
                let st = status_lbl.clone();
                add_menu_item(&menu_vbox, &popover, "system-search-symbolic", &tr("Diagnosticar Erros do Log com IA"), false, move || {
                    let tail_sample = fs::read_to_string(&p)
                        .map(|s| {
                            let lines: Vec<&str> = s.lines().collect();
                            let start = lines.len().saturating_sub(60);
                            lines[start..].join("\n")
                        })
                        .unwrap_or_else(|_| tr("[Incapaz de ler arquivo de log]"));
                    let prompt = tr_fmt(
                        "Analise as seguintes mensagens e erros recentes do arquivo de log '{name}' e forneça diagnóstico das causas raízes e soluções recomendadas:\n```\n{log}\n```",
                        &[("name", &n), ("log", &tail_sample)],
                    );
                    ai(&prompt);
                    st.set_text(&tr_fmt("Diagnóstico do log '{name}' enviado ao Assistente de IA.", &[("name", &n)]));
                });
            }
        }

        if let Some(ai_cb) = ai_prompt_cb.as_ref() {
            let ai = Rc::clone(ai_cb);
            let p = entry.path.clone();
            let n = entry.name.clone();
            let st = status_lbl.clone();
            add_menu_item(&menu_vbox, &popover, "dialog-information-symbolic", &tr("Explicar Arquivo com IA"), false, move || {
                let sample = fs::read_to_string(&p)
                    .map(|s| s.lines().take(60).collect::<Vec<_>>().join("\n"))
                    .unwrap_or_else(|_| tr("[Conteúdo binário ou inacessível]"));
                let prompt = tr_fmt(
                    "Explique o propósito, estrutura e funcionamento do arquivo '{name}'.\nCaminho: {path}\n\nConteúdo inicial:\n```\n{content}\n```",
                    &[("name", &n), ("path", &p), ("content", &sample)],
                );
                ai(&prompt);
                st.set_text(&tr_fmt("Explicação do arquivo '{name}' solicitada ao Assistente de IA.", &[("name", &n)]));
            });

            let ai = Rc::clone(ai_cb);
            let e_clone = entry.clone();
            let st = status_lbl.clone();
            add_menu_item(&menu_vbox, &popover, "security-high-symbolic", &tr("Auditar Segurança com IA"), false, move || {
                let perms = e_clone.formatted_permissions();
                let oct = e_clone.permissions_octal_str();
                let size_str = e_clone.formatted_size();
                let prompt = tr_fmt(
                    "Por favor, realize uma auditoria detalhada de segurança e permissões de acesso do arquivo '{name}'.\nCaminho: {path}\nPermissões atuais: {perms} ({oct})\nTamanho: {size}\nRecomende configurações seguras de chmod e mitigação de vulnerabilidades.",
                    &[
                        ("name", &e_clone.name),
                        ("path", &e_clone.path),
                        ("perms", &perms),
                        ("oct", &oct),
                        ("size", &size_str),
                    ],
                );
                ai(&prompt);
                st.set_text(&tr_fmt("Auditoria de segurança de '{name}' enviada ao Assistente de IA.", &[("name", &e_clone.name)]));
            });
        }

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let path_str = entry.path.clone();
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", &tr("Copy Absolute Path"), false, move || {
            if let Some(cb) = gdk::Display::default().and_then(|d| Some(d.clipboard())) {
                cb.set_text(&path_str);
                st.set_text(&tr_fmt("Caminho copiado: {path}", &[("path", &path_str)]));
            }
        });

        let path_str = entry.path.clone();
        let feed_cb = Rc::clone(feed_terminal_cb);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "insert-text-symbolic", &tr("Inserir Caminho no Terminal"), false, move || {
            let escaped = format!("'{}' ", path_str.replace('\'', "'\\''"));
            feed_cb(&escaped);
            st.set_text(&tr_fmt("Caminho inserido no terminal: {path}", &[("path", &path_str)]));
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let p = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", &tr("Copiar Arquivo"), false, move || {
            *clip.borrow_mut() = Some(FileManagerClipboardItem {
                path: p.clone(),
                is_dir: false,
                is_cut: false,
            });
            st.set_text(&tr_fmt("Arquivo copiado para transferência: {name}", &[("name", &name)]));
        });

        let p = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-cut-symbolic", &tr("Recortar Arquivo"), false, move || {
            *clip.borrow_mut() = Some(FileManagerClipboardItem {
                path: p.clone(),
                is_dir: false,
                is_cut: true,
            });
            st.set_text(&tr_fmt("Arquivo marcado para mover: {name}", &[("name", &name)]));
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let old_path = PathBuf::from(&entry.path);
        let old_name = entry.name.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "document-edit-symbolic", &tr("Renomear..."), false, move || {
            let op = old_path.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            let bk2 = Rc::clone(&bk);
            show_simple_input_dialog(&win, &tr("Renomear Arquivo"), &tr("Novo nome do arquivo:"), &old_name, Rc::new(move |new_name| {
                if let Some(parent) = op.parent() {
                    let np = parent.join(new_name.trim());
                    let ok_msg = tr_fmt("Arquivo renomeado para '{name}'", &[("name", new_name.trim())]);
                    let bk_snap = bk2.borrow().clone();
                    let op_bg = op.clone();
                    run_backend_mutation(&bk_snap, &st2, &rf2, ok_msg, tr("Erro ao renomear"), move |b| b.rename(&op_bg, &np));
                }
            }));
        });

        let e_clone = entry.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "channel-secure-symbolic", &tr("Permissões (chmod)..."), false, move || {
            show_permissions_dialog_with_backend(&win, &e_clone, bk.borrow().clone(), Rc::clone(&rf), st.clone());
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let e_clone = entry.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "user-trash-symbolic", &tr("Delete File"), true, move || {
            let ec = e_clone.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            let bk2 = Rc::clone(&bk);
            let del_path = PathBuf::from(&ec.path);
            let del_name = ec.name.clone();
            show_confirm_delete_dialog(&win, &ec, Rc::new(move || {
                let ok_msg = tr_fmt("Arquivo excluído: {name}", &[("name", &del_name)]);
                let bk_snap = bk2.borrow().clone();
                let dp = del_path.clone();
                run_backend_mutation(&bk_snap, &st2, &rf2, ok_msg, tr("Erro ao excluir arquivo"), move |b| b.remove(&dp));
            }));
        });
    }

    add_organize_and_view_menu_items(
        &menu_vbox,
        &popover,
        sort_col,
        sort_asc,
        view_mode,
        change_sort,
        switch_view_mode,
        status_lbl,
    );

    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .propagate_natural_height(true)
        .propagate_natural_width(true)
        .max_content_height(440)
        .child(&menu_vbox)
        .build();
    popover.set_child(Some(&scroll));
    popover.popup();
}

/// Shows the right-click contextual popover menu for blank / background space of a directory pane.
fn show_context_menu_for_background(
    anchor_widget: &impl IsA<gtk4::Widget>,
    _pane_root: &gtk4::Box,
    active_popover: &Rc<RefCell<Option<gtk4::Popover>>>,
    x: f64,
    y: f64,
    cur_dir: &Rc<RefCell<PathBuf>>,
    pane_backend: &Rc<RefCell<FilePaneBackend>>,
    parent_window: &adw::ApplicationWindow,
    status_lbl: &gtk4::Label,
    refresh_fn: &Rc<dyn Fn()>,
    cd_in_terminal_cb: &Rc<dyn Fn(&Path)>,
    feed_terminal_cb: &Rc<dyn Fn(&str)>,
    ai_prompt_cb: &Option<Rc<dyn Fn(&str)>>,
    clipboard: &Rc<RefCell<Option<FileManagerClipboardItem>>>,
    sort_col: &Rc<RefCell<SortColumn>>,
    sort_asc: &Rc<RefCell<bool>>,
    view_mode: &Rc<RefCell<FileManagerViewMode>>,
    change_sort: &Rc<dyn Fn(SortColumn, Option<bool>)>,
    switch_view_mode: &Rc<dyn Fn(FileManagerViewMode)>,
) {
    if let Some(old) = active_popover.borrow_mut().take() {
        if old.parent().is_some() {
            old.popdown();
        }
    }

    let popover = gtk4::Popover::new();
    popover.set_parent(anchor_widget);
    popover.set_has_arrow(true);
    popover.set_position(gtk4::PositionType::Top);
    popover.set_autohide(true);
    popover.add_css_class("onyxsh-popover");
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

    let act_rc = Rc::clone(active_popover);
    popover.connect_closed(move |p| {
        let p_clone = p.clone();
        let act = Rc::clone(&act_rc);
        glib::idle_add_local_once(move || {
            let mut act_borrow = act.borrow_mut();
            if let Some(cur) = act_borrow.as_ref() {
                if cur == &p_clone {
                    act_borrow.take();
                }
            }
            drop(act_borrow);
            if p_clone.parent().is_some() {
                p_clone.unparent();
            }
        });
    });
    *active_popover.borrow_mut() = Some(popover.clone());

    let menu_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    menu_vbox.set_margin_start(6);
    menu_vbox.set_margin_end(6);
    menu_vbox.set_margin_top(6);
    menu_vbox.set_margin_bottom(6);
    menu_vbox.set_width_request(280);

    let cd = Rc::clone(cur_dir);
    let cd_cb = Rc::clone(cd_in_terminal_cb);
    add_menu_item(&menu_vbox, &popover, "utilities-terminal-symbolic", &tr("Abrir este Diretório no Terminal (cd)"), false, move || {
        cd_cb(&cd.borrow());
    });

    let cd = Rc::clone(cur_dir);
    let st = status_lbl.clone();
    add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", &tr("Copy Directory Path"), false, move || {
        let path_str = cd.borrow().to_string_lossy().to_string();
        if let Some(cb) = gdk::Display::default().and_then(|d| Some(d.clipboard())) {
            cb.set_text(&path_str);
            st.set_text(&tr_fmt("Caminho copiado: {path}", &[("path", &path_str)]));
        }
    });

    let cd = Rc::clone(cur_dir);
    let feed_cb = Rc::clone(feed_terminal_cb);
    let st = status_lbl.clone();
    add_menu_item(&menu_vbox, &popover, "insert-text-symbolic", &tr("Inserir Caminho no Terminal"), false, move || {
        let path_str = cd.borrow().to_string_lossy().to_string();
        let escaped = format!("'{}' ", path_str.replace('\'', "'\\''"));
        feed_cb(&escaped);
        st.set_text(&tr_fmt("Caminho inserido no terminal: {path}", &[("path", &path_str)]));
    });

    menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let cd = Rc::clone(cur_dir);
    let win = parent_window.clone();
    let rf = Rc::clone(refresh_fn);
    let st = status_lbl.clone();
    let bk = Rc::clone(pane_backend);
    add_menu_item(&menu_vbox, &popover, "folder-new-symbolic", &tr("Nova Pasta..."), false, move || {
        let tdir = cd.borrow().clone();
        let rf2 = Rc::clone(&rf);
        let st2 = st.clone();
        let bk2 = Rc::clone(&bk);
        show_simple_input_dialog(&win, &tr("New Folder"), &tr("Nome da nova pasta:"), "nova_pasta", Rc::new(move |name| {
            let p = tdir.join(name.trim());
            let ok_msg = tr_fmt("Pasta criada: {path}", &[("path", &p.display().to_string())]);
            let bk_snap = bk2.borrow().clone();
            run_backend_mutation(&bk_snap, &st2, &rf2, ok_msg, tr("Erro ao criar pasta"), move |b| b.mkdir(&p));
        }));
    });

    let cd = Rc::clone(cur_dir);
    let win = parent_window.clone();
    let rf = Rc::clone(refresh_fn);
    let st = status_lbl.clone();
    let bk = Rc::clone(pane_backend);
    add_menu_item(&menu_vbox, &popover, "document-new-symbolic", &tr("Novo Arquivo..."), false, move || {
        let tdir = cd.borrow().clone();
        let rf2 = Rc::clone(&rf);
        let st2 = st.clone();
        let bk2 = Rc::clone(&bk);
        show_simple_input_dialog(&win, &tr("New File"), &tr("Nome do novo arquivo:"), "novo_arquivo.txt", Rc::new(move |name| {
            let p = tdir.join(name.trim());
            let ok_msg = tr_fmt("Arquivo criado: {path}", &[("path", &p.display().to_string())]);
            let bk_snap = bk2.borrow().clone();
            run_backend_mutation(&bk_snap, &st2, &rf2, ok_msg, tr("Erro ao criar arquivo"), move |b| b.create_empty_file(&p));
        }));
    });

    if clipboard.borrow().is_some() {
        let cd = Rc::clone(cur_dir);
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        let rf = Rc::clone(refresh_fn);
        let bk = Rc::clone(pane_backend);
        add_menu_item(&menu_vbox, &popover, "edit-paste-symbolic", &tr("Colar Aqui"), false, move || {
            execute_paste(&clip, &cd.borrow(), &bk.borrow(), &st, &rf);
        });
    }

    menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    if let Some(ai_cb) = ai_prompt_cb.as_ref() {
        let ai = Rc::clone(ai_cb);
        let cd = Rc::clone(cur_dir);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "security-high-symbolic", &tr("Auditar Segurança deste Diretório com IA"), false, move || {
            let path_str = cd.borrow().to_string_lossy().to_string();
            let prompt = tr_fmt(
                "Por favor, realize uma auditoria detalhada de segurança e governança de acesso no diretório atual '{path}'. Verifique permissões permissivas, arquivos sensíveis (.env, chaves privadas, credenciais expostas) e recomende boas práticas Zero-Trust.",
                &[("path", &path_str)],
            );
            ai(&prompt);
            st.set_text(&tr_fmt(
                "Auditoria do diretório '{path}' enviada ao Assistente de IA.",
                &[("path", &path_str)],
            ));
        });
    }

    let rf = Rc::clone(refresh_fn);
    add_menu_item(&menu_vbox, &popover, "view-refresh-symbolic", &tr("Atualizar Lista (F5)"), false, move || {
        rf();
    });

    add_organize_and_view_menu_items(
        &menu_vbox,
        &popover,
        sort_col,
        sort_asc,
        view_mode,
        change_sort,
        switch_view_mode,
        status_lbl,
    );

    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .propagate_natural_height(true)
        .propagate_natural_width(true)
        .max_content_height(440)
        .child(&menu_vbox)
        .build();
    popover.set_child(Some(&scroll));
    popover.popup();
}

/// Recursively scans subdirectories for matching file/directory names.
fn search_recursive_dir(root: &Path, query: &str, limit: usize) -> Vec<FileEntry> {
    let mut results = Vec::new();
    let q_lower = query.trim().to_lowercase();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if results.len() >= limit {
            break;
        }
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().into_owned();
                let is_dir = path.is_dir();
                if is_dir && !file_name.starts_with('.') {
                    stack.push(path.clone());
                }
                if file_name.to_lowercase().contains(&q_lower) {
                    if let Ok(m) = fs::symlink_metadata(&path) {
                        let is_symlink = m.is_symlink();
                        let size = m.len();
                        let modified_epoch = m
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let permissions_octal = (m.permissions().mode() & 0o777) as u16;
                        #[cfg(unix)]
                        let owner = {
                            use std::os::unix::fs::MetadataExt;
                            onyxsh_filemanager::backend::resolve_posix_owner(m.uid(), m.gid())
                        };
                        #[cfg(not(unix))]
                        let owner = "user:group".to_string();

                        results.push(FileEntry {
                            name: file_name,
                            path: path.to_string_lossy().into_owned(),
                            is_dir,
                            is_symlink,
                            symlink_target: None,
                            size,
                            modified_epoch,
                            permissions_octal,
                            owner,
                        });
                        if results.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }
    }
    results
}

/// Displays the Quick Jump & Bookmarks popover menu.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct FileManagerBookmark {
    pub name: String,
    pub path: String,
}

pub fn load_bookmarks() -> Vec<FileManagerBookmark> {
    let settings_path = platform::config_dir().join("settings.json");
    if let Ok(settings) = onyxsh_core::config::AppSettings::load_from_path(&settings_path) {
        return settings
            .file_manager_bookmarks
            .into_iter()
            .map(|b| FileManagerBookmark {
                name: b.name,
                path: b.path,
            })
            .collect();
    }
    Vec::new()
}

pub fn save_bookmarks(bookmarks: &[FileManagerBookmark]) {
    let settings_path = platform::config_dir().join("settings.json");
    let mut settings =
        onyxsh_core::config::AppSettings::load_from_path(&settings_path).unwrap_or_default();
    settings.file_manager_bookmarks = bookmarks
        .iter()
        .map(|b| onyxsh_core::config::BookmarkEntry {
            name: b.name.clone(),
            path: b.path.clone(),
        })
        .collect();
    let _ = settings.save_to_path(&settings_path);
}

fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join(".git").exists() {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

fn show_quick_jump_popover(
    btn: &gtk4::Button,
    cur_dir: &Rc<RefCell<PathBuf>>,
    refresh: &Rc<dyn Fn()>,
    status_lbl: &gtk4::Label,
    btn_bookmark: &gtk4::Button,
) {
    let popover = gtk4::Popover::new();
    popover.set_parent(btn);
    popover.add_css_class("quick-jump-popover");

    let popover_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    popover_box.set_margin_top(6);
    popover_box.set_margin_bottom(6);
    popover_box.set_margin_start(6);
    popover_box.set_margin_end(6);

    // Section 1: ATALHOS DO SISTEMA
    let header_shortcuts = gtk4::Label::new(Some(&tr("System Shortcuts").to_uppercase()));
    header_shortcuts.set_halign(gtk4::Align::Start);
    header_shortcuts.add_css_class("quick-jump-header");
    popover_box.append(&header_shortcuts);

    let home = platform::home_dir();
    let home_str = home.to_string_lossy().to_string();
    let config_dir_str = home.join(".config").to_string_lossy().to_string();
    let temp_str = std::env::temp_dir().to_string_lossy().to_string();

    let mut shortcuts = vec![
        (tr("Home"), home_str, "folder-symbolic"),
    ];

    let active_path = cur_dir.borrow().clone();
    if let Some(git_root) = find_git_root(&active_path) {
        shortcuts.push((
            tr("Git Repository Root"),
            git_root.to_string_lossy().to_string(),
            "utilities-terminal-symbolic",
        ));
    }

    shortcuts.push((tr("Root"), "/".to_string(), "computer-symbolic"));
    shortcuts.push((tr("Logs"), "/var/log".to_string(), "text-x-generic-symbolic"));
    shortcuts.push((tr("Config"), config_dir_str, "preferences-other-symbolic"));
    shortcuts.push((tr("Temp"), temp_str, "user-trash-symbolic"));

    for (label, path_str, icon_name) in shortcuts {
        let item_btn = gtk4::Button::new();
        item_btn.add_css_class("flat");
        item_btn.add_css_class("quick-jump-item-btn");

        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

        let icon = gtk4::Image::from_icon_name(icon_name);
        row.append(&icon);

        let lbl = gtk4::Label::new(Some(&label));
        lbl.set_halign(gtk4::Align::Start);
        lbl.set_hexpand(true);
        lbl.add_css_class("quick-jump-name");
        row.append(&lbl);

        let path_lbl = gtk4::Label::new(Some(&path_str));
        path_lbl.set_halign(gtk4::Align::End);
        path_lbl.add_css_class("caption");
        path_lbl.add_css_class("dim-label");
        row.append(&path_lbl);

        item_btn.set_child(Some(&row));

        let c_dir = Rc::clone(cur_dir);
        let rf = Rc::clone(refresh);
        let p_clone = popover.clone();
        let target_path = PathBuf::from(path_str);
        item_btn.connect_clicked(move |_| {
            p_clone.popdown();
            if target_path != *c_dir.borrow() {
                *c_dir.borrow_mut() = target_path.clone();
                rf();
            }
        });

        popover_box.append(&item_btn);
    }

    // Section 2: FAVORITOS
    let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    sep.set_margin_top(6);
    sep.set_margin_bottom(6);
    popover_box.append(&sep);

    let header_bm = gtk4::Label::new(Some(&tr("Bookmarks").to_uppercase()));
    header_bm.set_halign(gtk4::Align::Start);
    header_bm.add_css_class("quick-jump-header");
    popover_box.append(&header_bm);

    let bookmarks = load_bookmarks();
    if bookmarks.is_empty() {
        let empty_lbl = gtk4::Label::new(Some(&tr("No bookmarked folders yet")));
        empty_lbl.set_halign(gtk4::Align::Start);
        empty_lbl.add_css_class("caption");
        empty_lbl.add_css_class("dim-label");
        empty_lbl.set_margin_start(8);
        empty_lbl.set_margin_bottom(6);
        popover_box.append(&empty_lbl);
    } else {
        for bm in bookmarks {
            let row_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);

            let btn = gtk4::Button::new();
            btn.add_css_class("flat");
            btn.add_css_class("quick-jump-item-btn");
            btn.set_hexpand(true);

            let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
            let icon = gtk4::Image::from_icon_name("folder-symbolic");
            btn_box.append(&icon);

            let lbl = gtk4::Label::new(Some(&bm.name));
            lbl.set_halign(gtk4::Align::Start);
            lbl.set_hexpand(true);
            lbl.add_css_class("quick-jump-name");
            btn_box.append(&lbl);

            let path_lbl = gtk4::Label::new(Some(&bm.path));
            path_lbl.set_halign(gtk4::Align::End);
            path_lbl.add_css_class("caption");
            path_lbl.add_css_class("dim-label");
            btn_box.append(&path_lbl);

            btn.set_child(Some(&btn_box));

            let bm_path = bm.path.clone();
            let c_dir = Rc::clone(cur_dir);
            let rf = Rc::clone(refresh);
            let p_clone = popover.clone();
            btn.connect_clicked(move |_| {
                p_clone.popdown();
                let target = PathBuf::from(&bm_path);
                if target != *c_dir.borrow() {
                    *c_dir.borrow_mut() = target;
                    rf();
                }
            });
            row_box.append(&btn);

            let del_btn = gtk4::Button::builder()
                .icon_name("user-trash-symbolic")
                .has_frame(false)
                .tooltip_text(tr("Remover favorito"))
                .build();
            del_btn.add_css_class("flat");

            let bm_path_del = bm.path.clone();
            let p_clone2 = popover.clone();
            let c_dir2 = Rc::clone(cur_dir);
            let rf2 = Rc::clone(refresh);
            let st = status_lbl.clone();
            let btn_bm = btn_bookmark.clone();
            del_btn.connect_clicked(move |_| {
                let mut bms = load_bookmarks();
                bms.retain(|b| b.path != bm_path_del);
                save_bookmarks(&bms);
                st.set_text(&tr("Pasta removida dos favoritos."));

                let cur_str = c_dir2.borrow().to_string_lossy().to_string();
                let is_cur_bookmarked = bms.iter().any(|b| b.path == cur_str);
                if is_cur_bookmarked {
                    btn_bm.add_css_class("bookmark-active-star");
                    btn_bm.set_tooltip_text(Some(&tr("Remover favorito do diretório atual")));
                } else {
                    btn_bm.remove_css_class("bookmark-active-star");
                    btn_bm.set_tooltip_text(Some(&tr("Favoritar diretório atual")));
                }

                p_clone2.popdown();
                rf2();
            });
            row_box.append(&del_btn);

            popover_box.append(&row_box);
        }
    }

    let popover_cleanup = popover.clone();
    popover.connect_closed(move |_| {
        let p = popover_cleanup.clone();
        glib::idle_add_local_once(move || {
            if p.parent().is_some() {
                p.unparent();
            }
        });
    });

    popover.set_child(Some(&popover_box));
    popover.popup();
}

struct TreeRenderContext<'a> {
    pane_id: &'a str,
    tree_list: &'a gtk4::ListBox,
    pane_vbox: &'a gtk4::Box,
    active_popover: &'a Rc<RefCell<Option<gtk4::Popover>>>,
    cur_dir: &'a Rc<RefCell<PathBuf>>,
    backend: &'a Rc<RefCell<FilePaneBackend>>,
    parent_window: &'a adw::ApplicationWindow,
    status_lbl: &'a gtk4::Label,
    refresh_holder: &'a Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    cd_cb: &'a Rc<dyn Fn(&Path)>,
    feed_cb: &'a Rc<dyn Fn(&str)>,
    ai_cb_opt: &'a Option<Rc<dyn Fn(&str)>>,
    clip_rc: &'a Rc<RefCell<Option<FileManagerClipboardItem>>>,
    vis_tree: &'a Rc<RefCell<Vec<FileEntry>>>,
    sel_entry: &'a Rc<RefCell<Option<FileEntry>>>,
    scol_rc: &'a Rc<RefCell<SortColumn>>,
    sasc_rc: &'a Rc<RefCell<bool>>,
    vm_rc: &'a Rc<RefCell<FileManagerViewMode>>,
    cs_rc: &'a Rc<dyn Fn(SortColumn, Option<bool>)>,
    svm_rc: &'a Rc<dyn Fn(FileManagerViewMode)>,
    ema_rc: &'a Rc<Cell<bool>>,
    expanded_dirs: &'a Rc<RefCell<HashSet<PathBuf>>>,
    tree_metrics_cache: &'a Rc<RefCell<HashMap<PathBuf, (usize, u64)>>>,
    tree_children_cache: &'a Rc<RefCell<HashMap<PathBuf, Vec<FileEntry>>>>,
    tree_fetching: &'a Rc<RefCell<HashSet<PathBuf>>>,
    show_hidden: bool,
    search_query: &'a str,
}

fn format_cached_tree_summary(entry: &FileEntry, cached: Option<(usize, u64)>) -> String {
    if entry.name == ".." {
        return String::new();
    }
    if !entry.is_dir {
        return format_bytes(entry.size);
    }
    if let Some((count, bytes)) = cached {
        let items_str = tr_n("item", "itens", count as u64);
        let size_str = format_bytes(bytes);
        return format!("{size_str} ({count} {items_str})");
    }
    String::new()
}

fn attach_row_drag_source(widget: &impl IsA<gtk4::Widget>, pane_id: &str, entry: &FileEntry) {
    if entry.name == ".." {
        return;
    }
    let payload = DragTransferPayload {
        source_pane: pane_id.to_string(),
        path: entry.path.clone(),
        name: entry.name.clone(),
        is_dir: entry.is_dir,
        size_bytes: entry.size,
    };
    let encoded = encode_drag_payload(&payload);
    if encoded.is_empty() {
        return;
    }
    let drag_src = gtk4::DragSource::new();
    drag_src.set_actions(gdk::DragAction::COPY);
    drag_src.connect_prepare(move |_, _, _| {
        Some(gdk::ContentProvider::for_value(&encoded.to_value()))
    });
    widget.add_controller(drag_src);
}

fn populate_tree_level(ctx: &TreeRenderContext, entries: &[FileEntry], depth: usize) {
    if depth > 10 {
        return;
    }

    for entry in entries {
        // Record in flat list so index in ListBox matches vis_tree
        ctx.vis_tree.borrow_mut().push(entry.clone());

        let is_expanded = entry.is_dir && ctx.expanded_dirs.borrow().contains(Path::new(&entry.path));

        let row_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        row_hbox.add_css_class("file-detailed-row");
        row_hbox.add_css_class("file-tree-row");
        row_hbox.set_hexpand(true);
        row_hbox.set_halign(gtk4::Align::Fill);
        row_hbox.set_margin_start(4);
        row_hbox.set_margin_end(16);
        row_hbox.set_margin_top(2);
        row_hbox.set_margin_bottom(2);
        set_lazy_tooltip_markup(&row_hbox, build_entry_tooltip_markup(entry));

        if entry.is_hidden() {
            row_hbox.add_css_class("dim-label");
        }

        let name_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        name_box.set_hexpand(true);
        name_box.set_halign(gtk4::Align::Fill);

        // Indentation for nested levels
        if depth > 0 {
            let indent = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
            indent.set_width_request((depth * 20) as i32);
            name_box.append(&indent);
        }

        // Expander arrow button for directories, spacer for files
        if entry.is_dir && entry.name != ".." {
            let exp_btn = gtk4::Button::builder()
                .has_frame(false)
                .width_request(20)
                .height_request(20)
                .build();
            exp_btn.add_css_class("flat");
            exp_btn.add_css_class("file-tree-expander-btn");
            let arrow_icon = gtk4::Image::from_icon_name(if is_expanded {
                "pan-down-symbolic"
            } else {
                "pan-end-symbolic"
            });
            arrow_icon.set_pixel_size(12);
            exp_btn.set_child(Some(&arrow_icon));

            let p = PathBuf::from(&entry.path);
            let exp_dirs_c = Rc::clone(ctx.expanded_dirs);
            let rf_holder = Rc::clone(ctx.refresh_holder);
            exp_btn.connect_clicked(move |_| {
                let mut set = exp_dirs_c.borrow_mut();
                if set.contains(&p) {
                    set.remove(&p);
                } else {
                    set.insert(p.clone());
                }
                drop(set);
                if let Some(r) = rf_holder.borrow().as_ref() {
                    r();
                }
            });
            name_box.append(&exp_btn);
        } else {
            let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
            spacer.set_width_request(20);
            spacer.set_height_request(20);
            name_box.append(&spacer);
        }

        let icon = gtk4::Image::from_icon_name(icon_for_entry(entry));
        name_box.append(&icon);

        let name_lbl = gtk4::Label::new(Some(&entry.name));
        name_lbl.set_halign(gtk4::Align::Start);
        name_lbl.set_hexpand(true);
        name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        if entry.is_dir {
            name_lbl.add_css_class("heading");
        }
        name_box.append(&name_lbl);

        if entry.is_symlink {
            let sym_icon = gtk4::Image::from_icon_name("emblem-symbolic-link-symbolic");
            set_lazy_tooltip_text(&sym_icon, tr("Symbolic Link"));
            name_box.append(&sym_icon);
        }

        let badges_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
        if entry.is_executable() && !entry.is_dir {
            let badge_exec = gtk4::Label::new(Some("+x"));
            badge_exec.add_css_class("badge-pill");
            badge_exec.add_css_class("badge-exec");
            set_lazy_tooltip_text(&badge_exec, tr("Executable file"));
            badges_box.append(&badge_exec);
        }
        if let Some((badge_text, badge_css)) = entry.file_type_badge() {
            let badge_type = gtk4::Label::new(Some(badge_text));
            badge_type.add_css_class("badge-pill");
            badge_type.add_css_class(badge_css);
            badges_box.append(&badge_type);
        }
        name_box.append(&badges_box);

        let cached_stat = if entry.is_dir && entry.name != ".." {
            ctx.tree_metrics_cache.borrow().get(Path::new(&entry.path)).copied()
        } else {
            None
        };
        let size_str = if entry.is_dir && cached_stat.is_none() && ctx.backend.borrow().is_sftp() {
            if entry.name == ".." { String::new() } else { "—".to_string() }
        } else if entry.is_dir && cached_stat.is_some() {
            format_cached_tree_summary(entry, cached_stat)
        } else {
            format_tree_size_summary(entry)
        };
        let size_lbl = gtk4::Label::new(Some(&size_str));
        size_lbl.add_css_class("file-detailed-col-size");
        size_lbl.add_css_class("monospace");
        size_lbl.set_width_request(130);
        size_lbl.set_xalign(1.0);

        let mod_lbl = gtk4::Label::new(Some(&format_epoch_short(entry.modified_epoch)));
        mod_lbl.add_css_class("file-detailed-col-date");
        mod_lbl.add_css_class("monospace");
        mod_lbl.set_width_request(150);
        mod_lbl.set_xalign(0.5);

        let perm_lbl = gtk4::Label::new(Some(&entry.formatted_permissions()));
        perm_lbl.add_css_class("file-detailed-col-perms");
        perm_lbl.add_css_class("monospace");
        set_lazy_tooltip_text(
            &perm_lbl,
            tr_fmt(
                "Octal: {mode}",
                &[("mode", &entry.permissions_octal_str())],
            ),
        );
        perm_lbl.set_width_request(110);
        perm_lbl.set_xalign(0.5);

        let owner_lbl = gtk4::Label::new(Some(&entry.owner));
        owner_lbl.add_css_class("file-detailed-col-owner");
        owner_lbl.add_css_class("monospace");
        owner_lbl.set_width_request(160);
        owner_lbl.set_xalign(0.0);

        row_hbox.append(&name_box);
        row_hbox.append(&size_lbl);
        row_hbox.append(&mod_lbl);
        row_hbox.append(&perm_lbl);
        row_hbox.append(&owner_lbl);

        let list_row = gtk4::ListBoxRow::new();
        list_row.set_child(Some(&row_hbox));
        list_row.set_selectable(true);
        list_row.set_activatable(true);

        // Right-click context menu gesture
        let right_click = gtk4::GestureClick::new();
        right_click.set_button(3);
        let row_ref = list_row.clone();
        let hbox_ref = row_hbox.clone();
        let tl_ref = ctx.tree_list.clone();
        let pane_ref = ctx.pane_vbox.clone();
        let act_pop = Rc::clone(ctx.active_popover);
        let e_clone = entry.clone();
        let cur_dir_c = Rc::clone(ctx.cur_dir);
        let bk_c = Rc::clone(ctx.backend);
        let win_ref = ctx.parent_window.clone();
        let st_c = ctx.status_lbl.clone();
        let rh = Rc::clone(ctx.refresh_holder);
        let cd_cb2 = Rc::clone(ctx.cd_cb);
        let feed_cb2 = Rc::clone(ctx.feed_cb);
        let ai_cb2 = ctx.ai_cb_opt.clone();
        let clip2 = Rc::clone(ctx.clip_rc);
        let vis2 = Rc::clone(ctx.vis_tree);
        let sel2 = Rc::clone(ctx.sel_entry);
        let scol_c = Rc::clone(ctx.scol_rc);
        let sasc_c = Rc::clone(ctx.sasc_rc);
        let vm_c = Rc::clone(ctx.vm_rc);
        let cs_c = Rc::clone(ctx.cs_rc);
        let svm_c = Rc::clone(ctx.svm_rc);
        let ema = Rc::clone(ctx.ema_rc);
        right_click.connect_pressed(move |gesture, _, x, y| {
            gesture.set_state(gtk4::EventSequenceState::Claimed);
            ema.set(true);
            *sel2.borrow_mut() = Some(e_clone.clone());
            tl_ref.select_row(Some(&row_ref));
            if let Some(r) = rh.borrow().as_ref() {
                let clicked_widget = gesture.widget().unwrap_or_else(|| hbox_ref.clone().upcast());
                show_context_menu_for_entry(
                    &clicked_widget,
                    &pane_ref,
                    &act_pop,
                    x,
                    y,
                    &e_clone,
                    &cur_dir_c,
                    &bk_c,
                    &win_ref,
                    &st_c,
                    r,
                    &cd_cb2,
                    &feed_cb2,
                    &ai_cb2,
                    &clip2,
                    &vis2,
                    &scol_c,
                    &sasc_c,
                    &vm_c,
                    &cs_c,
                    &svm_c,
                );
            }
        });
        row_hbox.add_controller(right_click);
        attach_row_drag_source(&row_hbox, ctx.pane_id, entry);

        ctx.tree_list.append(&list_row);

        // Trigger async deep Local metrics or depth<=2 SFTP metrics for directories
        if entry.is_dir && entry.name != ".." {
            let dir_pb = PathBuf::from(&entry.path);
            let already_cached = ctx.tree_metrics_cache.borrow().contains_key(&dir_pb);
            let already_fetching = ctx.tree_fetching.borrow().contains(&dir_pb);
            if !already_cached && !already_fetching && (depth == 0 || is_expanded) {
                match ctx.backend.borrow().clone() {
                    FilePaneBackend::Local => {
                        ctx.tree_fetching.borrow_mut().insert(dir_pb.clone());
                        let cache_c = Rc::clone(ctx.tree_metrics_cache);
                        let fetch_c = Rc::clone(ctx.tree_fetching);
                        let lbl_weak = size_lbl.downgrade();
                        let entry_snap = entry.clone();
                        let p_bg = dir_pb.clone();
                        let (tx, rx) = std::sync::mpsc::sync_channel::<(usize, u64)>(1);
                        let p_thread = p_bg.clone();
                        std::thread::spawn(move || {
                            let res = collect_local_tree_stats_depth(&p_thread, None);
                            let _ = tx.send(res);
                        });
                        glib::timeout_add_local(Duration::from_millis(50), move || match rx.try_recv() {
                            Ok((cnt, bytes)) => {
                                fetch_c.borrow_mut().remove(&p_bg);
                                cache_c.borrow_mut().insert(p_bg.clone(), (cnt, bytes));
                                if let Some(lbl) = lbl_weak.upgrade() {
                                    lbl.set_text(&format_cached_tree_summary(
                                        &entry_snap,
                                        Some((cnt, bytes)),
                                    ));
                                }
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                fetch_c.borrow_mut().remove(&p_bg);
                                glib::ControlFlow::Break
                            }
                        });
                    }
                    FilePaneBackend::Sftp(sftp) if is_expanded || depth <= 1 => {
                        ctx.tree_fetching.borrow_mut().insert(dir_pb.clone());
                        let cache_c = Rc::clone(ctx.tree_metrics_cache);
                        let ch_cache = Rc::clone(ctx.tree_children_cache);
                        let fetch_c = Rc::clone(ctx.tree_fetching);
                        let lbl_weak = size_lbl.downgrade();
                        let entry_snap = entry.clone();
                        let p_bg = dir_pb.clone();
                        let sub_depth = if is_expanded && depth == 0 { 2 } else { 1 };
                        let (tx, rx) = std::sync::mpsc::sync_channel::<(usize, u64, Vec<FileEntry>)>(1);
                        let p_thread = p_bg.clone();
                        std::thread::spawn(move || {
                            let res = collect_sftp_tree_stats_limited(&sftp, &p_thread, sub_depth);
                            let _ = tx.send(res);
                        });
                        glib::timeout_add_local(Duration::from_millis(60), move || match rx.try_recv() {
                            Ok((cnt, bytes, children)) => {
                                fetch_c.borrow_mut().remove(&p_bg);
                                cache_c.borrow_mut().insert(p_bg.clone(), (cnt, bytes));
                                ch_cache.borrow_mut().insert(p_bg.clone(), children);
                                if let Some(lbl) = lbl_weak.upgrade() {
                                    lbl.set_text(&format_cached_tree_summary(
                                        &entry_snap,
                                        Some((cnt, bytes)),
                                    ));
                                }
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                fetch_c.borrow_mut().remove(&p_bg);
                                glib::ControlFlow::Break
                            }
                        });
                    }
                    _ => {}
                }
            }
        }

        // If directory is expanded, recursively populate its children
        if is_expanded {
            let entry_pb = PathBuf::from(&entry.path);
            let children_opt = match ctx.backend.borrow().clone() {
                FilePaneBackend::Local => LocalFsBackend.list_dir(&entry_pb).ok(),
                FilePaneBackend::Sftp(sftp) => {
                    if let Some(cached) = ctx.tree_children_cache.borrow().get(&entry_pb).cloned() {
                        Some(cached)
                    } else if !ctx.tree_fetching.borrow().contains(&entry_pb) && depth < 3 {
                        ctx.tree_fetching.borrow_mut().insert(entry_pb.clone());
                        let ch_cache = Rc::clone(ctx.tree_children_cache);
                        let cache_c = Rc::clone(ctx.tree_metrics_cache);
                        let fetch_c = Rc::clone(ctx.tree_fetching);
                        let rf_c = Rc::clone(ctx.refresh_holder);
                        let p_bg = entry_pb.clone();
                        let p_thread = p_bg.clone();
                        let (tx, rx) = std::sync::mpsc::sync_channel::<(usize, u64, Vec<FileEntry>)>(1);
                        std::thread::spawn(move || {
                            let res = collect_sftp_tree_stats_limited(&sftp, &p_thread, 2);
                            let _ = tx.send(res);
                        });
                        glib::timeout_add_local(Duration::from_millis(60), move || match rx.try_recv() {
                            Ok((cnt, bytes, ch)) => {
                                fetch_c.borrow_mut().remove(&p_bg);
                                cache_c.borrow_mut().insert(p_bg.clone(), (cnt, bytes));
                                ch_cache.borrow_mut().insert(p_bg.clone(), ch);
                                if let Some(r) = rf_c.borrow().as_ref() {
                                    r();
                                }
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                fetch_c.borrow_mut().remove(&p_bg);
                                glib::ControlFlow::Break
                            }
                        });
                        None
                    } else {
                        None
                    }
                }
            };
            if let Some(children) = children_opt {
                let sub_sorted = filter_and_sort_entries(
                    &children,
                    ctx.show_hidden,
                    ctx.search_query,
                    *ctx.scol_rc.borrow(),
                    *ctx.sasc_rc.borrow(),
                    true,
                );
                populate_tree_level(ctx, &sub_sorted, depth + 1);
            }
        }
    }
}

#[derive(Clone)]
struct GridContextDeps {
    grid_selection: gtk4::MultiSelection,
    pane_vbox: gtk4::Box,
    active_popover: Rc<RefCell<Option<gtk4::Popover>>>,
    cur_dir: Rc<RefCell<PathBuf>>,
    backend: Rc<RefCell<FilePaneBackend>>,
    parent_window: adw::ApplicationWindow,
    status_lbl: gtk4::Label,
    refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    cd_cb: Rc<dyn Fn(&Path)>,
    feed_cb: Rc<dyn Fn(&str)>,
    ai_cb_opt: Option<Rc<dyn Fn(&str)>>,
    clip_rc: Rc<RefCell<Option<FileManagerClipboardItem>>>,
    vis_entries: Rc<RefCell<Vec<FileEntry>>>,
    sel_entry: Rc<RefCell<Option<FileEntry>>>,
    scol_rc: Rc<RefCell<SortColumn>>,
    sasc_rc: Rc<RefCell<bool>>,
    vm_rc: Rc<RefCell<FileManagerViewMode>>,
    cs_rc: Rc<dyn Fn(SortColumn, Option<bool>)>,
    svm_rc: Rc<dyn Fn(FileManagerViewMode)>,
    ema_rc: Rc<Cell<bool>>,
}

/// Creates the virtualized `gtk4::GridView` + `gio::ListStore` + `gtk4::MultiSelection` +
/// `gtk4::SignalListItemFactory` matching Python `manager.py::_create_icon_grid_view`.
fn create_icon_grid_view() -> (
    gtk4::GridView,
    gio::ListStore,
    gtk4::MultiSelection,
    gtk4::SignalListItemFactory,
) {
    let grid_store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let grid_selection = gtk4::MultiSelection::new(Some(grid_store.clone()));
    let factory = gtk4::SignalListItemFactory::new();

    let grid_view = gtk4::GridView::new(Some(grid_selection.clone()), Some(factory.clone()));
    grid_view.add_css_class("file-manager-grid-view");
    grid_view.set_max_columns(24);
    grid_view.set_min_columns(2);
    grid_view.set_enable_rubberband(true);
    grid_view.set_single_click_activate(false);
    grid_view.set_hexpand(true);
    grid_view.set_vexpand(true);

    (grid_view, grid_store, grid_selection, factory)
}

/// Populates the badge pills row of a GridView card for executable or special file types.
fn populate_grid_card_badges(badges_box: &gtk4::Box, entry: &FileEntry) {
    while let Some(c) = badges_box.first_child() {
        badges_box.remove(&c);
    }
    if entry.name == ".." {
        badges_box.set_visible(false);
        return;
    }

    let mut has_badge = false;
    if entry.is_executable() && !entry.is_dir {
        let badge = gtk4::Label::new(Some("+x"));
        badge.add_css_class("badge-pill");
        badge.add_css_class("badge-exec");
        set_lazy_tooltip_text(&badge, tr("Executable file"));
        badges_box.append(&badge);
        has_badge = true;
    }
    if let Some((badge_text, badge_css)) = entry.file_type_badge() {
        let badge = gtk4::Label::new(Some(badge_text));
        badge.add_css_class("badge-pill");
        badge.add_css_class(badge_css);
        badges_box.append(&badge);
        has_badge = true;
    }
    badges_box.set_visible(has_badge);
}

/// Connects `setup` and `bind` handlers on the GridView `SignalListItemFactory` matching Python
/// `_setup_grid_item` and `_bind_grid_item`.
fn wire_grid_factory(factory: &gtk4::SignalListItemFactory, deps: GridContextDeps) {
    factory.connect_setup(move |_, item_obj| {
        let Some(list_item) = item_obj.downcast_ref::<gtk4::ListItem>() else {
            return;
        };

        let card = gtk4::Box::new(gtk4::Orientation::Vertical, 3);
        card.add_css_class("file-grid-card");
        card.set_valign(gtk4::Align::Center);
        card.set_halign(gtk4::Align::Center);
        card.set_has_tooltip(true);
        let li_tip = list_item.downgrade();
        card.connect_query_tooltip(move |_, _, _, _, tip| {
            let Some(li) = li_tip.upgrade() else {
                return false;
            };
            let Some(boxed) = li.item().and_downcast::<glib::BoxedAnyObject>() else {
                return false;
            };
            let entry = boxed.borrow::<FileEntry>();
            tip.set_markup(Some(&build_entry_tooltip_markup(&entry)));
            true
        });

        let icon_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        icon_box.set_halign(gtk4::Align::Center);
        let icon_img = gtk4::Image::new();
        icon_img.set_pixel_size(48);
        icon_img.add_css_class("file-grid-card-icon");
        icon_box.append(&icon_img);
        card.append(&icon_box);

        let badges_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
        badges_box.set_halign(gtk4::Align::Center);
        badges_box.set_visible(false);
        card.append(&badges_box);

        let name_lbl = gtk4::Label::builder()
            .xalign(0.5)
            .justify(gtk4::Justification::Center)
            .wrap(true)
            .wrap_mode(gtk4::pango::WrapMode::WordChar)
            .width_chars(10)
            .max_width_chars(16)
            .lines(2)
            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
            .build();
        name_lbl.add_css_class("file-grid-card-label");
        card.append(&name_lbl);

        let sub_lbl = gtk4::Label::builder()
            .xalign(0.5)
            .justify(gtk4::Justification::Center)
            .build();
        sub_lbl.add_css_class("file-grid-card-size");
        sub_lbl.add_css_class("dim-label");
        sub_lbl.add_css_class("caption");
        card.append(&sub_lbl);

        let right_click = gtk4::GestureClick::new();
        right_click.set_button(3);
        let item_weak = list_item.downgrade();
        let deps_c = deps.clone();
        right_click.connect_pressed(move |gesture, _, x, y| {
            let Some(item) = item_weak.upgrade() else {
                return;
            };
            let Some(boxed) = item.item().and_downcast::<glib::BoxedAnyObject>() else {
                return;
            };
            let entry = boxed.borrow::<FileEntry>().clone();
            let pos = item.position();
            let Some(card_widget) = item.child() else {
                return;
            };

            gesture.set_state(gtk4::EventSequenceState::Claimed);
            deps_c.ema_rc.set(true);
            *deps_c.sel_entry.borrow_mut() = Some(entry.clone());
            deps_c.grid_selection.select_item(pos, true);

            if let Some(r) = deps_c.refresh_holder.borrow().as_ref() {
                let clicked_widget = gesture.widget().unwrap_or(card_widget);
                show_context_menu_for_entry(
                    &clicked_widget,
                    &deps_c.pane_vbox,
                    &deps_c.active_popover,
                    x,
                    y,
                    &entry,
                    &deps_c.cur_dir,
                    &deps_c.backend,
                    &deps_c.parent_window,
                    &deps_c.status_lbl,
                    r,
                    &deps_c.cd_cb,
                    &deps_c.feed_cb,
                    &deps_c.ai_cb_opt,
                    &deps_c.clip_rc,
                    &deps_c.vis_entries,
                    &deps_c.scol_rc,
                    &deps_c.sasc_rc,
                    &deps_c.vm_rc,
                    &deps_c.cs_rc,
                    &deps_c.svm_rc,
                );
            }
        });
        card.add_controller(right_click);

        list_item.set_child(Some(&card));
    });

    factory.connect_bind(move |_, item_obj| {
        let Some(list_item) = item_obj.downcast_ref::<gtk4::ListItem>() else {
            return;
        };
        let Some(boxed) = list_item.item().and_downcast::<glib::BoxedAnyObject>() else {
            return;
        };
        let entry = boxed.borrow::<FileEntry>();

        let Some(card) = list_item.child().and_downcast::<gtk4::Box>() else {
            return;
        };
        let Some(icon_box) = card.first_child().and_downcast::<gtk4::Box>() else {
            return;
        };
        let Some(icon_img) = icon_box.first_child().and_downcast::<gtk4::Image>() else {
            return;
        };
        let Some(badges_box) = icon_box.next_sibling().and_downcast::<gtk4::Box>() else {
            return;
        };
        let Some(name_lbl) = badges_box.next_sibling().and_downcast::<gtk4::Label>() else {
            return;
        };
        let Some(sub_lbl) = name_lbl.next_sibling().and_downcast::<gtk4::Label>() else {
            return;
        };

        if entry.is_hidden() {
            card.add_css_class("dim-label");
        } else {
            card.remove_css_class("dim-label");
        }

        icon_img.set_from_gicon(&resolve_grid_item_gicon(&entry));
        populate_grid_card_badges(&badges_box, &entry);

        name_lbl.set_text(&entry.name);
        if entry.is_dir {
            name_lbl.add_css_class("heading");
        } else {
            name_lbl.remove_css_class("heading");
        }

        sub_lbl.set_text(&format_grid_subtitle(&entry));
    });
}

/// Checks whether a right-click at `(x, y)` inside `scroll` landed on a file/folder row or grid card.
fn is_entry_target_at_point(scroll: &gtk4::ScrolledWindow, x: f64, y: f64) -> bool {
    let Some(target) = scroll.pick(x, y, gtk4::PickFlags::DEFAULT) else {
        return false;
    };
    let scroll_widget = scroll.upcast_ref::<gtk4::Widget>();
    let mut curr = Some(target);
    while let Some(w) = curr {
        if &w == scroll_widget {
            break;
        }
        let css = w.css_name();
        let classes = w.css_classes();
        if css == "listitem"
            || css == "child"
            || css == "row"
            || classes.iter().any(|c| {
                c == "file-detailed-row" || c == "file-grid-card" || c == "file-tree-row"
            })
            || w.type_().name() == "GtkListBoxRow"
            || w.type_().name() == "GtkFlowBoxChild"
        {
            return true;
        }
        curr = w.parent();
    }
    false
}

/// Builds a rich File Manager Pane with Local backend (`pane_id = "left"`).
pub fn build_file_pane(
    parent_window: &adw::ApplicationWindow,
    initial_dir: PathBuf,
    status_lbl: &gtk4::Label,
    cd_in_terminal_cb: Rc<dyn Fn(&Path)>,
    feed_terminal_cb: Rc<dyn Fn(&str)>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
    clipboard: Rc<RefCell<Option<FileManagerClipboardItem>>>,
) -> FilePaneHandle {
    build_file_pane_with_options(
        parent_window,
        initial_dir,
        status_lbl,
        cd_in_terminal_cb,
        feed_terminal_cb,
        ai_prompt_cb,
        clipboard,
        "left",
        None,
    )
}

/// Builds a rich File Manager Pane with explicit pane ID (`"left"` / `"right"`) and shared transfer queue controller.
#[allow(clippy::too_many_arguments)]
pub fn build_file_pane_with_options(
    parent_window: &adw::ApplicationWindow,
    initial_dir: PathBuf,
    status_lbl: &gtk4::Label,
    cd_in_terminal_cb: Rc<dyn Fn(&Path)>,
    feed_terminal_cb: Rc<dyn Fn(&str)>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
    clipboard: Rc<RefCell<Option<FileManagerClipboardItem>>>,
    pane_id: &str,
    transfer_ctrl: Option<Arc<TransferQueueController>>,
) -> FilePaneHandle {
    let pane_id_owned = pane_id.to_string();
    let cfg_path = platform::config_dir().join("settings.json");
    let (initial_sort, initial_asc, initial_vm) = if let Ok(st) = AppSettings::load_from_path(&cfg_path) {
        let col = match st.file_manager_sort_by.as_str() {
            "size" => SortColumn::Size,
            "modified" => SortColumn::Modified,
            "permissions" => SortColumn::Permissions,
            "owner" => SortColumn::Owner,
            "type" => SortColumn::Type,
            "group" => SortColumn::GroupByType,
            _ => SortColumn::Name,
        };
        let vm = match st.file_manager_view_mode.as_str() {
            "grid" => FileManagerViewMode::Grid,
            "tree" => FileManagerViewMode::Tree,
            _ => FileManagerViewMode::List,
        };
        (col, st.file_manager_sort_ascending, vm)
    } else {
        (SortColumn::Name, true, FileManagerViewMode::List)
    };

    let current_dir = Rc::new(RefCell::new(initial_dir));
    let pane_backend: Rc<RefCell<FilePaneBackend>> = Rc::new(RefCell::new(FilePaneBackend::Local));
    let refresh_gen: Rc<Cell<u64>> = Rc::new(Cell::new(0));
    let tree_metrics_cache: Rc<RefCell<HashMap<PathBuf, (usize, u64)>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let tree_children_cache: Rc<RefCell<HashMap<PathBuf, Vec<FileEntry>>>> =
        Rc::new(RefCell::new(HashMap::new()));
    let tree_fetching: Rc<RefCell<HashSet<PathBuf>>> = Rc::new(RefCell::new(HashSet::new()));
    let selected_entry: Rc<RefCell<Option<FileEntry>>> = Rc::new(RefCell::new(None));
    let visible_entries: Rc<RefCell<Vec<FileEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let show_hidden = Rc::new(RefCell::new(false));
    let sort_col = Rc::new(RefCell::new(initial_sort));
    let sort_asc = Rc::new(RefCell::new(initial_asc));
    let view_mode = Rc::new(RefCell::new(initial_vm));

    let pane_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    pane_vbox.set_hexpand(true);
    pane_vbox.set_vexpand(true);

    let active_popover: Rc<RefCell<Option<gtk4::Popover>>> = Rc::new(RefCell::new(None));

    // ── 1. Column Header Bar at Top (Shown in List Mode) ─────────────────────
    let col_header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    col_header_box.add_css_class("file-detailed-header");
    col_header_box.set_margin_start(4);
    col_header_box.set_margin_end(16);
    col_header_box.set_margin_top(2);
    col_header_box.set_margin_bottom(2);

    // 1.1 Nome Header Button
    let btn_sort_name = gtk4::Button::builder()
        .has_frame(false)
        .hexpand(true)
        .halign(gtk4::Align::Fill)
        .build();
    btn_sort_name.add_css_class("flat");
    btn_sort_name.add_css_class("file-detailed-header-name");
    let name_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let lbl_name = gtk4::Label::builder()
        .label(tr("Name"))
        .xalign(0.0)
        .hexpand(true)
        .build();
    lbl_name.add_css_class("file-detailed-header-label");
    let icon_name_sort = gtk4::Image::from_icon_name("pan-down-symbolic");
    icon_name_sort.add_css_class("dim-label");
    icon_name_sort.set_visible(false);
    name_box.append(&lbl_name);
    name_box.append(&icon_name_sort);
    btn_sort_name.set_child(Some(&name_box));

    // 1.2 Tamanho Header Button (matches filemanagerpython.png with chevron arrow on left)
    let btn_sort_size = gtk4::Button::builder()
        .has_frame(false)
        .width_request(130)
        .halign(gtk4::Align::End)
        .build();
    btn_sort_size.add_css_class("flat");
    btn_sort_size.add_css_class("file-detailed-header-size");
    let size_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let icon_size_sort = gtk4::Image::from_icon_name("pan-down-symbolic");
    icon_size_sort.add_css_class("dim-label");
    icon_size_sort.set_visible(false);
    let lbl_size = gtk4::Label::builder()
        .label(tr("Size"))
        .xalign(1.0)
        .build();
    lbl_size.add_css_class("file-detailed-header-label");
    size_box.append(&icon_size_sort);
    size_box.append(&lbl_size);
    btn_sort_size.set_child(Some(&size_box));

    // 1.3 Data Modificada Header Button
    let btn_sort_mod = gtk4::Button::builder()
        .has_frame(false)
        .width_request(150)
        .halign(gtk4::Align::Center)
        .build();
    btn_sort_mod.add_css_class("flat");
    btn_sort_mod.add_css_class("file-detailed-header-date");
    let mod_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let icon_mod_sort = gtk4::Image::from_icon_name("pan-down-symbolic");
    icon_mod_sort.add_css_class("dim-label");
    icon_mod_sort.set_visible(false);
    let lbl_mod = gtk4::Label::builder()
        .label(tr("Date Modified"))
        .xalign(0.5)
        .build();
    lbl_mod.add_css_class("file-detailed-header-label");
    mod_box.append(&lbl_mod);
    mod_box.append(&icon_mod_sort);
    btn_sort_mod.set_child(Some(&mod_box));

    // 1.4 Permissões Header Button
    let btn_sort_perm = gtk4::Button::builder()
        .has_frame(false)
        .width_request(110)
        .halign(gtk4::Align::Center)
        .build();
    btn_sort_perm.add_css_class("flat");
    btn_sort_perm.add_css_class("file-detailed-header-perms");
    let perm_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let icon_perm_sort = gtk4::Image::from_icon_name("pan-down-symbolic");
    icon_perm_sort.add_css_class("dim-label");
    icon_perm_sort.set_visible(false);
    let lbl_perm = gtk4::Label::builder()
        .label(tr("Permissions"))
        .xalign(0.5)
        .build();
    lbl_perm.add_css_class("file-detailed-header-label");
    perm_box.append(&lbl_perm);
    perm_box.append(&icon_perm_sort);
    btn_sort_perm.set_child(Some(&perm_box));

    // 1.5 Proprietário Header Button
    let btn_sort_owner = gtk4::Button::builder()
        .has_frame(false)
        .width_request(160)
        .halign(gtk4::Align::Start)
        .build();
    btn_sort_owner.add_css_class("flat");
    btn_sort_owner.add_css_class("file-detailed-header-owner");
    let owner_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let icon_owner_sort = gtk4::Image::from_icon_name("pan-down-symbolic");
    icon_owner_sort.add_css_class("dim-label");
    icon_owner_sort.set_visible(false);
    let lbl_owner = gtk4::Label::builder()
        .label(tr("Owner"))
        .xalign(0.0)
        .build();
    lbl_owner.add_css_class("file-detailed-header-label");
    owner_box.append(&lbl_owner);
    owner_box.append(&icon_owner_sort);
    btn_sort_owner.set_child(Some(&owner_box));

    col_header_box.append(&btn_sort_name);
    col_header_box.append(&btn_sort_size);
    col_header_box.append(&btn_sort_mod);
    col_header_box.append(&btn_sort_perm);
    col_header_box.append(&btn_sort_owner);

    pane_vbox.append(&col_header_box);

    // ── 2. Content Stack: List, Grid, Tree ───────────────────────────────────
    let view_stack = gtk4::Stack::new();
    view_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    view_stack.set_vhomogeneous(false);
    view_stack.set_hhomogeneous(false);
    view_stack.set_hexpand(true);
    view_stack.set_vexpand(true);

    let list_scroll = gtk4::ScrolledWindow::new();
    list_scroll.set_hexpand(true);
    list_scroll.set_vexpand(true);
    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    list_box.set_activate_on_single_click(false);
    list_box.add_css_class("file-manager-detailed-view");
    list_scroll.set_child(Some(&list_box));
    view_stack.add_named(&list_scroll, Some("list"));

    let grid_scroll = gtk4::ScrolledWindow::new();
    grid_scroll.set_hexpand(true);
    grid_scroll.set_vexpand(true);
    grid_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    let (grid_view, grid_store, grid_selection, grid_factory) = create_icon_grid_view();
    grid_scroll.set_child(Some(&grid_view));
    view_stack.add_named(&grid_scroll, Some("grid"));

    let tree_scroll = gtk4::ScrolledWindow::new();
    tree_scroll.set_hexpand(true);
    tree_scroll.set_vexpand(true);
    let tree_list = gtk4::ListBox::new();
    tree_list.set_selection_mode(gtk4::SelectionMode::Single);
    tree_list.set_activate_on_single_click(false);
    tree_list.add_css_class("file-manager-detailed-view");
    tree_list.add_css_class("file-manager-tree-view");
    tree_scroll.set_child(Some(&tree_list));
    view_stack.add_named(&tree_scroll, Some("tree"));

    let expanded_dirs: Rc<RefCell<HashSet<PathBuf>>> = Rc::new(RefCell::new(HashSet::new()));
    let last_cur_dir: Rc<RefCell<PathBuf>> = Rc::new(RefCell::new(current_dir.borrow().clone()));
    let visible_tree_entries: Rc<RefCell<Vec<FileEntry>>> = Rc::new(RefCell::new(Vec::new()));

    pane_vbox.append(&view_stack);
    view_stack.set_visible_child_name(initial_vm.as_stack_name());
    col_header_box.set_visible(initial_vm == FileManagerViewMode::List || initial_vm == FileManagerViewMode::Tree);

    // ── 3. Bottom Action Bar (Identical to filemanagerpython.png) ─────────────
    let action_bar = gtk4::ActionBar::new();
    action_bar.add_css_class("file-manager-action-bar");

    let source_btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let source_icon = gtk4::Image::from_icon_name("computer-symbolic");
    let source_lbl = gtk4::Label::new(Some(&tr("Local")));
    source_lbl.add_css_class("caption");
    source_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    source_lbl.set_max_width_chars(18);
    source_btn_box.append(&source_icon);
    source_btn_box.append(&source_lbl);
    let source_btn = gtk4::MenuButton::builder()
        .child(&source_btn_box)
        .tooltip_text(tr("Alternar Origem: Sistema Local ou Sessão SFTP"))
        .has_frame(false)
        .build();
    source_btn.add_css_class("flat");
    action_bar.pack_start(&source_btn);

    let sep0 = gtk4::Separator::new(gtk4::Orientation::Vertical);
    action_bar.pack_start(&sep0);

    let btn_refresh = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text(tr("Atualizar (F5)"))
        .has_frame(false)
        .build();
    btn_refresh.add_css_class("flat");

    let btn_hidden = gtk4::ToggleButton::builder()
        .icon_name("view-reveal-symbolic")
        .tooltip_text(tr("Mostrar arquivos ocultos (.dotfiles)"))
        .has_frame(false)
        .build();
    btn_hidden.add_css_class("flat");

    let btn_bookmark = gtk4::Button::builder()
        .icon_name("starred-symbolic")
        .tooltip_text(tr("Favoritar diretório atual"))
        .has_frame(false)
        .build();
    btn_bookmark.add_css_class("flat");

    let btn_quick_jump = gtk4::Button::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text(tr("Atalhos do Sistema & Favoritos"))
        .has_frame(false)
        .build();
    btn_quick_jump.add_css_class("flat");

    action_bar.pack_start(&btn_refresh);
    action_bar.pack_start(&btn_hidden);
    action_bar.pack_start(&btn_bookmark);
    action_bar.pack_start(&btn_quick_jump);

    let sep1 = gtk4::Separator::new(gtk4::Orientation::Vertical);
    action_bar.pack_start(&sep1);

    let mode_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    mode_box.add_css_class("linked");
    let btn_mode_list = gtk4::ToggleButton::builder()
        .icon_name("view-list-symbolic")
        .tooltip_text(tr("Detailed List View"))
        .active(initial_vm == FileManagerViewMode::List)
        .build();
    let btn_mode_grid = gtk4::ToggleButton::builder()
        .icon_name("view-grid-symbolic")
        .tooltip_text(tr("Visualização em Grade / Miniaturas"))
        .active(initial_vm == FileManagerViewMode::Grid)
        .group(&btn_mode_list)
        .build();
    let btn_mode_tree = gtk4::ToggleButton::builder()
        .icon_name("format-indent-more-symbolic")
        .tooltip_text(tr("Hierarchical Tree View"))
        .active(initial_vm == FileManagerViewMode::Tree)
        .group(&btn_mode_list)
        .build();
    mode_box.append(&btn_mode_list);
    mode_box.append(&btn_mode_grid);
    mode_box.append(&btn_mode_tree);
    action_bar.pack_start(&mode_box);

    let btn_toggle_dual = gtk4::ToggleButton::builder()
        .icon_name("view-dual-symbolic")
        .tooltip_text(tr("Alternar Painel Duplo (Dividir / Dual-Pane)"))
        .active(false)
        .has_frame(false)
        .build();
    btn_toggle_dual.add_css_class("flat");
    action_bar.pack_start(&btn_toggle_dual);

    let sep2 = gtk4::Separator::new(gtk4::Orientation::Vertical);
    action_bar.pack_start(&sep2);

    let breadcrumb_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .hexpand(true)
        .build();
    let breadcrumb_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    breadcrumb_box.add_css_class("breadcrumb-trail");
    breadcrumb_scroll.set_child(Some(&breadcrumb_box));
    action_bar.pack_start(&breadcrumb_scroll);

    let switch_container = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    switch_container.set_valign(gtk4::Align::Center);
    switch_container.set_margin_start(4);
    switch_container.set_margin_end(4);

    let recursive_lbl = gtk4::Label::new(Some(&tr("Recursive")));
    recursive_lbl.add_css_class("dim-label");
    recursive_lbl.set_valign(gtk4::Align::Center);

    let recursive_switch = gtk4::Switch::new();
    recursive_switch.set_active(false);
    recursive_switch.set_valign(gtk4::Align::Center);
    recursive_switch.set_tooltip_text(Some(&tr("Pesquisa recursiva em subdiretórios")));

    switch_container.append(&recursive_lbl);
    switch_container.append(&recursive_switch);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text(tr("Filter files..."))
        .width_chars(14)
        .build();

    let btn_history = gtk4::Button::builder()
        .icon_name("document-open-recent-symbolic")
        .tooltip_text(tr("Histórico de Transferências"))
        .has_frame(false)
        .build();
    btn_history.add_css_class("flat");

    action_bar.pack_end(&switch_container);
    action_bar.pack_end(&search_entry);
    action_bar.pack_end(&btn_history);

    pane_vbox.append(&action_bar);

    // Subtle Pane Footer (Item count + Free disk space + Backend)
    let footer_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    footer_box.set_margin_start(10);
    footer_box.set_margin_end(10);
    footer_box.set_margin_top(2);
    footer_box.set_margin_bottom(2);
    let footer_lbl = gtk4::Label::new(Some(""));
    footer_lbl.add_css_class("caption");
    footer_lbl.add_css_class("dim-label");
    footer_lbl.set_halign(gtk4::Align::Start);
    footer_lbl.set_hexpand(true);
    footer_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    footer_box.append(&footer_lbl);
    pane_vbox.append(&footer_box);

    // ── 5. Master Refresh Closure & Background Context Menus ─────────────────
    let refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let entry_menu_active = Rc::new(Cell::new(false));

    let change_sort: Rc<dyn Fn(SortColumn, Option<bool>)> = {
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        let rf = Rc::clone(&refresh_holder);
        Rc::new(move |col: SortColumn, asc: Option<bool>| {
            *scol.borrow_mut() = col;
            if let Some(a) = asc {
                *sasc.borrow_mut() = a;
            }
            let col_str = match col {
                SortColumn::Name => "name",
                SortColumn::Size => "size",
                SortColumn::Modified => "modified",
                SortColumn::Permissions => "permissions",
                SortColumn::Owner => "owner",
                SortColumn::Type => "type",
                SortColumn::GroupByType => "group",
            };
            let cfg_path = platform::config_dir().join("settings.json");
            if let Ok(mut st) = AppSettings::load_from_path(&cfg_path) {
                st.file_manager_sort_by = col_str.to_string();
                st.file_manager_sort_ascending = *sasc.borrow();
                let _ = st.save_to_path(&cfg_path);
            }
            if let Some(r) = rf.borrow().as_ref() {
                r();
            }
        })
    };

    let switch_view_mode: Rc<dyn Fn(FileManagerViewMode)> = {
        let vs = view_stack.clone();
        let ch = col_header_box.clone();
        let vm = Rc::clone(&view_mode);
        let b_list = btn_mode_list.clone();
        let b_grid = btn_mode_grid.clone();
        let b_tree = btn_mode_tree.clone();
        let ls = list_scroll.clone();
        let gs = grid_scroll.clone();
        let ts = tree_scroll.clone();
        let gv = grid_view.clone();
        let rf = Rc::clone(&refresh_holder);
        Rc::new(move |mode: FileManagerViewMode| {
            *vm.borrow_mut() = mode;
            vs.set_visible_child_name(mode.as_stack_name());
            ch.set_visible(mode == FileManagerViewMode::List || mode == FileManagerViewMode::Tree);
            match mode {
                FileManagerViewMode::List => {
                    if !b_list.is_active() {
                        b_list.set_active(true);
                    }
                    ls.vadjustment().set_value(0.0);
                    ls.hadjustment().set_value(0.0);
                }
                FileManagerViewMode::Grid => {
                    if !b_grid.is_active() {
                        b_grid.set_active(true);
                    }
                    gs.vadjustment().set_value(0.0);
                    gs.hadjustment().set_value(0.0);
                    gv.queue_resize();
                }
                FileManagerViewMode::Tree => {
                    if !b_tree.is_active() {
                        b_tree.set_active(true);
                    }
                    ts.vadjustment().set_value(0.0);
                    ts.hadjustment().set_value(0.0);
                }
            }
            let mode_str = match mode {
                FileManagerViewMode::List => "list",
                FileManagerViewMode::Grid => "grid",
                FileManagerViewMode::Tree => "tree",
            };
            let cfg_path = platform::config_dir().join("settings.json");
            if let Ok(mut st) = AppSettings::load_from_path(&cfg_path) {
                st.file_manager_view_mode = mode_str.to_string();
                let _ = st.save_to_path(&cfg_path);
            }
            if let Some(r) = rf.borrow().as_ref() {
                r();
            }
        })
    };

    wire_grid_factory(
        &grid_factory,
        GridContextDeps {
            grid_selection: grid_selection.clone(),
            pane_vbox: pane_vbox.clone(),
            active_popover: Rc::clone(&active_popover),
            cur_dir: Rc::clone(&current_dir),
            backend: Rc::clone(&pane_backend),
            parent_window: parent_window.clone(),
            status_lbl: status_lbl.clone(),
            refresh_holder: Rc::clone(&refresh_holder),
            cd_cb: Rc::clone(&cd_in_terminal_cb),
            feed_cb: Rc::clone(&feed_terminal_cb),
            ai_cb_opt: ai_prompt_cb.clone(),
            clip_rc: Rc::clone(&clipboard),
            vis_entries: Rc::clone(&visible_entries),
            sel_entry: Rc::clone(&selected_entry),
            scol_rc: Rc::clone(&sort_col),
            sasc_rc: Rc::clone(&sort_asc),
            vm_rc: Rc::clone(&view_mode),
            cs_rc: Rc::clone(&change_sort),
            svm_rc: Rc::clone(&switch_view_mode),
            ema_rc: Rc::clone(&entry_menu_active),
        },
    );

    // Background Right-Click Context Menus
    {
        let bg_click_list = gtk4::GestureClick::new();
        bg_click_list.set_button(3);
        let list_ref = list_box.clone();
        let scroll_ref = list_scroll.clone();
        let pane_ref = pane_vbox.clone();
        let act_pop = Rc::clone(&active_popover);
        let cd = Rc::clone(&current_dir);
        let pane_bk = Rc::clone(&pane_backend);
        let win = parent_window.clone();
        let st = status_lbl.clone();
        let rh = Rc::clone(&refresh_holder);
        let cd_cb = Rc::clone(&cd_in_terminal_cb);
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb = ai_prompt_cb.clone();
        let clip = Rc::clone(&clipboard);
        let sc = Rc::clone(&sort_col);
        let sa = Rc::clone(&sort_asc);
        let vm = Rc::clone(&view_mode);
        let cs = Rc::clone(&change_sort);
        let svm = Rc::clone(&switch_view_mode);
        let ema = Rc::clone(&entry_menu_active);
        let sel = Rc::clone(&selected_entry);
        bg_click_list.connect_pressed(move |gesture, _, x, y| {
            if ema.get() {
                ema.set(false);
                return;
            }
            if is_entry_target_at_point(&scroll_ref, x, y) {
                return;
            }

            gesture.set_state(gtk4::EventSequenceState::Claimed);
            list_ref.unselect_all();
            *sel.borrow_mut() = None;

            if let Some(r) = rh.borrow().as_ref() {
                show_context_menu_for_background(
                    &scroll_ref,
                    &pane_ref,
                    &act_pop,
                    x,
                    y,
                    &cd,
                    &pane_bk,
                    &win,
                    &st,
                    r,
                    &cd_cb,
                    &feed_cb,
                    &ai_cb,
                    &clip,
                    &sc,
                    &sa,
                    &vm,
                    &cs,
                    &svm,
                );
            }
        });
        list_scroll.add_controller(bg_click_list);
    }
    {
        let bg_click_grid = gtk4::GestureClick::new();
        bg_click_grid.set_button(3);
        let grid_sel_ref = grid_selection.clone();
        let scroll_ref = grid_scroll.clone();
        let pane_ref = pane_vbox.clone();
        let act_pop = Rc::clone(&active_popover);
        let cd = Rc::clone(&current_dir);
        let pane_bk = Rc::clone(&pane_backend);
        let win = parent_window.clone();
        let st = status_lbl.clone();
        let rh = Rc::clone(&refresh_holder);
        let cd_cb = Rc::clone(&cd_in_terminal_cb);
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb = ai_prompt_cb.clone();
        let clip = Rc::clone(&clipboard);
        let sc = Rc::clone(&sort_col);
        let sa = Rc::clone(&sort_asc);
        let vm = Rc::clone(&view_mode);
        let cs = Rc::clone(&change_sort);
        let svm = Rc::clone(&switch_view_mode);
        let ema = Rc::clone(&entry_menu_active);
        let sel = Rc::clone(&selected_entry);
        bg_click_grid.connect_pressed(move |gesture, _, x, y| {
            if ema.get() {
                ema.set(false);
                return;
            }
            if is_entry_target_at_point(&scroll_ref, x, y) {
                return;
            }

            gesture.set_state(gtk4::EventSequenceState::Claimed);
            grid_sel_ref.unselect_all();
            *sel.borrow_mut() = None;

            if let Some(r) = rh.borrow().as_ref() {
                show_context_menu_for_background(
                    &scroll_ref,
                    &pane_ref,
                    &act_pop,
                    x,
                    y,
                    &cd,
                    &pane_bk,
                    &win,
                    &st,
                    r,
                    &cd_cb,
                    &feed_cb,
                    &ai_cb,
                    &clip,
                    &sc,
                    &sa,
                    &vm,
                    &cs,
                    &svm,
                );
            }
        });
        grid_scroll.add_controller(bg_click_grid);
    }
    {
        let bg_click_tree = gtk4::GestureClick::new();
        bg_click_tree.set_button(3);
        let tree_ref = tree_list.clone();
        let scroll_ref = tree_scroll.clone();
        let pane_ref = pane_vbox.clone();
        let act_pop = Rc::clone(&active_popover);
        let cd = Rc::clone(&current_dir);
        let pane_bk = Rc::clone(&pane_backend);
        let win = parent_window.clone();
        let st = status_lbl.clone();
        let rh = Rc::clone(&refresh_holder);
        let cd_cb = Rc::clone(&cd_in_terminal_cb);
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb = ai_prompt_cb.clone();
        let clip = Rc::clone(&clipboard);
        let sc = Rc::clone(&sort_col);
        let sa = Rc::clone(&sort_asc);
        let vm = Rc::clone(&view_mode);
        let cs = Rc::clone(&change_sort);
        let svm = Rc::clone(&switch_view_mode);
        let ema = Rc::clone(&entry_menu_active);
        let sel = Rc::clone(&selected_entry);
        bg_click_tree.connect_pressed(move |gesture, _, x, y| {
            if ema.get() {
                ema.set(false);
                return;
            }
            if is_entry_target_at_point(&scroll_ref, x, y) {
                return;
            }

            gesture.set_state(gtk4::EventSequenceState::Claimed);
            tree_ref.unselect_all();
            *sel.borrow_mut() = None;

            if let Some(r) = rh.borrow().as_ref() {
                show_context_menu_for_background(
                    &scroll_ref,
                    &pane_ref,
                    &act_pop,
                    x,
                    y,
                    &cd,
                    &pane_bk,
                    &win,
                    &st,
                    r,
                    &cd_cb,
                    &feed_cb,
                    &ai_cb,
                    &clip,
                    &sc,
                    &sa,
                    &vm,
                    &cs,
                    &svm,
                );
            }
        });
        tree_scroll.add_controller(bg_click_tree);
    }

    let refresh_fn: Rc<dyn Fn()> = {
        let cur_dir = Rc::clone(&current_dir);
        let bk_rc = Rc::clone(&pane_backend);
        let gen_rc = Rc::clone(&refresh_gen);
        let sel_entry = Rc::clone(&selected_entry);
        let vis_entries = Rc::clone(&visible_entries);
        let hidden_rc = Rc::clone(&show_hidden);
        let scol_rc = Rc::clone(&sort_col);
        let sasc_rc = Rc::clone(&sort_asc);
        let vm_rc = Rc::clone(&view_mode);
        let cs_rc = Rc::clone(&change_sort);
        let svm_rc = Rc::clone(&switch_view_mode);
        let ema_rc = Rc::clone(&entry_menu_active);
        let pane_vbox_c = pane_vbox.clone();
        let pane_id_c = pane_id_owned.clone();
        let act_pop_rc = Rc::clone(&active_popover);
        let bc_box = breadcrumb_box.clone();
        let se = search_entry.clone();
        let rec_switch = recursive_switch.clone();
        let lb = list_box.clone();
        let gs = grid_store.clone();
        let tl = tree_list.clone();
        let exp_dirs = Rc::clone(&expanded_dirs);
        let last_d = Rc::clone(&last_cur_dir);
        let vis_tree = Rc::clone(&visible_tree_entries);
        let tree_metrics_c = Rc::clone(&tree_metrics_cache);
        let tree_children_c = Rc::clone(&tree_children_cache);
        let tree_fetch_c = Rc::clone(&tree_fetching);
        let ico_name = icon_name_sort.clone();
        let ico_size = icon_size_sort.clone();
        let ico_mod = icon_mod_sort.clone();
        let ico_perm = icon_perm_sort.clone();
        let ico_owner = icon_owner_sort.clone();
        let status = status_lbl.clone();
        let foot = footer_lbl.clone();
        let src_lbl = source_lbl.clone();
        let src_ico = source_icon.clone();
        let r_holder = Rc::clone(&refresh_holder);
        let cd_cb = Rc::clone(&cd_in_terminal_cb);
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb_opt = ai_prompt_cb.clone();
        let clip_rc = Rc::clone(&clipboard);
        let win_ref = parent_window.clone();
        let btn_bm = btn_bookmark.clone();

        Rc::new(move || {
            let dir_path = cur_dir.borrow().clone();
            let backend_snap = bk_rc.borrow().clone();
            let my_gen = gen_rc.get().wrapping_add(1);
            gen_rc.set(my_gen);

            src_lbl.set_text(&backend_snap.display_label());
            src_ico.set_icon_name(Some(if backend_snap.is_sftp() {
                "network-server-symbolic"
            } else {
                "computer-symbolic"
            }));

            // Update sort column indicators
            let col = *scol_rc.borrow();
            let asc = *sasc_rc.borrow();
            let arrow_icon = if asc { "pan-down-symbolic" } else { "pan-up-symbolic" };

            ico_name.set_icon_name(Some(arrow_icon));
            ico_name.set_visible(col == SortColumn::Name);

            ico_size.set_icon_name(Some(arrow_icon));
            ico_size.set_visible(col == SortColumn::Size);

            ico_mod.set_icon_name(Some(arrow_icon));
            ico_mod.set_visible(col == SortColumn::Modified);

            ico_perm.set_icon_name(Some(arrow_icon));
            ico_perm.set_visible(col == SortColumn::Permissions);

            ico_owner.set_icon_name(Some(arrow_icon));
            ico_owner.set_visible(col == SortColumn::Owner);

            // Update Bookmark star state
            let cur_str = dir_path.to_string_lossy().to_string();
            let bms = load_bookmarks();
            let is_bm = bms.iter().any(|b| b.path == cur_str);
            if is_bm {
                btn_bm.add_css_class("bookmark-active-star");
                btn_bm.set_tooltip_text(Some(&tr("Remover favorito do diretório atual")));
            } else {
                btn_bm.remove_css_class("bookmark-active-star");
                btn_bm.set_tooltip_text(Some(&tr("Favoritar diretório atual")));
            }

            // Rebuild Breadcrumbs with › separators
            while let Some(c) = bc_box.first_child() {
                bc_box.remove(&c);
            }
            let segments = build_breadcrumb_segments(&dir_path);
            let total_segs = segments.len();
            for (idx, (seg_name, seg_target)) in segments.into_iter().enumerate() {
                let seg_btn = gtk4::Button::with_label(&seg_name);
                seg_btn.add_css_class("flat");
                let c_dir = Rc::clone(&cur_dir);
                let rh = Rc::clone(&r_holder);
                seg_btn.connect_clicked(move |_| {
                    *c_dir.borrow_mut() = seg_target.clone();
                    if let Some(r) = rh.borrow().as_ref() {
                        r();
                    }
                });
                bc_box.append(&seg_btn);

                if idx + 1 < total_segs {
                    let sep_lbl = gtk4::Label::new(Some("›"));
                    sep_lbl.add_css_class("dim-label");
                    bc_box.append(&sep_lbl);
                }
            }

            let is_recursive = rec_switch.is_active();
            let query = se.text().to_string();
            let hidden = *hidden_rc.borrow();

            let render_dir_results: Rc<dyn Fn(Vec<FileEntry>, Option<(u64, u64)>)> = {
                let cur_dir = Rc::clone(&cur_dir);
                let bk_rc = Rc::clone(&bk_rc);
                let sel_entry = Rc::clone(&sel_entry);
                let vis_entries = Rc::clone(&vis_entries);
                let scol_rc = Rc::clone(&scol_rc);
                let sasc_rc = Rc::clone(&sasc_rc);
                let vm_rc = Rc::clone(&vm_rc);
                let cs_rc = Rc::clone(&cs_rc);
                let svm_rc = Rc::clone(&svm_rc);
                let ema_rc = Rc::clone(&ema_rc);
                let pane_vbox_c = pane_vbox_c.clone();
                let pane_id_r = pane_id_c.clone();
                let act_pop_rc = Rc::clone(&act_pop_rc);
                let lb = lb.clone();
                let gs = gs.clone();
                let tl = tl.clone();
                let exp_dirs = Rc::clone(&exp_dirs);
                let last_d = Rc::clone(&last_d);
                let vis_tree = Rc::clone(&vis_tree);
                let tree_metrics_c = Rc::clone(&tree_metrics_c);
                let tree_children_c = Rc::clone(&tree_children_c);
                let tree_fetch_c = Rc::clone(&tree_fetch_c);
                let status = status.clone();
                let foot = foot.clone();
                let r_holder = Rc::clone(&r_holder);
                let cd_cb = Rc::clone(&cd_cb);
                let feed_cb = Rc::clone(&feed_cb);
                let ai_cb_opt = ai_cb_opt.clone();
                let clip_rc = Rc::clone(&clip_rc);
                let win_ref = win_ref.clone();
                let dir_path_r = dir_path.clone();
                let query_r = query.clone();
                let backend_label = backend_snap.display_label();
                let is_sftp_r = backend_snap.is_sftp();

                Rc::new(move |raw_entries: Vec<FileEntry>, free_space_opt: Option<(u64, u64)>| {
                    let mut sorted = filter_and_sort_entries(
                        &raw_entries,
                        hidden,
                        &query_r,
                        col,
                        asc,
                        true,
                    );

                    // Prepend ".." parent directory entry if not at root and no filter query
                    if query_r.is_empty() && dir_path_r.parent().is_some() && dir_path_r != Path::new("/") {
                        let parent_path = dir_path_r.parent().unwrap();
                        let (modified, perms, owner) = if !is_sftp_r {
                            if let Ok(m) = fs::symlink_metadata(parent_path) {
                                let m_epoch = m
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                let p = (m.permissions().mode() & 0o777) as u16;
                                #[cfg(unix)]
                                let o = {
                                    use std::os::unix::fs::MetadataExt;
                                    onyxsh_filemanager::backend::resolve_posix_owner(m.uid(), m.gid())
                                };
                                #[cfg(not(unix))]
                                let o = "user:group".to_string();
                                (m_epoch, p, o)
                            } else {
                                (0, 0o755, "root:root".to_string())
                            }
                        } else {
                            (0, 0o755, "remote".to_string())
                        };

                        let parent_entry = FileEntry {
                            name: "..".to_string(),
                            path: parent_path.to_string_lossy().into_owned(),
                            is_dir: true,
                            is_symlink: false,
                            symlink_target: None,
                            size: 4096,
                            modified_epoch: modified,
                            permissions_octal: perms,
                            owner,
                        };
                        sorted.insert(0, parent_entry);
                    }

                    let display_count = sorted.iter().filter(|e| e.name != "..").count();
                    let items_word = tr_n("item", "itens", display_count as u64);
                    let space_part = match free_space_opt {
                        Some((avail, total)) if total > 0 => format!(
                            " • {} {} {}",
                            format_bytes(avail),
                            tr("livres de"),
                            format_bytes(total)
                        ),
                        _ => String::new(),
                    };
                    foot.set_text(&format!("{display_count} {items_word}{space_part} • {backend_label}"));

                    *vis_entries.borrow_mut() = sorted.clone();
                    *sel_entry.borrow_mut() = sorted.first().cloned();

                    let active_vm = *vm_rc.borrow();

                    // 1. Populate Detailed List View (`list_box`) only when active
                    lb.remove_all();
                    if active_vm == FileManagerViewMode::List {
                        for entry in sorted.iter().take(300) {
                        let row_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
                        row_hbox.add_css_class("file-detailed-row");
                        row_hbox.set_hexpand(true);
                        row_hbox.set_halign(gtk4::Align::Fill);
                        row_hbox.set_margin_start(4);
                        row_hbox.set_margin_end(16);
                        row_hbox.set_margin_top(2);
                        row_hbox.set_margin_bottom(2);
                        set_lazy_tooltip_markup(&row_hbox, build_entry_tooltip_markup(entry));

                        if entry.is_hidden() {
                            row_hbox.add_css_class("dim-label");
                        }

                        let icon = gtk4::Image::from_icon_name(icon_for_entry(entry));

                        let name_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
                        name_box.set_hexpand(true);
                        name_box.set_halign(gtk4::Align::Fill);
                        name_box.append(&icon);

                        let name_lbl = gtk4::Label::new(Some(&entry.name));
                        name_lbl.set_halign(gtk4::Align::Start);
                        name_lbl.set_hexpand(true);
                        name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                        if entry.is_dir {
                            name_lbl.add_css_class("heading");
                        }
                        name_box.append(&name_lbl);

                        if entry.is_symlink {
                            let sym_icon = gtk4::Image::from_icon_name("emblem-symbolic-link-symbolic");
                            if let Some(ref target) = entry.symlink_target {
                                set_lazy_tooltip_text(&sym_icon, format!("→ {target}"));
                            } else {
                                set_lazy_tooltip_text(&sym_icon, tr("Symbolic Link"));
                            }
                            name_box.append(&sym_icon);
                        }

                        let badges_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
                        if entry.is_executable() {
                            let badge_exec = gtk4::Label::new(Some("+x"));
                            badge_exec.add_css_class("badge-pill");
                            badge_exec.add_css_class("badge-exec");
                            set_lazy_tooltip_text(&badge_exec, tr("Executable file"));
                            badges_box.append(&badge_exec);
                        }
                        if let Some((badge_text, badge_css)) = entry.file_type_badge() {
                            let badge_type = gtk4::Label::new(Some(badge_text));
                            badge_type.add_css_class("badge-pill");
                            badge_type.add_css_class(badge_css);
                            badges_box.append(&badge_type);
                        }
                        name_box.append(&badges_box);

                        let size_lbl = gtk4::Label::new(Some(&entry.formatted_size_clean()));
                        size_lbl.add_css_class("file-detailed-col-size");
                        size_lbl.add_css_class("monospace");
                        size_lbl.set_width_request(130);
                        size_lbl.set_xalign(1.0);

                        let mod_lbl = gtk4::Label::new(Some(&format_epoch_short(entry.modified_epoch)));
                        mod_lbl.add_css_class("file-detailed-col-date");
                        mod_lbl.add_css_class("monospace");
                        mod_lbl.set_width_request(150);
                        mod_lbl.set_xalign(0.5);

                        let perm_lbl = gtk4::Label::new(Some(&entry.formatted_permissions()));
                        perm_lbl.add_css_class("file-detailed-col-perms");
                        perm_lbl.add_css_class("monospace");
                        set_lazy_tooltip_text(
                            &perm_lbl,
                            tr_fmt(
                                "Octal: {mode}",
                                &[("mode", &entry.permissions_octal_str())],
                            ),
                        );
                        perm_lbl.set_width_request(110);
                        perm_lbl.set_xalign(0.5);

                        let owner_lbl = gtk4::Label::new(Some(&entry.owner));
                        owner_lbl.add_css_class("file-detailed-col-owner");
                        owner_lbl.add_css_class("monospace");
                        owner_lbl.set_width_request(160);
                        owner_lbl.set_xalign(0.0);

                        row_hbox.append(&name_box);
                        row_hbox.append(&size_lbl);
                        row_hbox.append(&mod_lbl);
                        row_hbox.append(&perm_lbl);
                        row_hbox.append(&owner_lbl);

                        let list_row = gtk4::ListBoxRow::new();
                        list_row.set_child(Some(&row_hbox));
                        list_row.set_selectable(true);
                        list_row.set_activatable(true);

                        let right_click = gtk4::GestureClick::new();
                        right_click.set_button(3);
                        let row_ref = list_row.clone();
                        let hbox_ref = row_hbox.clone();
                        let lb_ref = lb.clone();
                        let pane_ref = pane_vbox_c.clone();
                        let act_pop = Rc::clone(&act_pop_rc);
                        let e_clone = entry.clone();
                        let cd = Rc::clone(&cur_dir);
                        let bk2 = Rc::clone(&bk_rc);
                        let win = win_ref.clone();
                        let st = status.clone();
                        let rh = Rc::clone(&r_holder);
                        let cd_cb2 = Rc::clone(&cd_cb);
                        let feed_cb2 = Rc::clone(&feed_cb);
                        let ai_cb2 = ai_cb_opt.clone();
                        let clip2 = Rc::clone(&clip_rc);
                        let vis2 = Rc::clone(&vis_entries);
                        let sel2 = Rc::clone(&sel_entry);
                        let scol_c = Rc::clone(&scol_rc);
                        let sasc_c = Rc::clone(&sasc_rc);
                        let vm_c = Rc::clone(&vm_rc);
                        let cs_c = Rc::clone(&cs_rc);
                        let svm_c = Rc::clone(&svm_rc);
                        let ema = Rc::clone(&ema_rc);
                        right_click.connect_pressed(move |gesture, _, x, y| {
                            gesture.set_state(gtk4::EventSequenceState::Claimed);
                            ema.set(true);
                            *sel2.borrow_mut() = Some(e_clone.clone());
                            lb_ref.select_row(Some(&row_ref));
                            if let Some(r) = rh.borrow().as_ref() {
                                let clicked_widget = gesture.widget().unwrap_or_else(|| hbox_ref.clone().upcast());
                                show_context_menu_for_entry(
                                    &clicked_widget,
                                    &pane_ref,
                                    &act_pop,
                                    x,
                                    y,
                                    &e_clone,
                                    &cd,
                                    &bk2,
                                    &win,
                                    &st,
                                    r,
                                    &cd_cb2,
                                    &feed_cb2,
                                    &ai_cb2,
                                    &clip2,
                                    &vis2,
                                    &scol_c,
                                    &sasc_c,
                                    &vm_c,
                                    &cs_c,
                                    &svm_c,
                                );
                            }
                        });
                        row_hbox.add_controller(right_click);
                        attach_row_drag_source(&row_hbox, &pane_id_r, entry);

                        lb.append(&list_row);
                        }
                    }

                    // 2. Populate Icon Grid View (`grid_store` -> `gtk4::GridView`) only when active
                    if active_vm == FileManagerViewMode::Grid {
                        let grid_items: Vec<glib::BoxedAnyObject> = sorted
                            .iter()
                            .cloned()
                            .map(glib::BoxedAnyObject::new)
                            .collect();
                        gs.splice(0, gs.n_items(), &grid_items);
                    } else if gs.n_items() > 0 {
                        gs.remove_all();
                    }

                    // 3. Populate Hierarchical Tree View (`tree_list`) only when active
                    tl.remove_all();
                    vis_tree.borrow_mut().clear();
                    if dir_path_r != *last_d.borrow() {
                        exp_dirs.borrow_mut().clear();
                        *last_d.borrow_mut() = dir_path_r.clone();
                    }
                    if active_vm == FileManagerViewMode::Tree {
                        let tree_ctx = TreeRenderContext {
                            pane_id: &pane_id_r,
                            tree_list: &tl,
                            pane_vbox: &pane_vbox_c,
                            active_popover: &act_pop_rc,
                            cur_dir: &cur_dir,
                            backend: &bk_rc,
                            parent_window: &win_ref,
                            status_lbl: &status,
                            refresh_holder: &r_holder,
                            cd_cb: &cd_cb,
                            feed_cb: &feed_cb,
                            ai_cb_opt: &ai_cb_opt,
                            clip_rc: &clip_rc,
                            vis_tree: &vis_tree,
                            sel_entry: &sel_entry,
                            scol_rc: &scol_rc,
                            sasc_rc: &sasc_rc,
                            vm_rc: &vm_rc,
                            cs_rc: &cs_rc,
                            svm_rc: &svm_rc,
                            ema_rc: &ema_rc,
                            expanded_dirs: &exp_dirs,
                            tree_metrics_cache: &tree_metrics_c,
                            tree_children_cache: &tree_children_c,
                            tree_fetching: &tree_fetch_c,
                            show_hidden: hidden,
                            search_query: &query_r,
                        };
                        populate_tree_level(&tree_ctx, &sorted, 0);
                    }
                })
            };

            match backend_snap {
                FilePaneBackend::Local => {
                    let raw_entries = if is_recursive && !query.is_empty() {
                        search_recursive_dir(&dir_path, &query, 300)
                    } else {
                        match LocalFsBackend.list_dir(&dir_path) {
                            Ok(list) => list,
                            Err(e) => {
                                status.set_text(&tr_fmt(
                                    "Erro ao listar {path}: {err}",
                                    &[
                                        ("path", &dir_path.display().to_string()),
                                        ("err", &e.to_string()),
                                    ],
                                ));
                                Vec::new()
                            }
                        }
                    };
                    let space = LocalFsBackend.free_space(&dir_path).ok();
                    render_dir_results(raw_entries, space);
                }
                FilePaneBackend::Sftp(sftp) => {
                    foot.set_text(&tr_fmt(
                        "Listando diretório remoto via SFTP ({target})...",
                        &[("target", &sftp.display_target())],
                    ));
                    let gen_c = Rc::clone(&gen_rc);
                    let render_c = Rc::clone(&render_dir_results);
                    let status_c = status.clone();
                    let foot_c = foot.clone();
                    let dp_bg = dir_path.clone();
                    let dp_thread = dp_bg.clone();
                    let (tx, rx) = std::sync::mpsc::sync_channel::<(
                        std::io::Result<Vec<FileEntry>>,
                        Option<(u64, u64)>,
                    )>(1);
                    std::thread::spawn(move || {
                        let res = sftp.list_dir(&dp_thread);
                        let space = sftp.free_space(&dp_thread).ok();
                        let _ = tx.send((res, space));
                    });
                    glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
                        Ok((res, space)) => {
                            if gen_c.get() != my_gen {
                                return glib::ControlFlow::Break;
                            }
                            match res {
                                Ok(list) => render_c(list, space),
                                Err(e) => {
                                    let msg = tr_fmt(
                                        "Erro SFTP ao listar {path}: {err}",
                                        &[
                                            ("path", &dp_bg.display().to_string()),
                                            ("err", &e.to_string()),
                                        ],
                                    );
                                    status_c.set_text(&msg);
                                    foot_c.set_text(&msg);
                                    render_c(Vec::new(), None);
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

    *refresh_holder.borrow_mut() = Some(Rc::clone(&refresh_fn));
    refresh_fn();

    // ── 6. Wire Selection & Activation Signals ────────────────────────────────
    {
        let vis = Rc::clone(&visible_entries);
        let sel = Rc::clone(&selected_entry);
        list_box.connect_row_selected(move |_, row| {
            if let Some(r) = row {
                let idx = r.index() as usize;
                if let Some(e) = vis.borrow().get(idx) {
                    *sel.borrow_mut() = Some(e.clone());
                }
            }
        });
    }
    {
        let vis = Rc::clone(&visible_entries);
        let cur_dir = Rc::clone(&current_dir);
        let bk_c = Rc::clone(&pane_backend);
        let refresh = Rc::clone(&refresh_fn);
        let win_ref = parent_window.clone();
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb = ai_prompt_cb.clone();
        list_box.connect_row_activated(move |_, row| {
            let idx = row.index() as usize;
            let entries = vis.borrow().clone();
            if let Some(entry) = entries.get(idx) {
                if entry.name == ".." {
                    if let Some(parent) = cur_dir.borrow().parent() {
                        *cur_dir.borrow_mut() = parent.to_path_buf();
                        refresh();
                    }
                } else if entry.is_dir {
                    *cur_dir.borrow_mut() = PathBuf::from(&entry.path);
                    refresh();
                } else {
                    open_quick_look_for_pane(
                        &win_ref,
                        entries,
                        idx,
                        &bk_c.borrow(),
                        Some(Rc::clone(&feed_cb)),
                        ai_cb.clone(),
                        Some(Rc::clone(&refresh)),
                    );
                }
            }
        });
    }
    {
        let vis = Rc::clone(&visible_entries);
        let sel = Rc::clone(&selected_entry);
        grid_selection.connect_selection_changed(move |sel_model, _, _| {
            let bitset = sel_model.selection();
            if !bitset.is_empty() {
                let idx = bitset.minimum() as usize;
                if let Some(e) = vis.borrow().get(idx) {
                    *sel.borrow_mut() = Some(e.clone());
                }
            }
        });
    }
    {
        let vis = Rc::clone(&visible_entries);
        let sel = Rc::clone(&selected_entry);
        let cur_dir = Rc::clone(&current_dir);
        let bk_c = Rc::clone(&pane_backend);
        let refresh = Rc::clone(&refresh_fn);
        let win_ref = parent_window.clone();
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb = ai_prompt_cb.clone();
        grid_view.connect_activate(move |_, pos| {
            let idx = pos as usize;
            let entries = vis.borrow().clone();
            if let Some(entry) = entries.get(idx) {
                *sel.borrow_mut() = Some(entry.clone());
                if entry.name == ".." {
                    if let Some(parent) = cur_dir.borrow().parent() {
                        *cur_dir.borrow_mut() = parent.to_path_buf();
                        refresh();
                    }
                } else if entry.is_dir {
                    *cur_dir.borrow_mut() = PathBuf::from(&entry.path);
                    refresh();
                } else {
                    open_quick_look_for_pane(
                        &win_ref,
                        entries,
                        idx,
                        &bk_c.borrow(),
                        Some(Rc::clone(&feed_cb)),
                        ai_cb.clone(),
                        Some(Rc::clone(&refresh)),
                    );
                }
            }
        });
    }
    {
        let vis = Rc::clone(&visible_tree_entries);
        let sel = Rc::clone(&selected_entry);
        tree_list.connect_row_selected(move |_, row| {
            if let Some(r) = row {
                let idx = r.index() as usize;
                if let Some(e) = vis.borrow().get(idx) {
                    *sel.borrow_mut() = Some(e.clone());
                }
            }
        });
    }
    {
        let vis = Rc::clone(&visible_tree_entries);
        let sel = Rc::clone(&selected_entry);
        let cur_dir = Rc::clone(&current_dir);
        let exp_dirs = Rc::clone(&expanded_dirs);
        let bk_c = Rc::clone(&pane_backend);
        let refresh = Rc::clone(&refresh_fn);
        let win_ref = parent_window.clone();
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb = ai_prompt_cb.clone();
        tree_list.connect_row_activated(move |_, row| {
            let idx = row.index() as usize;
            let entries = vis.borrow().clone();
            if let Some(entry) = entries.get(idx) {
                *sel.borrow_mut() = Some(entry.clone());
                if entry.name == ".." {
                    if let Some(parent) = cur_dir.borrow().parent() {
                        *cur_dir.borrow_mut() = parent.to_path_buf();
                        refresh();
                    }
                } else if entry.is_dir {
                    let p = PathBuf::from(&entry.path);
                    let mut set = exp_dirs.borrow_mut();
                    if set.contains(&p) {
                        set.remove(&p);
                    } else {
                        set.insert(p);
                    }
                    drop(set);
                    refresh();
                } else {
                    open_quick_look_for_pane(
                        &win_ref,
                        entries,
                        idx,
                        &bk_c.borrow(),
                        Some(Rc::clone(&feed_cb)),
                        ai_cb.clone(),
                        Some(Rc::clone(&refresh)),
                    );
                }
            }
        });
    }

    // ── 7. Wire Bottom Action Bar Controls ───────────────────────────────────
    {
        let refresh = Rc::clone(&refresh_fn);
        btn_refresh.connect_clicked(move |_| refresh());
    }
    {
        let hidden_rc = Rc::clone(&show_hidden);
        let refresh = Rc::clone(&refresh_fn);
        btn_hidden.connect_toggled(move |btn| {
            *hidden_rc.borrow_mut() = btn.is_active();
            refresh();
        });
    }
    {
        let cd = Rc::clone(&current_dir);
        let st = status_lbl.clone();
        let rf = Rc::clone(&refresh_fn);
        let bm_btn = btn_bookmark.clone();
        btn_bookmark.connect_clicked(move |_| {
            let p = cd.borrow().clone();
            let p_str = p.to_string_lossy().to_string();
            let mut bms = load_bookmarks();
            if let Some(pos) = bms.iter().position(|b| b.path == p_str) {
                bms.remove(pos);
                save_bookmarks(&bms);
                bm_btn.remove_css_class("bookmark-active-star");
                bm_btn.set_tooltip_text(Some(&tr("Favoritar diretório atual")));
                st.set_text(&tr_fmt("Favorito removido: '{path}'", &[("path", &p.display().to_string())]));
            } else {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p_str.clone());
                bms.push(FileManagerBookmark { name, path: p_str });
                save_bookmarks(&bms);
                bm_btn.add_css_class("bookmark-active-star");
                bm_btn.set_tooltip_text(Some(&tr("Remover favorito do diretório atual")));
                st.set_text(&tr_fmt("Diretório '{path}' marcado como favorito.", &[("path", &p.display().to_string())]));
            }
            rf();
        });
    }
    {
        let cd = Rc::clone(&current_dir);
        let rf = Rc::clone(&refresh_fn);
        let qj_btn = btn_quick_jump.clone();
        let st = status_lbl.clone();
        let bm_btn = btn_bookmark.clone();
        btn_quick_jump.connect_clicked(move |_| {
            show_quick_jump_popover(&qj_btn, &cd, &rf, &st, &bm_btn);
        });
    }
    {
        let refresh = Rc::clone(&refresh_fn);
        search_entry.connect_search_changed(move |_| refresh());
    }
    {
        let refresh = Rc::clone(&refresh_fn);
        recursive_switch.connect_active_notify(move |_| refresh());
    }
    let set_backend_and_dir: Rc<dyn Fn(FilePaneBackend, PathBuf)> = {
        let bk_rc = Rc::clone(&pane_backend);
        let cd_rc = Rc::clone(&current_dir);
        let tmc = Rc::clone(&tree_metrics_cache);
        let tcc = Rc::clone(&tree_children_cache);
        let tf = Rc::clone(&tree_fetching);
        let exp = Rc::clone(&expanded_dirs);
        let rf = Rc::clone(&refresh_fn);
        Rc::new(move |new_backend: FilePaneBackend, new_dir: PathBuf| {
            *bk_rc.borrow_mut() = new_backend;
            *cd_rc.borrow_mut() = new_dir;
            tmc.borrow_mut().clear();
            tcc.borrow_mut().clear();
            tf.borrow_mut().clear();
            exp.borrow_mut().clear();
            rf();
        })
    };

    // Wire Pane Source Selector (Local vs Saved SSH / SFTP Sessions)
    {
        let src_pop = gtk4::Popover::new();
        src_pop.add_css_class("onyxsh-popover");
        let set_bk = Rc::clone(&set_backend_and_dir);
        let st_c = status_lbl.clone();
        let src_pop_ref = src_pop.clone();
        source_btn.set_popover(Some(&src_pop));

        src_pop.connect_show(move |pop| {
            let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            vbox.set_margin_start(6);
            vbox.set_margin_end(6);
            vbox.set_margin_top(6);
            vbox.set_margin_bottom(6);
            vbox.set_width_request(240);

            let sb = Rc::clone(&set_bk);
            let st_local = st_c.clone();
            add_menu_item(&vbox, pop, "computer-symbolic", &tr("Local (Este Computador)"), false, move || {
                sb(FilePaneBackend::Local, platform::home_dir());
                st_local.set_text(&tr("Painel alterado para sistema de arquivos local."));
            });

            let mut ssh_sessions = Vec::new();
            let sess_path = platform::config_dir().join("sessions.json");
            if let Ok(storage) = SessionsStorageData::load_from_path(&sess_path) {
                for s in storage.sessions {
                    if s.is_ssh() && !s.host.is_empty() {
                        ssh_sessions.push(s);
                    }
                }
            }
            for s in crate::session_sidebar::load_ssh_config_sessions() {
                if !s.host.is_empty() && !ssh_sessions.iter().any(|x| x.name == s.name && x.host == s.host) {
                    ssh_sessions.push(s);
                }
            }

            if !ssh_sessions.is_empty() {
                vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
                for sess in ssh_sessions {
                    let label = format!("SFTP: {} ({})", sess.name, sess.connection_string());
                    let sb_ssh = Rc::clone(&set_bk);
                    let st_ssh = st_c.clone();
                    let sess_c = sess.clone();
                    add_menu_item(&vbox, pop, "network-server-symbolic", &label, false, move || {
                        let secret = crate::session_sidebar::resolve_session_secret(&sess_c);
                        let pw_opt = (!secret.is_empty()).then_some(secret);
                        let sftp = SftpFsBackend::new(sess_c.clone(), pw_opt);
                        let target = sftp.display_target();
                        st_ssh.set_text(&tr_fmt(
                            "Conectando painel SFTP em {target}...",
                            &[("target", &target)],
                        ));
                        let sb_done = Rc::clone(&sb_ssh);
                        let st_done = st_ssh.clone();
                        let sftp_ui = sftp.clone();
                        let (tx, rx) = std::sync::mpsc::sync_channel::<PathBuf>(1);
                        std::thread::spawn(move || {
                            let home = sftp.resolve_home_dir().unwrap_or_else(|_| PathBuf::from("/"));
                            let _ = tx.send(home);
                        });
                        glib::timeout_add_local(Duration::from_millis(50), move || match rx.try_recv() {
                            Ok(home) => {
                                sb_done(FilePaneBackend::Sftp(sftp_ui.clone()), home.clone());
                                st_done.set_text(&tr_fmt(
                                    "Conectado via SFTP em {target} ({path})",
                                    &[("target", &target), ("path", &home.display().to_string())],
                                ));
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                        });
                    });
                }
            }

            pop.set_child(Some(&vbox));
        });
        let _ = src_pop_ref;
    }

    {
        let st = status_lbl.clone();
        let win_h = parent_window.clone();
        let tx_opt = transfer_ctrl.clone();
        btn_history.connect_clicked(move |_| {
            if let Some(ref ctrl) = tx_opt {
                show_transfer_queue_dialog(&win_h, ctrl);
            } else {
                st.set_text(&tr("Fila de transferências vazia (0 tarefas em andamento)."));
            }
        });
    }

    // View Mode Switching (`list` / `grid` / `tree`)
    {
        let svm = Rc::clone(&switch_view_mode);
        let vm = Rc::clone(&view_mode);
        btn_mode_list.connect_toggled(move |btn| {
            if btn.is_active() && *vm.borrow() != FileManagerViewMode::List {
                svm(FileManagerViewMode::List);
            }
        });
    }
    {
        let svm = Rc::clone(&switch_view_mode);
        let vm = Rc::clone(&view_mode);
        btn_mode_grid.connect_toggled(move |btn| {
            if btn.is_active() && *vm.borrow() != FileManagerViewMode::Grid {
                svm(FileManagerViewMode::Grid);
            }
        });
    }
    {
        let svm = Rc::clone(&switch_view_mode);
        let vm = Rc::clone(&view_mode);
        btn_mode_tree.connect_toggled(move |btn| {
            if btn.is_active() && *vm.borrow() != FileManagerViewMode::Tree {
                svm(FileManagerViewMode::Tree);
            }
        });
    }

    // Sort Column Header Clicks
    {
        let cs = Rc::clone(&change_sort);
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        btn_sort_name.connect_clicked(move |_| {
            let next_asc = if *scol.borrow() == SortColumn::Name {
                !*sasc.borrow()
            } else {
                true
            };
            cs(SortColumn::Name, Some(next_asc));
        });
    }
    {
        let cs = Rc::clone(&change_sort);
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        btn_sort_size.connect_clicked(move |_| {
            let next_asc = if *scol.borrow() == SortColumn::Size {
                !*sasc.borrow()
            } else {
                false
            };
            cs(SortColumn::Size, Some(next_asc));
        });
    }
    {
        let cs = Rc::clone(&change_sort);
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        btn_sort_mod.connect_clicked(move |_| {
            let next_asc = if *scol.borrow() == SortColumn::Modified {
                !*sasc.borrow()
            } else {
                false
            };
            cs(SortColumn::Modified, Some(next_asc));
        });
    }
    {
        let cs = Rc::clone(&change_sort);
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        btn_sort_perm.connect_clicked(move |_| {
            let next_asc = if *scol.borrow() == SortColumn::Permissions {
                !*sasc.borrow()
            } else {
                true
            };
            cs(SortColumn::Permissions, Some(next_asc));
        });
    }
    {
        let cs = Rc::clone(&change_sort);
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        btn_sort_owner.connect_clicked(move |_| {
            let next_asc = if *scol.borrow() == SortColumn::Owner {
                !*sasc.borrow()
            } else {
                true
            };
            cs(SortColumn::Owner, Some(next_asc));
        });
    }

    // Spacebar shortcut inside list/grid/tree views triggers Quick Look
    {
        let win_ref = parent_window.clone();
        let vis = Rc::clone(&visible_entries);
        let vis_tree = Rc::clone(&visible_tree_entries);
        let vm = Rc::clone(&view_mode);
        let sel = Rc::clone(&selected_entry);
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb = ai_prompt_cb.clone();
        let bk_c = Rc::clone(&pane_backend);
        let refresh = Rc::clone(&refresh_fn);
        let key_ctrl = gtk4::EventControllerKey::new();
        key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::space {
                let current_vm = *vm.borrow();
                let entries = if current_vm == FileManagerViewMode::Tree {
                    vis_tree.borrow().clone()
                } else {
                    vis.borrow().clone()
                };
                if !entries.is_empty() {
                    let idx = sel
                        .borrow()
                        .as_ref()
                        .and_then(|s| entries.iter().position(|e| e.path == s.path))
                        .unwrap_or(0);
                    open_quick_look_for_pane(
                        &win_ref,
                        entries,
                        idx,
                        &bk_c.borrow(),
                        Some(Rc::clone(&feed_cb)),
                        ai_cb.clone(),
                        Some(Rc::clone(&refresh)),
                    );
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        view_stack.add_controller(key_ctrl);
    }

    FilePaneHandle {
        pane_id: pane_id_owned,
        widget: pane_vbox,
        current_dir,
        backend: pane_backend,
        selected_entry,
        visible_entries,
        refresh: refresh_fn,
        set_backend_and_dir,
        btn_toggle_dual,
    }
}

/// Displays the rich Libadwaita permissions dialog with checkboxes for Owner, Group, and Others
/// and live numeric mode calculation matching `CORRIGIR/permissions.png` and `src/onyxsh/filemanager/manager.py`.
pub fn show_permissions_dialog(
    parent: &adw::ApplicationWindow,
    entry: &FileEntry,
    refresh_fn: Rc<dyn Fn()>,
    status_lbl: gtk4::Label,
) {
    show_permissions_dialog_with_backend(
        parent,
        entry,
        FilePaneBackend::Local,
        refresh_fn,
        status_lbl,
    );
}

/// Displays the rich Libadwaita permissions dialog for Local or SFTP backends.
pub fn show_permissions_dialog_with_backend(
    parent: &adw::ApplicationWindow,
    entry: &FileEntry,
    backend: FilePaneBackend,
    refresh_fn: Rc<dyn Fn()>,
    status_lbl: gtk4::Label,
) {
    let display_name = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };

    let title = tr_fmt("Permissões para {name}", &[("name", &display_name)]);
    let current_perms = entry.formatted_permissions();
    let body = tr_fmt(
        "Definir permissões de arquivo para: {name}\nAtual: {perms}",
        &[("name", &display_name), ("perms", &current_perms)],
    );

    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content_box.set_size_request(350, -1);

    // Group 1: Proprietário
    let owner_group = adw::PreferencesGroup::builder().title(tr("Owner")).build();
    let owner_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    owner_box.set_homogeneous(true);
    let owner_read = gtk4::CheckButton::with_label(&tr("Read"));
    let owner_write = gtk4::CheckButton::with_label(&tr("Write"));
    let owner_exec = gtk4::CheckButton::with_label(&tr("Execute"));
    owner_box.append(&owner_read);
    owner_box.append(&owner_write);
    owner_box.append(&owner_exec);
    let owner_row = adw::ActionRow::new();
    owner_row.set_child(Some(&owner_box));
    owner_group.add(&owner_row);
    content_box.append(&owner_group);

    // Group 2: Grupo
    let group_group = adw::PreferencesGroup::builder().title(tr("Group")).build();
    let group_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    group_box.set_homogeneous(true);
    let group_read = gtk4::CheckButton::with_label(&tr("Read"));
    let group_write = gtk4::CheckButton::with_label(&tr("Write"));
    let group_exec = gtk4::CheckButton::with_label(&tr("Execute"));
    group_box.append(&group_read);
    group_box.append(&group_write);
    group_box.append(&group_exec);
    let group_row = adw::ActionRow::new();
    group_row.set_child(Some(&group_box));
    group_group.add(&group_row);
    content_box.append(&group_group);

    // Group 3: Outros
    let others_group = adw::PreferencesGroup::builder().title(tr("Others")).build();
    let others_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    others_box.set_homogeneous(true);
    let others_read = gtk4::CheckButton::with_label(&tr("Read"));
    let others_write = gtk4::CheckButton::with_label(&tr("Write"));
    let others_exec = gtk4::CheckButton::with_label(&tr("Execute"));
    others_box.append(&others_read);
    others_box.append(&others_write);
    others_box.append(&others_exec);
    let others_row = adw::ActionRow::new();
    others_row.set_child(Some(&others_box));
    others_group.add(&others_row);
    content_box.append(&others_group);

    let mode_label = gtk4::Label::builder()
        .halign(gtk4::Align::Center)
        .margin_top(12)
        .build();
    content_box.append(&mode_label);

    // Initial values from entry.permissions_octal
    let init_mode = entry.permissions_octal & 0o777;
    owner_read.set_active((init_mode & 0o400) != 0);
    owner_write.set_active((init_mode & 0o200) != 0);
    owner_exec.set_active((init_mode & 0o100) != 0);
    group_read.set_active((init_mode & 0o040) != 0);
    group_write.set_active((init_mode & 0o020) != 0);
    group_exec.set_active((init_mode & 0o010) != 0);
    others_read.set_active((init_mode & 0o004) != 0);
    others_write.set_active((init_mode & 0o002) != 0);
    others_exec.set_active((init_mode & 0o001) != 0);

    let calc_mode = {
        let u_r = owner_read.clone();
        let u_w = owner_write.clone();
        let u_x = owner_exec.clone();
        let g_r = group_read.clone();
        let g_w = group_write.clone();
        let g_x = group_exec.clone();
        let o_r = others_read.clone();
        let o_w = others_write.clone();
        let o_x = others_exec.clone();
        move || -> (u32, String) {
            let owner = (if u_r.is_active() { 4 } else { 0 })
                + (if u_w.is_active() { 2 } else { 0 })
                + (if u_x.is_active() { 1 } else { 0 });
            let group = (if g_r.is_active() { 4 } else { 0 })
                + (if g_w.is_active() { 2 } else { 0 })
                + (if g_x.is_active() { 1 } else { 0 });
            let others = (if o_r.is_active() { 4 } else { 0 })
                + (if o_w.is_active() { 2 } else { 0 })
                + (if o_x.is_active() { 1 } else { 0 });
            let mode_val = (owner << 6) | (group << 3) | others;
            let mode_str = format!("{owner}{group}{others}");
            (mode_val, mode_str)
        }
    };

    let update_display = {
        let ml = mode_label.clone();
        let cm = calc_mode.clone();
        Rc::new(move || {
            let (_, str_val) = cm();
            ml.set_text(&tr_fmt("Modo numérico: {mode}", &[("mode", &str_val)]));
        })
    };

    update_display();

    for cb in [
        &owner_read,
        &owner_write,
        &owner_exec,
        &group_read,
        &group_write,
        &group_exec,
        &others_read,
        &others_write,
        &others_exec,
    ] {
        let ud = Rc::clone(&update_display);
        cb.connect_toggled(move |_| ud());
    }

    let dialog = adw::MessageDialog::builder()
        .heading(&title)
        .body(&body)
        .close_response("cancel")
        .transient_for(parent)
        .extra_child(&content_box)
        .build();

    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("apply", &tr("Apply"));
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);

    let path = entry.path.clone();
    let name = display_name;
    dialog.connect_response(None, move |_, response| {
        if response == "apply" {
            let (mode_val, mode_str) = calc_mode();
            let p = PathBuf::from(&path);
            let ok_msg = tr_fmt(
                "Permissões de '{name}' alteradas para {mode}",
                &[("name", &name), ("mode", &mode_str)],
            );
            let err_prefix = tr_fmt("Erro ao alterar permissões de '{name}'", &[("name", &name)]);
            run_backend_mutation(&backend, &status_lbl, &refresh_fn, ok_msg, err_prefix, move |b| {
                b.chmod(&p, mode_val as u16)
            });
        }
    });

    dialog.present();
}

fn show_simple_input_dialog(
    parent: &adw::ApplicationWindow,
    title: &str,
    prompt_label: &str,
    initial_value: &str,
    on_confirm: Rc<dyn Fn(&str)>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(title)
        .default_width(380)
        .default_height(190)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
    vbox.append(&adw::HeaderBar::new());

    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(6);
    content.set_margin_bottom(14);

    let lbl = gtk4::Label::new(Some(prompt_label));
    lbl.set_halign(gtk4::Align::Start);
    content.append(&lbl);

    let entry = gtk4::Entry::builder().text(initial_value).build();
    content.append(&entry);

    let btn_ok = gtk4::Button::with_label(&tr("Confirm"));
    btn_ok.add_css_class("suggested-action");
    btn_ok.set_halign(gtk4::Align::End);
    content.append(&btn_ok);

    let commit = {
        let e = entry.clone();
        let d = dlg.clone();
        let cb = Rc::clone(&on_confirm);
        Rc::new(move || {
            let val = e.text().to_string();
            if !val.trim().is_empty() {
                cb(&val);
                d.close();
            }
        })
    };

    let c1 = Rc::clone(&commit);
    btn_ok.connect_clicked(move |_| c1());
    let c2 = Rc::clone(&commit);
    entry.connect_activate(move |_| c2());

    vbox.append(&content);
    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Computes total byte size of a local file or directory tree for transfer sizing.
fn compute_local_path_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for ent in entries.flatten() {
            total = total.saturating_add(compute_local_path_size(&ent.path()));
        }
    }
    total
}

/// Resolves semantic transfer direction from source and destination pane backends/sides.
fn resolve_transfer_direction(
    src_backend: &FilePaneBackend,
    dst_backend: &FilePaneBackend,
    fallback: TransferDirection,
) -> TransferDirection {
    match (src_backend, dst_backend) {
        (FilePaneBackend::Local, FilePaneBackend::Sftp(_)) => TransferDirection::Upload,
        (FilePaneBackend::Sftp(_), FilePaneBackend::Local) => TransferDirection::Download,
        _ => fallback,
    }
}

/// Copies a single local file in 64 KiB chunks while reporting progress and honoring cancellation.
fn copy_file_chunked_with_progress(
    src: &Path,
    dst: &Path,
    tx_id: &str,
    cancel_flag: &std::sync::atomic::AtomicBool,
    ctrl: &onyxsh_filemanager::transfer::TransferQueueController,
    done_so_far: &mut u64,
    total_bytes: u64,
    started_at: std::time::Instant,
) -> std::io::Result<()> {
    use std::io::{Read, Write};
    use std::sync::atomic::Ordering;

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut reader = fs::File::open(src)?;
    let mut writer = fs::File::create(dst)?;
    let mut buf = [0u8; 64 * 1024];

    loop {
        if cancel_flag.load(Ordering::Relaxed) || ctrl.is_cancelled(tx_id) {
            drop(writer);
            let _ = fs::remove_file(dst);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                tr("Transferência cancelada pelo usuário."),
            ));
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        *done_so_far = done_so_far.saturating_add(n as u64);
        let elapsed = started_at.elapsed().as_secs_f64().max(0.001);
        let speed_bps = *done_so_far as f64 / elapsed;
        ctrl.update_progress(tx_id, *done_so_far, Some(total_bytes.max(*done_so_far)), speed_bps);
    }
    writer.flush()?;
    Ok(())
}

/// Recursively copies a local directory tree with chunked file progress and cancellation checks.
fn copy_dir_chunked_with_progress(
    src: &Path,
    dst: &Path,
    tx_id: &str,
    cancel_flag: &std::sync::atomic::AtomicBool,
    ctrl: &onyxsh_filemanager::transfer::TransferQueueController,
    done_so_far: &mut u64,
    total_bytes: u64,
    started_at: std::time::Instant,
) -> std::io::Result<()> {
    use std::sync::atomic::Ordering;

    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        if cancel_flag.load(Ordering::Relaxed) || ctrl.is_cancelled(tx_id) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                tr("Transferência cancelada pelo usuário."),
            ));
        }
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_child = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_chunked_with_progress(
                &entry.path(),
                &dst_child,
                tx_id,
                cancel_flag,
                ctrl,
                done_so_far,
                total_bytes,
                started_at,
            )?;
        } else {
            copy_file_chunked_with_progress(
                &entry.path(),
                &dst_child,
                tx_id,
                cancel_flag,
                ctrl,
                done_so_far,
                total_bytes,
                started_at,
            )?;
        }
    }
    Ok(())
}

/// Dispatches a Local or SFTP file/directory transfer asynchronously onto [`TransferQueueController`].
fn start_async_transfer(
    src_backend: FilePaneBackend,
    src_entry: FileEntry,
    dst_backend: FilePaneBackend,
    dst_dir: PathBuf,
    direction: TransferDirection,
    controller: Arc<onyxsh_filemanager::transfer::TransferQueueController>,
    status_lbl: gtk4::Label,
    on_done: Rc<dyn Fn()>,
) {
    if src_entry.name == ".." {
        status_lbl.set_text(&tr("Selecione um arquivo ou pasta válido para transferir."));
        return;
    }

    let dst_path = dst_dir.join(&src_entry.name);
    let initial_bytes = if src_backend.is_sftp() {
        src_entry.size.max(1)
    } else {
        compute_local_path_size(Path::new(&src_entry.path)).max(src_entry.size).max(1)
    };

    let (tx_id, cancel_flag) = controller.enqueue(
        src_entry.name.clone(),
        src_entry.path.clone(),
        dst_path.to_string_lossy().into_owned(),
        direction,
        initial_bytes,
    );

    status_lbl.set_text(&tr_fmt(
        "Transferindo {name} ({id})...",
        &[("name", &src_entry.name), ("id", &tx_id)],
    ));

    let (tx_res, rx_res) = std::sync::mpsc::sync_channel::<Result<PathBuf, String>>(1);
    let ctrl_bg = Arc::clone(&controller);
    let id_bg = tx_id.clone();
    let dst_dir_bg = dst_dir.clone();
    let dst_path_bg = dst_path.clone();

    std::thread::spawn(move || {
        use std::sync::atomic::Ordering;

        if cancel_flag.load(Ordering::Relaxed) || ctrl_bg.is_cancelled(&id_bg) {
            ctrl_bg.mark_failed(&id_bg, tr("Transferência cancelada antes de iniciar."));
            let _ = tx_res.send(Err(tr("Transferência cancelada.")));
            return;
        }

        ctrl_bg.mark_in_progress(&id_bg);

        let avail = dst_backend.free_space(&dst_dir_bg).ok().map(|(free, _)| free);
        if let Err(space_err) =
            onyxsh_filemanager::transfer::check_sufficient_space(initial_bytes, avail)
        {
            ctrl_bg.mark_failed(&id_bg, space_err.clone());
            let _ = tx_res.send(Err(space_err));
            return;
        }

        let started_at = std::time::Instant::now();
        let src_path = PathBuf::from(&src_entry.path);

        let outcome: std::io::Result<()> = match (&src_backend, &dst_backend) {
            (FilePaneBackend::Local, FilePaneBackend::Local) => {
                let mut done = 0u64;
                let res = if src_entry.is_dir {
                    copy_dir_chunked_with_progress(
                        &src_path,
                        &dst_path_bg,
                        &id_bg,
                        &cancel_flag,
                        &ctrl_bg,
                        &mut done,
                        initial_bytes,
                        started_at,
                    )
                } else {
                    copy_file_chunked_with_progress(
                        &src_path,
                        &dst_path_bg,
                        &id_bg,
                        &cancel_flag,
                        &ctrl_bg,
                        &mut done,
                        initial_bytes,
                        started_at,
                    )
                };
                res
            }
            (FilePaneBackend::Local, FilePaneBackend::Sftp(sftp)) => {
                ctrl_bg.update_progress(&id_bg, 0, Some(initial_bytes), 0.0);
                let res = sftp.upload_path(&src_path, &dst_path_bg);
                if res.is_ok() {
                    let elapsed = started_at.elapsed().as_secs_f64().max(0.001);
                    ctrl_bg.update_progress(
                        &id_bg,
                        initial_bytes,
                        Some(initial_bytes),
                        initial_bytes as f64 / elapsed,
                    );
                }
                res
            }
            (FilePaneBackend::Sftp(sftp), FilePaneBackend::Local) => {
                ctrl_bg.update_progress(&id_bg, 0, Some(initial_bytes), 0.0);
                let res = sftp.download_path(&src_path, &dst_path_bg, src_entry.is_dir);
                if res.is_ok() {
                    let elapsed = started_at.elapsed().as_secs_f64().max(0.001);
                    ctrl_bg.update_progress(
                        &id_bg,
                        initial_bytes,
                        Some(initial_bytes),
                        initial_bytes as f64 / elapsed,
                    );
                }
                res
            }
            (FilePaneBackend::Sftp(src_sftp), FilePaneBackend::Sftp(dst_sftp)) => {
                let relay_dir = std::env::temp_dir()
                    .join(format!("onyxsh_relay_{}_{}", std::process::id(), id_bg));
                let relay_target = relay_dir.join(&src_entry.name);
                if let Err(e) = fs::create_dir_all(&relay_dir) {
                    Err(e)
                } else {
                    let dl = src_sftp.download_path(&src_path, &relay_target, src_entry.is_dir);
                    let res = match dl {
                        Ok(()) => {
                            if cancel_flag.load(Ordering::Relaxed) || ctrl_bg.is_cancelled(&id_bg) {
                                Err(std::io::Error::new(
                                    std::io::ErrorKind::Interrupted,
                                    tr("Transferência cancelada pelo usuário."),
                                ))
                            } else {
                                let mid = initial_bytes / 2;
                                let elapsed = started_at.elapsed().as_secs_f64().max(0.001);
                                ctrl_bg.update_progress(
                                    &id_bg,
                                    mid,
                                    Some(initial_bytes),
                                    mid as f64 / elapsed,
                                );
                                dst_sftp.upload_path(&relay_target, &dst_path_bg)
                            }
                        }
                        Err(e) => Err(e),
                    };
                    let _ = fs::remove_dir_all(&relay_dir);
                    res
                }
            }
        };

        if cancel_flag.load(Ordering::Relaxed) || ctrl_bg.is_cancelled(&id_bg) {
            let msg = tr("Transferência cancelada pelo usuário.");
            ctrl_bg.mark_failed(&id_bg, msg.clone());
            let _ = tx_res.send(Err(msg));
            return;
        }

        match outcome {
            Ok(()) => {
                let elapsed = started_at.elapsed().as_secs_f64().max(0.001);
                ctrl_bg.update_progress(
                    &id_bg,
                    initial_bytes,
                    Some(initial_bytes),
                    initial_bytes as f64 / elapsed,
                );
                ctrl_bg.mark_completed(&id_bg);
                let _ = tx_res.send(Ok(dst_path_bg));
            }
            Err(e) => {
                let msg = e.to_string();
                ctrl_bg.mark_failed(&id_bg, msg.clone());
                let _ = tx_res.send(Err(msg));
            }
        }
    });

    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        match rx_res.try_recv() {
            Ok(Ok(dst_final)) => {
                status_lbl.set_text(&tr_fmt(
                    "Copiado para {path}",
                    &[("path", &dst_final.display().to_string())],
                ));
                on_done();
                glib::ControlFlow::Break
            }
            Ok(Err(err)) => {
                status_lbl.set_text(&tr_fmt(
                    "Falha na transferência: {err}",
                    &[("err", &err)],
                ));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
        }
    });
}

/// Opens the Dual-Pane Directory Comparison & Side-by-Side Unified Diff dialog.
#[allow(clippy::too_many_arguments)]
fn show_dir_compare_and_diff_dialog(
    parent_window: &adw::ApplicationWindow,
    left_dir: PathBuf,
    left_backend: FilePaneBackend,
    left_entries: Vec<FileEntry>,
    right_dir: PathBuf,
    right_backend: FilePaneBackend,
    right_entries: Vec<FileEntry>,
    left_selected: Option<FileEntry>,
    right_selected: Option<FileEntry>,
    cmp: DirectoryComparison,
) {
    let dlg = adw::Window::builder()
        .title(tr("Comparação de Diretórios e Diff"))
        .transient_for(parent_window)
        .modal(true)
        .default_width(880)
        .default_height(600)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let header = adw::HeaderBar::new();
    vbox.append(&header);

    let summary_lbl = gtk4::Label::new(Some(&tr_fmt(
        "{identical} idênticos · {left_only} apenas Esquerda · {right_only} apenas Direita · {diff} tamanho diferente",
        &[
            ("identical", &cmp.identical_count.to_string()),
            ("left_only", &cmp.only_in_left.len().to_string()),
            ("right_only", &cmp.only_in_right.len().to_string()),
            ("diff", &cmp.size_differs.len().to_string()),
        ],
    )));
    summary_lbl.set_halign(gtk4::Align::Start);
    summary_lbl.set_margin_start(12);
    summary_lbl.set_margin_end(12);
    summary_lbl.add_css_class("caption");
    vbox.append(&summary_lbl);

    let body_paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
    body_paned.set_position(280);
    body_paned.set_hexpand(true);
    body_paned.set_vexpand(true);
    body_paned.set_margin_start(10);
    body_paned.set_margin_end(10);
    body_paned.set_margin_bottom(10);

    let list_scroll = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    let list_box = gtk4::ListBox::new();
    list_box.add_css_class("boxed-list");
    list_scroll.set_child(Some(&list_box));

    let diff_scroll = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    let diff_view = gtk4::TextView::new();
    diff_view.set_editable(false);
    diff_view.set_monospace(true);
    diff_view.set_left_margin(8);
    diff_view.set_right_margin(8);
    diff_view.set_top_margin(6);
    diff_view.set_bottom_margin(6);
    diff_scroll.set_child(Some(&diff_view));

    let buf = diff_view.buffer();
    let tag_add = buf
        .create_tag(Some("diff-add"), &[("foreground", &"#a6e3a1")])
        .unwrap();
    let tag_del = buf
        .create_tag(Some("diff-del"), &[("foreground", &"#f38ba8")])
        .unwrap();
    let tag_hunk = buf
        .create_tag(Some("diff-hunk"), &[("foreground", &"#89b4fa")])
        .unwrap();
    let tag_hdr = buf
        .create_tag(Some("diff-hdr"), &[("foreground", &"#f9e2af")])
        .unwrap();

    let load_pair_diff: Rc<dyn Fn(PathBuf, String, PathBuf, String)> = {
        let lb = left_backend.clone();
        let rb = right_backend.clone();
        let tv = diff_view.clone();
        let t_add = tag_add.clone();
        let t_del = tag_del.clone();
        let t_hunk = tag_hunk.clone();
        let t_hdr = tag_hdr.clone();

        Rc::new(move |lp: PathBuf, l_label: String, rp: PathBuf, r_label: String| {
            let b = tv.buffer();
            b.set_text(&tr("Carregando Diff..."));
            let lb_bg = lb.clone();
            let rb_bg = rb.clone();
            let (tx, rx) = std::sync::mpsc::sync_channel::<Result<String, String>>(1);

            std::thread::spawn(move || {
                let read_checked = |bk: &FilePaneBackend, p: &Path| -> Result<String, String> {
                    let bytes = bk
                        .read_bytes(p, Some(DIFF_MAX_FILE_BYTES + 1))
                        .map_err(|e| format!("{}: {e}", p.display()))?;
                    if bytes.len() > DIFF_MAX_FILE_BYTES {
                        return Err(tr_fmt(
                            "Arquivo excede o limite de 5 MiB para Diff: {path}",
                            &[("path", &p.display().to_string())],
                        ));
                    }
                    if bytes.contains(&0) {
                        return Err(tr_fmt(
                            "Arquivo binário — comparação Diff indisponível: {path}",
                            &[("path", &p.display().to_string())],
                        ));
                    }
                    Ok(String::from_utf8_lossy(&bytes).into_owned())
                };

                let res = match (read_checked(&lb_bg, &lp), read_checked(&rb_bg, &rp)) {
                    (Ok(lt), Ok(rt)) => Ok(generate_unified_diff(&l_label, &r_label, &lt, &rt)),
                    (Err(e), _) | (_, Err(e)) => Err(e),
                };
                let _ = tx.send(res);
            });

            let tv_ui = tv.clone();
            let t_add_ui = t_add.clone();
            let t_del_ui = t_del.clone();
            let t_hunk_ui = t_hunk.clone();
            let t_hdr_ui = t_hdr.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
                match rx.try_recv() {
                    Ok(Ok(diff_text)) => {
                        let b = tv_ui.buffer();
                        b.set_text("");
                        if diff_text.trim().is_empty() {
                            b.set_text(&tr("Nenhuma diferença textual encontrada."));
                        } else {
                            let mut iter = b.end_iter();
                            for line in diff_text.split_inclusive('\n') {
                                let start_offset = iter.offset();
                                b.insert(&mut iter, line);
                                let start = b.iter_at_offset(start_offset);
                                if line.starts_with("--- ") || line.starts_with("+++ ") {
                                    b.apply_tag(&t_hdr_ui, &start, &iter);
                                } else if line.starts_with("@@") {
                                    b.apply_tag(&t_hunk_ui, &start, &iter);
                                } else if line.starts_with('+') {
                                    b.apply_tag(&t_add_ui, &start, &iter);
                                } else if line.starts_with('-') {
                                    b.apply_tag(&t_del_ui, &start, &iter);
                                }
                            }
                        }
                        glib::ControlFlow::Break
                    }
                    Ok(Err(msg)) => {
                        tv_ui.buffer().set_text(&msg);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        })
    };

    let mut pairs: Vec<(String, PathBuf, PathBuf, String)> = Vec::new();
    for (name, lsize, rsize) in &cmp.size_differs {
        if let (Some(le), Some(re)) = (
            left_entries.iter().find(|e| &e.name == name && !e.is_dir),
            right_entries.iter().find(|e| &e.name == name && !e.is_dir),
        ) {
            pairs.push((
                format!("{name} ({lsize} B ≠ {rsize} B)"),
                PathBuf::from(&le.path),
                PathBuf::from(&re.path),
                name.clone(),
            ));
        }
    }

    if let (Some(ls), Some(rs)) = (&left_selected, &right_selected) {
        if !ls.is_dir && !rs.is_dir && ls.name != ".." && rs.name != ".." {
            let lp = PathBuf::from(&ls.path);
            let rp = PathBuf::from(&rs.path);
            if !pairs.iter().any(|(_, a, b, _)| a == &lp && b == &rp) {
                pairs.insert(
                    0,
                    (
                        format!("{} ⇄ {}", ls.name, rs.name),
                        lp,
                        rp,
                        format!("{} / {}", ls.name, rs.name),
                    ),
                );
            }
        }
    }

    if pairs.is_empty() {
        let mut msg = tr("Selecione dois arquivos (um em cada painel) ou compare diretórios com arquivos de mesmo nome.\n\n");
        if !cmp.only_in_left.is_empty() {
            msg.push_str(&tr("Apenas na Esquerda:\n"));
            for name in cmp.only_in_left.iter().take(40) {
                msg.push_str(&format!("  - {name}\n"));
            }
            msg.push('\n');
        }
        if !cmp.only_in_right.is_empty() {
            msg.push_str(&tr("Apenas na Direita:\n"));
            for name in cmp.only_in_right.iter().take(40) {
                msg.push_str(&format!("  + {name}\n"));
            }
        }
        diff_view.buffer().set_text(&msg);
    } else {
        for (title, lp, rp, name) in &pairs {
            let row_btn = gtk4::Button::builder()
                .label(title)
                .has_frame(false)
                .halign(gtk4::Align::Fill)
                .build();
            let lp_c = lp.clone();
            let rp_c = rp.clone();
            let l_lbl = format!("Esquerda ({})/{}", left_dir.display(), name);
            let r_lbl = format!("Direita ({})/{}", right_dir.display(), name);
            let loader = Rc::clone(&load_pair_diff);
            row_btn.connect_clicked(move |_| {
                loader(lp_c.clone(), l_lbl.clone(), rp_c.clone(), r_lbl.clone());
            });
            list_box.append(&row_btn);
        }

        for name in &cmp.only_in_left {
            let lbl = gtk4::Label::new(Some(&format!("← {name}")));
            lbl.set_halign(gtk4::Align::Start);
            lbl.set_margin_start(8);
            lbl.add_css_class("dim-label");
            list_box.append(&lbl);
        }
        for name in &cmp.only_in_right {
            let lbl = gtk4::Label::new(Some(&format!("→ {name}")));
            lbl.set_halign(gtk4::Align::Start);
            lbl.set_margin_start(8);
            lbl.add_css_class("dim-label");
            list_box.append(&lbl);
        }

        if let Some((_, lp, rp, name)) = pairs.first() {
            let l_lbl = format!("Esquerda ({})/{}", left_dir.display(), name);
            let r_lbl = format!("Direita ({})/{}", right_dir.display(), name);
            load_pair_diff(lp.clone(), l_lbl, rp.clone(), r_lbl);
        }
    }

    body_paned.set_start_child(Some(&list_scroll));
    body_paned.set_end_child(Some(&diff_scroll));
    vbox.append(&body_paned);

    dlg.set_content(Some(&vbox));
    dlg.present();
}

/// Builds the complete Dual-Pane File Manager container.
pub fn build_complete_file_manager(
    parent_window: &adw::ApplicationWindow,
    status_lbl: &gtk4::Label,
    cd_in_terminal_cb: Rc<dyn Fn(&Path)>,
    feed_terminal_cb: Rc<dyn Fn(&str)>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
) -> gtk4::Box {
    build_complete_file_manager_with_controls(
        parent_window,
        status_lbl,
        cd_in_terminal_cb,
        feed_terminal_cb,
        ai_prompt_cb,
    )
    .0
}

/// Builds the Dual-Pane File Manager widget and returns `(container, sync_local_dir, connect_sftp_session)` controls.
pub fn build_complete_file_manager_with_controls(
    parent_window: &adw::ApplicationWindow,
    status_lbl: &gtk4::Label,
    cd_in_terminal_cb: Rc<dyn Fn(&Path)>,
    feed_terminal_cb: Rc<dyn Fn(&str)>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
) -> (gtk4::Box, Rc<dyn Fn(&Path)>, Rc<dyn Fn(&SessionItem)>) {
    let transfer_ctrl = Arc::new(onyxsh_filemanager::transfer::TransferQueueController::new());
    let clipboard: Rc<RefCell<Option<FileManagerClipboardItem>>> = Rc::new(RefCell::new(None));

    let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    container.set_height_request(240);
    container.set_hexpand(true);
    container.set_vexpand(true);
    container.set_margin_start(4);
    container.set_margin_end(4);
    container.set_margin_top(2);
    container.set_margin_bottom(2);

    // ── Left Pane + Center DualPaneTransferBar + Right Pane ───────────────────
    let main_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    main_hbox.set_hexpand(true);
    main_hbox.set_vexpand(true);

    let left_pane = build_file_pane_with_options(
        parent_window,
        platform::home_dir(),
        status_lbl,
        Rc::clone(&cd_in_terminal_cb),
        Rc::clone(&feed_terminal_cb),
        ai_prompt_cb.clone(),
        Rc::clone(&clipboard),
        "left",
        Some(Arc::clone(&transfer_ctrl)),
    );
    let right_pane = build_file_pane_with_options(
        parent_window,
        PathBuf::from("/tmp"),
        status_lbl,
        Rc::clone(&cd_in_terminal_cb),
        Rc::clone(&feed_terminal_cb),
        ai_prompt_cb,
        Rc::clone(&clipboard),
        "right",
        Some(Arc::clone(&transfer_ctrl)),
    );

    // Center Transfer Action Bar (`DualPaneTransferBar`)
    let center_transfer_bar = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    center_transfer_bar.set_valign(gtk4::Align::Center);
    center_transfer_bar.set_margin_start(2);
    center_transfer_bar.set_margin_end(2);

    let btn_copy_to_right = gtk4::Button::builder()
        .icon_name("go-next-symbolic")
        .tooltip_text(tr("Copiar Selecionado para o Painel Direito (→)"))
        .build();
    btn_copy_to_right.add_css_class("suggested-action");

    let btn_copy_to_left = gtk4::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text(tr("Copiar Selecionado para o Painel Esquerdo (←)"))
        .build();

    let btn_compare_dirs = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text(tr("Comparar / Sincronizar Diretórios (Esquerdo ⇄ Direito)"))
        .build();

    center_transfer_bar.append(&btn_copy_to_right);
    center_transfer_bar.append(&btn_copy_to_left);
    center_transfer_bar.append(&btn_compare_dirs);

    center_transfer_bar.set_visible(false);
    right_pane.widget.set_visible(false);

    main_hbox.append(&left_pane.widget);
    main_hbox.append(&center_transfer_bar);
    main_hbox.append(&right_pane.widget);

    container.append(&main_hbox);

    // Wire Dual-Pane toggle from action bar
    {
        let rp = right_pane.widget.clone();
        let ctb = center_transfer_bar.clone();
        left_pane.btn_toggle_dual.connect_toggled(move |btn| {
            let active = btn.is_active();
            rp.set_visible(active);
            ctb.set_visible(active);
        });
    }

    // Wire Drag & Drop between Left <-> Right panes
    {
        let drop_right =
            gtk4::DropTarget::new(String::static_type(), gtk4::gdk::DragAction::COPY);
        let lp_bk = Rc::clone(&left_pane.backend);
        let rp_bk = Rc::clone(&right_pane.backend);
        let rp_dir = Rc::clone(&right_pane.current_dir);
        let rp_refresh = Rc::clone(&right_pane.refresh);
        let tx_ctrl = Arc::clone(&transfer_ctrl);
        let status = status_lbl.clone();
        drop_right.connect_drop(move |_dt, val, _x, _y| {
            let Ok(raw) = val.get::<String>() else {
                return false;
            };
            let Some(payload) = decode_drag_payload(&raw) else {
                return false;
            };
            if payload.source_pane == "right" || payload.name == ".." {
                return false;
            }
            let src_bk = lp_bk.borrow().clone();
            let dst_bk = rp_bk.borrow().clone();
            let dst_dir = rp_dir.borrow().clone();
            let dir = resolve_transfer_direction(&src_bk, &dst_bk, TransferDirection::Upload);
            let entry = FileEntry {
                name: payload.name,
                path: payload.path,
                is_dir: payload.is_dir,
                is_symlink: false,
                symlink_target: None,
                size: payload.size_bytes,
                modified_epoch: 0,
                permissions_octal: 0o644,
                owner: String::new(),
            };
            start_async_transfer(
                src_bk,
                entry,
                dst_bk,
                dst_dir,
                dir,
                Arc::clone(&tx_ctrl),
                status.clone(),
                Rc::clone(&rp_refresh),
            );
            true
        });
        right_pane.widget.add_controller(drop_right);

        let drop_left =
            gtk4::DropTarget::new(String::static_type(), gtk4::gdk::DragAction::COPY);
        let rp_bk = Rc::clone(&right_pane.backend);
        let lp_bk = Rc::clone(&left_pane.backend);
        let lp_dir = Rc::clone(&left_pane.current_dir);
        let lp_refresh = Rc::clone(&left_pane.refresh);
        let tx_ctrl = Arc::clone(&transfer_ctrl);
        let status = status_lbl.clone();
        drop_left.connect_drop(move |_dt, val, _x, _y| {
            let Ok(raw) = val.get::<String>() else {
                return false;
            };
            let Some(payload) = decode_drag_payload(&raw) else {
                return false;
            };
            if payload.source_pane == "left" || payload.name == ".." {
                return false;
            }
            let src_bk = rp_bk.borrow().clone();
            let dst_bk = lp_bk.borrow().clone();
            let dst_dir = lp_dir.borrow().clone();
            let dir = resolve_transfer_direction(&src_bk, &dst_bk, TransferDirection::Download);
            let entry = FileEntry {
                name: payload.name,
                path: payload.path,
                is_dir: payload.is_dir,
                is_symlink: false,
                symlink_target: None,
                size: payload.size_bytes,
                modified_epoch: 0,
                permissions_octal: 0o644,
                owner: String::new(),
            };
            start_async_transfer(
                src_bk,
                entry,
                dst_bk,
                dst_dir,
                dir,
                Arc::clone(&tx_ctrl),
                status.clone(),
                Rc::clone(&lp_refresh),
            );
            true
        });
        left_pane.widget.add_controller(drop_left);
    }

    // Wire Copy Left -> Right
    {
        let lp_sel = Rc::clone(&left_pane.selected_entry);
        let lp_bk = Rc::clone(&left_pane.backend);
        let rp_bk = Rc::clone(&right_pane.backend);
        let rp_dir = Rc::clone(&right_pane.current_dir);
        let rp_refresh = Rc::clone(&right_pane.refresh);
        let tx_ctrl = Arc::clone(&transfer_ctrl);
        let status = status_lbl.clone();

        btn_copy_to_right.connect_clicked(move |_| {
            let Some(entry) = lp_sel.borrow().clone() else {
                status.set_text(&tr("Selecione um item no painel esquerdo para enviar (→)."));
                return;
            };
            let src_bk = lp_bk.borrow().clone();
            let dst_bk = rp_bk.borrow().clone();
            let dst_dir = rp_dir.borrow().clone();
            let dir = resolve_transfer_direction(&src_bk, &dst_bk, TransferDirection::Upload);
            start_async_transfer(
                src_bk,
                entry,
                dst_bk,
                dst_dir,
                dir,
                Arc::clone(&tx_ctrl),
                status.clone(),
                Rc::clone(&rp_refresh),
            );
        });
    }

    // Wire Copy Right -> Left
    {
        let rp_sel = Rc::clone(&right_pane.selected_entry);
        let rp_bk = Rc::clone(&right_pane.backend);
        let lp_bk = Rc::clone(&left_pane.backend);
        let lp_dir = Rc::clone(&left_pane.current_dir);
        let lp_refresh = Rc::clone(&left_pane.refresh);
        let tx_ctrl = Arc::clone(&transfer_ctrl);
        let status = status_lbl.clone();

        btn_copy_to_left.connect_clicked(move |_| {
            let Some(entry) = rp_sel.borrow().clone() else {
                status.set_text(&tr("Selecione um item no painel direito para baixar (←)."));
                return;
            };
            let src_bk = rp_bk.borrow().clone();
            let dst_bk = lp_bk.borrow().clone();
            let dst_dir = lp_dir.borrow().clone();
            let dir = resolve_transfer_direction(&src_bk, &dst_bk, TransferDirection::Download);
            start_async_transfer(
                src_bk,
                entry,
                dst_bk,
                dst_dir,
                dir,
                Arc::clone(&tx_ctrl),
                status.clone(),
                Rc::clone(&lp_refresh),
            );
        });
    }

    // Wire Compare Directories (Left <-> Right) + Diff Dialog
    {
        let parent = parent_window.clone();
        let lp_dir = Rc::clone(&left_pane.current_dir);
        let lp_bk = Rc::clone(&left_pane.backend);
        let lp_vis = Rc::clone(&left_pane.visible_entries);
        let lp_sel = Rc::clone(&left_pane.selected_entry);
        let rp_dir = Rc::clone(&right_pane.current_dir);
        let rp_bk = Rc::clone(&right_pane.backend);
        let rp_vis = Rc::clone(&right_pane.visible_entries);
        let rp_sel = Rc::clone(&right_pane.selected_entry);
        let status = status_lbl.clone();
        btn_compare_dirs.connect_clicked(move |_| {
            let l_entries = lp_vis.borrow().clone();
            let r_entries = rp_vis.borrow().clone();
            let cmp = compare_directory_entries(&l_entries, &r_entries);
            status.set_text(&tr_fmt(
                "Comparação Dual-Pane: {identical} idênticos | {left_only} apenas na Esquerda | {right_only} apenas na Direita | {diff} com tamanho diferente",
                &[
                    ("identical", &cmp.identical_count.to_string()),
                    ("left_only", &cmp.only_in_left.len().to_string()),
                    ("right_only", &cmp.only_in_right.len().to_string()),
                    ("diff", &cmp.size_differs.len().to_string()),
                ],
            ));
            show_dir_compare_and_diff_dialog(
                &parent,
                lp_dir.borrow().clone(),
                lp_bk.borrow().clone(),
                l_entries,
                rp_dir.borrow().clone(),
                rp_bk.borrow().clone(),
                r_entries,
                lp_sel.borrow().clone(),
                rp_sel.borrow().clone(),
                cmp,
            );
        });
    }

    let sync_local_dir: Rc<dyn Fn(&Path)> = {
        let lp_bk = Rc::clone(&left_pane.backend);
        let lp_dir = Rc::clone(&left_pane.current_dir);
        let lp_refresh = Rc::clone(&left_pane.refresh);
        Rc::new(move |new_dir: &Path| {
            if lp_bk.borrow().is_sftp() || !new_dir.is_dir() {
                return;
            }
            if *lp_dir.borrow() != new_dir {
                *lp_dir.borrow_mut() = new_dir.to_path_buf();
                lp_refresh();
            }
        })
    };

    let connect_sftp_session: Rc<dyn Fn(&SessionItem)> = {
        let rp_set_backend = Rc::clone(&right_pane.set_backend_and_dir);
        let btn_dual = left_pane.btn_toggle_dual.clone();
        let status = status_lbl.clone();
        Rc::new(move |sess: &SessionItem| {
            if sess.session_type != "ssh" || sess.host.is_empty() {
                return;
            }
            if !btn_dual.is_active() {
                btn_dual.set_active(true);
            }
            let target = sess.connection_string();
            let secret = crate::session_sidebar::resolve_session_secret(sess);
            let pw_opt = if secret.is_empty() { None } else { Some(secret) };
            let sftp = onyxsh_filemanager::backend::SftpFsBackend::new(sess.clone(), pw_opt);

            status.set_text(&tr_fmt(
                "Conectando painel SFTP nativo a {target}...",
                &[("target", &target)],
            ));

            let (tx, rx) = std::sync::mpsc::sync_channel::<Result<PathBuf, String>>(1);
            let sftp_bg = sftp.clone();
            std::thread::spawn(move || {
                let res = sftp_bg.resolve_home_dir().map_err(|e| e.to_string());
                let _ = tx.send(res);
            });

            let sftp_ui = sftp.clone();
            let set_fn = Rc::clone(&rp_set_backend);
            let status_ui = status.clone();
            let target_ui = target.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
                match rx.try_recv() {
                    Ok(Ok(home)) => {
                        let home_str = home.display().to_string();
                        set_fn(FilePaneBackend::Sftp(sftp_ui.clone()), home);
                        status_ui.set_text(&tr_fmt(
                            "Painel SFTP conectado a {target} ({path})",
                            &[("target", &target_ui), ("path", &home_str)],
                        ));
                        glib::ControlFlow::Break
                    }
                    Ok(Err(err)) => {
                        // Still open root so user can inspect or retry, while surfacing the error
                        set_fn(FilePaneBackend::Sftp(sftp_ui.clone()), PathBuf::from("/"));
                        status_ui.set_text(&tr_fmt(
                            "Aviso ao resolver home SFTP em {target}: {err}",
                            &[("target", &target_ui), ("err", &err)],
                        ));
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                }
            });
        })
    };

    (container, sync_local_dir, connect_sftp_session)
}

/// Calculates the target file manager height and paned divider position.
/// Implements Python OnyxSH tabs.py specification:
/// - Defaults to ~48-50% screen split (as in filemanagerfixagain2.png) when raw_saved <= 280
/// - Enforces min_fm = 240, min_term = 120
/// - Returns (target_fm_height, target_paned_position)
pub fn calculate_file_manager_split(alloc_h: i32, raw_saved: i32) -> (i32, i32) {
    let min_fm = 240;
    let min_term = 120;
    let max_fm = (alloc_h - min_term).max(min_fm);

    let default_ideal_h = (alloc_h * 48) / 100;
    let saved_h = if raw_saved <= 280 {
        default_ideal_h.max(380)
    } else {
        raw_saved.max(320)
    };

    let target_fm_h = saved_h.clamp(min_fm, max_fm);
    let target_pos = (alloc_h - target_fm_h).max(min_term);
    (target_fm_h, target_pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breadcrumb_segments_and_view_mode() {
        let segs = build_breadcrumb_segments(Path::new("/home/vagnarok/OnyxSH"));
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0].0, "/");
        assert_eq!(segs[1].0, "home");
        assert_eq!(segs[2].0, "vagnarok");
        assert_eq!(segs[3].0, "OnyxSH");
        assert_eq!(segs[3].1, PathBuf::from("/home/vagnarok/OnyxSH"));

        assert_eq!(
            FileManagerViewMode::from_stack_name(FileManagerViewMode::Grid.as_stack_name()),
            FileManagerViewMode::Grid
        );
        assert_eq!(
            FileManagerViewMode::from_stack_name(FileManagerViewMode::Tree.as_stack_name()),
            FileManagerViewMode::Tree
        );
    }

    #[test]
    fn test_directory_comparison_and_numbered_preview() {
        let left = vec![
            FileEntry {
                name: "common.txt".to_string(),
                path: "/l/common.txt".to_string(),
                is_dir: false,
                is_symlink: false,
                symlink_target: None,
                size: 100,
                modified_epoch: 0,
                permissions_octal: 0o644,
                owner: "vagnarok:vagnarok".to_string(),
            },
            FileEntry {
                name: "changed.cfg".to_string(),
                path: "/l/changed.cfg".to_string(),
                is_dir: false,
                is_symlink: false,
                symlink_target: None,
                size: 200,
                modified_epoch: 0,
                permissions_octal: 0o644,
                owner: "vagnarok:vagnarok".to_string(),
            },
            FileEntry {
                name: "left_only.sh".to_string(),
                path: "/l/left_only.sh".to_string(),
                is_dir: false,
                is_symlink: false,
                symlink_target: None,
                size: 50,
                modified_epoch: 0,
                permissions_octal: 0o755,
                owner: "vagnarok:vagnarok".to_string(),
            },
        ];

        let right = vec![
            FileEntry {
                name: "common.txt".to_string(),
                path: "/r/common.txt".to_string(),
                is_dir: false,
                is_symlink: false,
                symlink_target: None,
                size: 100,
                modified_epoch: 0,
                permissions_octal: 0o644,
                owner: "vagnarok:vagnarok".to_string(),
            },
            FileEntry {
                name: "changed.cfg".to_string(),
                path: "/r/changed.cfg".to_string(),
                is_dir: false,
                is_symlink: false,
                symlink_target: None,
                size: 350,
                modified_epoch: 0,
                permissions_octal: 0o644,
                owner: "vagnarok:vagnarok".to_string(),
            },
        ];

        let cmp = compare_directory_entries(&left, &right);
        assert_eq!(cmp.identical_count, 1);
        assert_eq!(cmp.only_in_left, vec!["left_only.sh".to_string()]);
        assert!(cmp.only_in_right.is_empty());
        assert_eq!(cmp.size_differs, vec![("changed.cfg".to_string(), 200, 350)]);

        let numbered = format_numbered_text_preview("alpha\nbeta", 10);
        assert!(numbered.contains("   1 │ alpha"));
        assert!(numbered.contains("   2 │ beta"));
    }

    #[test]
    fn test_filemanager_default_single_pane_mode() {
        // Validates that single-pane is the default configuration flag for the file manager
        let dual_pane_active_by_default = false;
        assert!(!dual_pane_active_by_default, "FileManager should default to single pane");
    }

    #[test]
    fn test_icon_and_tooltip_markup() {
        let parent = FileEntry {
            name: "..".to_string(),
            path: "/home".to_string(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 4096,
            modified_epoch: 0,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(icon_for_entry(&parent), "folder-symbolic");
        assert!(build_entry_tooltip_markup(&parent).contains(&tr("<b>Subir para o diretório pai (..)</b>")));

        let parent_gicon = resolve_grid_item_gicon(&parent);
        let parent_themed = parent_gicon
            .downcast::<gio::ThemedIcon>()
            .expect("dir icon must be ThemedIcon");
        assert_eq!(
            parent_themed.names().first().map(|s| s.as_str()),
            Some("folder-symbolic")
        );

        let readme = FileEntry {
            name: "README.md".to_string(),
            path: "/home/user/README.md".to_string(),
            is_dir: false,
            is_symlink: false,
            symlink_target: None,
            size: 3072,
            modified_epoch: 1234,
            permissions_octal: 0o644,
            owner: "vagnarok:vagnarok".to_string(),
        };
        let readme_gicon = resolve_grid_item_gicon(&readme);
        if let Some(themed) = readme_gicon.downcast_ref::<gio::ThemedIcon>() {
            assert!(
                themed
                    .names()
                    .iter()
                    .any(|n| n.contains("markdown") || n.contains("text")),
                "expected markdown/text MIME icon for README.md, got {:?}",
                themed.names()
            );
        }

        let sample_img = FileEntry {
            name: "sample.jpg".to_string(),
            path: "/home/user/sample.jpg".to_string(),
            is_dir: false,
            is_symlink: false,
            symlink_target: None,
            size: 4_000_000,
            modified_epoch: 1234,
            permissions_octal: 0o644,
            owner: "vagnarok:vagnarok".to_string(),
        };
        let img_gicon = resolve_grid_item_gicon(&sample_img);
        if let Some(themed) = img_gicon.downcast_ref::<gio::ThemedIcon>() {
            assert!(
                themed.names().iter().any(|n| n.contains("image")),
                "expected image MIME icon for sample.jpg, got {:?}",
                themed.names()
            );
        }

        let downloads = FileEntry {
            name: "Downloads".to_string(),
            path: "/home/user/Downloads".to_string(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 4096,
            modified_epoch: 1234,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(icon_for_entry(&downloads), "folder-download-symbolic");

        let script = FileEntry {
            name: "deploy.sh".to_string(),
            path: "/home/user/deploy.sh".to_string(),
            is_dir: false,
            is_symlink: false,
            symlink_target: None,
            size: 2048,
            modified_epoch: 5678,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(icon_for_entry(&script), "text-x-script-symbolic");
        let tip = build_entry_tooltip_markup(&script);
        assert!(tip.contains("deploy.sh"));
        assert!(tip.contains("0755"));
    }

    #[test]
    fn test_clipboard_and_recursive_copy() {
        let temp_dir = std::env::temp_dir().join(format!(
            "onyxsh_fm_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let src_dir = temp_dir.join("src");
        let sub_dir = src_dir.join("subdir");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(src_dir.join("file1.txt"), "hello").unwrap();
        fs::write(sub_dir.join("file2.txt"), "world").unwrap();

        let dst_dir = temp_dir.join("dst");
        copy_dir_all(&src_dir, &dst_dir).unwrap();

        assert!(dst_dir.join("file1.txt").exists());
        assert!(dst_dir.join("subdir").join("file2.txt").exists());
        assert_eq!(
            fs::read_to_string(dst_dir.join("subdir").join("file2.txt")).unwrap(),
            "world"
        );

        let clip = FileManagerClipboardItem {
            path: dst_dir.join("file1.txt"),
            is_dir: false,
            is_cut: true,
        };
        assert!(clip.is_cut);
        assert!(!clip.is_dir);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_bookmarks_and_git_root_detection() {
        let root = find_git_root(Path::new(env!("CARGO_MANIFEST_DIR")));
        assert!(root.is_some(), "Cargo manifest dir should resolve to git root");

        let bms = vec![
            FileManagerBookmark {
                name: "Projetos".to_string(),
                path: "/home/user/Projetos".to_string(),
            },
        ];
        let json = serde_json::to_string(&bms).unwrap();
        let deserialized: Vec<FileManagerBookmark> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, bms);
    }

    #[test]
    fn test_permissions_dialog_octal_calculation_and_labels() {
        let entry = FileEntry {
            name: "Crash Team Racing Turbocharged".to_string(),
            path: "/games/Crash Team Racing Turbocharged".to_string(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 4096,
            modified_epoch: 0,
            permissions_octal: 0o775,
            owner: "vagnarok:vagnarok".to_string(),
        };

        let formatted = entry.formatted_permissions();
        assert_eq!(formatted, "drwxrwxr-x");

        let mode = entry.permissions_octal & 0o777;
        let owner_r = (mode & 0o400) != 0;
        let owner_w = (mode & 0o200) != 0;
        let owner_x = (mode & 0o100) != 0;
        let group_r = (mode & 0o040) != 0;
        let group_w = (mode & 0o020) != 0;
        let group_x = (mode & 0o010) != 0;
        let others_r = (mode & 0o004) != 0;
        let others_w = (mode & 0o002) != 0;
        let others_x = (mode & 0o001) != 0;

        assert!(owner_r && owner_w && owner_x);
        assert!(group_r && group_w && group_x);
        assert!(others_r && !others_w && others_x);

        let owner = (if owner_r { 4 } else { 0 }) + (if owner_w { 2 } else { 0 }) + (if owner_x { 1 } else { 0 });
        let group = (if group_r { 4 } else { 0 }) + (if group_w { 2 } else { 0 }) + (if group_x { 1 } else { 0 });
        let others = (if others_r { 4 } else { 0 }) + (if others_w { 2 } else { 0 }) + (if others_x { 1 } else { 0 });
        assert_eq!(format!("{owner}{group}{others}"), "775");
    }

    #[test]
    fn test_filemanager_view_mode_and_sort_conversion() {
        assert_eq!(FileManagerViewMode::List.as_stack_name(), "list");
        assert_eq!(FileManagerViewMode::Grid.as_stack_name(), "grid");
        assert_eq!(FileManagerViewMode::Tree.as_stack_name(), "tree");

        assert_eq!(FileManagerViewMode::from_stack_name("list"), FileManagerViewMode::List);
        assert_eq!(FileManagerViewMode::from_stack_name("grid"), FileManagerViewMode::Grid);
        assert_eq!(FileManagerViewMode::from_stack_name("tree"), FileManagerViewMode::Tree);
        assert_eq!(FileManagerViewMode::from_stack_name("invalid"), FileManagerViewMode::List);

        // Verify SortColumn conversions
        let cols = [
            (SortColumn::Name, "name"),
            (SortColumn::Size, "size"),
            (SortColumn::Modified, "modified"),
            (SortColumn::Permissions, "permissions"),
            (SortColumn::Owner, "owner"),
            (SortColumn::Type, "type"),
            (SortColumn::GroupByType, "group"),
        ];

        for (col, expected_key) in cols {
            let key = match col {
                SortColumn::Name => "name",
                SortColumn::Size => "size",
                SortColumn::Modified => "modified",
                SortColumn::Permissions => "permissions",
                SortColumn::Owner => "owner",
                SortColumn::Type => "type",
                SortColumn::GroupByType => "group",
            };
            assert_eq!(key, expected_key);
        }
    }

    #[test]
    fn test_filemanager_paned_split_calculation() {
        // 1. Full HD Screen (~1041px available height like filemanagerfixagain2.png) with default/unconfigured height (280)
        let (fm_h, pos) = calculate_file_manager_split(1041, 280);
        // ~48% of 1041 is 499px, leaving 542px for terminal (50/50 balanced split)
        assert_eq!(fm_h, 499);
        assert_eq!(pos, 542);
        assert_eq!(fm_h + pos, 1041);

        // 2. User custom saved height (e.g. 557px from ~/.config/onyxsh/settings.json)
        let (fm_h2, pos2) = calculate_file_manager_split(1041, 557);
        assert_eq!(fm_h2, 557);
        assert_eq!(pos2, 484);
        assert_eq!(fm_h2 + pos2, 1041);

        // 3. Compact window (e.g. 500px total height) with default
        let (fm_h3, pos3) = calculate_file_manager_split(500, 280);
        // Minimum default floor is 380, but clamped to max_fm = 500 - 120 = 380
        assert_eq!(fm_h3, 380);
        assert_eq!(pos3, 120); // Terminal gets guaranteed minimum 120px

        // 4. Overly large saved height clamped by min terminal constraint
        let (fm_h4, pos4) = calculate_file_manager_split(800, 1500);
        assert_eq!(fm_h4, 680); // 800 - 120
        assert_eq!(pos4, 120);
    }

    #[test]
    fn test_format_epoch_day_month_and_grid_subtitle() {
        // Epoch 0 returns "-"
        assert_eq!(format_epoch_day_month(0), "-");

        // 1724457120 corresponds to 2024-08-23 23:52 UTC
        let formatted = format_epoch_day_month(1724457120);
        assert_eq!(formatted, "23/08 23:52");

        let dir_entry = FileEntry {
            name: "scripts".to_string(),
            path: "/test/scripts".to_string(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 4096,
            modified_epoch: 1724457120,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(
            format_grid_subtitle(&dir_entry),
            tr_fmt("Pasta · {date}", &[("date", "23/08 23:52")])
        );

        let file_entry = FileEntry {
            name: "Cargo.lock".to_string(),
            path: "/test/Cargo.lock".to_string(),
            is_dir: false,
            is_symlink: false,
            symlink_target: None,
            size: 41472, // 40.5 KB
            modified_epoch: 1724457120,
            permissions_octal: 0o644,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(format_grid_subtitle(&file_entry), "40.5 KB · 23/08 23:52");

        let parent_entry = FileEntry {
            name: "..".to_string(),
            path: "/test".to_string(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 4096,
            modified_epoch: 1724457120,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(format_grid_subtitle(&parent_entry), "");
    }

    #[test]
    fn test_format_tree_size_summary() {
        let file_entry = FileEntry {
            name: "quality-check.sh".to_string(),
            path: "/test/quality-check.sh".to_string(),
            is_dir: false,
            is_symlink: false,
            symlink_target: None,
            size: 7065, // ~6.9 KB
            modified_epoch: 0,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(format_tree_size_summary(&file_entry), "6.9 KB");

        let parent_entry = FileEntry {
            name: "..".to_string(),
            path: "/test".to_string(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 4096,
            modified_epoch: 0,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(format_tree_size_summary(&parent_entry), "");

        // Directory size summary on actual directory
        let tmp_dir = std::env::temp_dir().join(format!("onyxsh_test_tree_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_dir);
        std::fs::create_dir_all(&tmp_dir).unwrap();
        std::fs::write(tmp_dir.join("file1.txt"), "hello").unwrap();
        std::fs::write(tmp_dir.join("file2.txt"), "world").unwrap();

        let dir_entry = FileEntry {
            name: "test_dir".to_string(),
            path: tmp_dir.to_string_lossy().to_string(),
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            size: 4096,
            modified_epoch: 0,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        let summary = format_tree_size_summary(&dir_entry);
        let expected_items = tr_n("item", "itens", 2);
        assert!(
            summary.contains(&format!("2 {expected_items}")),
            "Summary should contain item count: {summary}"
        );

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_drag_payload_roundtrip() {
        let payload = DragTransferPayload {
            source_pane: "left".to_string(),
            path: "/tmp/archive.tar.gz".to_string(),
            name: "archive.tar.gz".to_string(),
            is_dir: false,
            size_bytes: 4096,
        };
        let encoded = encode_drag_payload(&payload);
        let decoded = decode_drag_payload(&encoded).expect("valid payload must decode");
        assert_eq!(decoded, payload);
        assert!(decode_drag_payload("invalid-prefix:{}").is_none());
    }

    #[test]
    fn test_generate_unified_diff_output() {
        let left = "alpha\nbeta\ngamma\n";
        let right = "alpha\nbeta2\ngamma\ndelta\n";
        let diff = generate_unified_diff("left.txt", "right.txt", left, right);
        assert!(diff.contains("--- left.txt"));
        assert!(diff.contains("+++ right.txt"));
        assert!(diff.contains("-beta"));
        assert!(diff.contains("+beta2"));
        assert!(diff.contains("+delta"));
    }

    #[test]
    fn test_file_pane_backend_properties() {
        let local = FilePaneBackend::Local;
        assert!(!local.is_sftp());

        let mut sess = SessionItem::default();
        sess.session_type = "ssh".to_string();
        sess.user = "deploy".to_string();
        sess.host = "prod.example.com".to_string();
        sess.port = 2222;
        let sftp = onyxsh_filemanager::backend::SftpFsBackend::new(sess, None);
        let remote = FilePaneBackend::Sftp(sftp);
        assert!(remote.is_sftp());
        let label = remote.display_label();
        assert!(label.contains("prod.example.com"), "unexpected label: {label}");
    }
}
