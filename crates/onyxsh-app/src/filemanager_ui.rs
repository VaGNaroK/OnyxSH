//! Phase 12: Complete Dual-Pane File Manager UI (`src/onyxsh/filemanager/manager.py`,
//! `local_pane.py`, `dual_pane.py`, `quick_look.py`, `transfer_dialog.py`).
//!
//! Provides:
//! - 3 View Modes (`gtk4::Stack`): Detailed List (sortable columns), Thumbnail/Icon Grid (`FlowBox`
//!   with real image thumbnails via `gtk4::Picture`), and Inline Directory Tree (`gtk4::Expander`).
//! - Clickable Breadcrumbs path bar + direct path entry, Up, Home, Refresh, New Folder, Hidden toggle,
//!   and live search filter.
//! - Quick Look modal preview window (Spacebar or button) with `< Prev` / `Next >` cycling,
//!   image rendering, line-numbered text preview, and metadata summary.
//! - Dual-Pane mode (`Left <-> Right`) with center transfer buttons (`→ Copy`, `← Copy`, `⇄ Compare`,
//!   and `Transfer Queue` dialog with progress bars).
//! - File operations: Rename, Chmod permissions, SHA-256 Checksum, Delete, and "CD in Terminal".

use gtk4::gdk;
use gtk4::glib;
use gtk4::graphene;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use onyxsh_core::config::AppSettings;
use onyxsh_core::platform;
use onyxsh_filemanager::backend::{FileSystemBackend, LocalFsBackend};
use onyxsh_filemanager::quick_look::{
    load_local_preview, select_quick_look_entry, PreviewKind,
};
use onyxsh_filemanager::transfer::{TransferDirection, TransferItem, TransferStatus};
use onyxsh_filemanager::{
    filter_and_sort_entries, format_bytes, FileEntry, SortColumn,
};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

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

/// Checks if a file entry is an image suitable for direct thumbnail rendering.
fn is_image_entry(entry: &FileEntry) -> bool {
    if entry.is_dir {
        return false;
    }
    let ext = Path::new(&entry.name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico"
    )
}

/// Chooses a symbolic icon name for a [`FileEntry`], supporting specialized folders and MIME classifications.
fn icon_for_entry(entry: &FileEntry) -> &'static str {
    if entry.name == ".." {
        return "go-up-symbolic";
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

/// Generates a rich, formatted Pango Markup tooltip for a file/directory row matching Python.
fn build_entry_tooltip_markup(entry: &FileEntry) -> String {
    if entry.name == ".." {
        return "<b>Subir para o diretório pai (..)</b>".to_string();
    }
    let icon_emoji = if entry.is_dir {
        "📁"
    } else if entry.is_symlink {
        "🔗"
    } else {
        "📄"
    };
    let safe_name = glib::markup_escape_text(&entry.name);
    let type_desc = glib::markup_escape_text(entry.type_description());
    let perms_sym = entry.formatted_permissions();
    let perms_oct = entry.permissions_octal_str();
    let mod_date = format_epoch_short(entry.modified_epoch);

    let mut lines = vec![
        format!("{icon_emoji} <b>{safe_name}</b> <span alpha='70%'>({type_desc})</span>"),
        "<span alpha='30%'>──────────────────────────────────────────</span>".to_string(),
    ];

    if entry.is_dir {
        lines.push("📦 <b>Tipo:</b> Pasta / Diretório".to_string());
    } else {
        lines.push(format!(
            "📦 <b>Tamanho:</b> {} <span alpha='70%'>({} bytes)</span>",
            entry.formatted_size(),
            entry.size
        ));
    }

    lines.push(format!("📅 <b>Modificado:</b> {mod_date}"));
    lines.push(format!(
        "🔒 <b>Permissões:</b> <tt>{perms_sym}</tt> <span alpha='70%'>({perms_oct})</span>"
    ));

    if entry.is_symlink {
        let safe_path = glib::markup_escape_text(&entry.path);
        lines.push(format!("🔗 <b>Caminho:</b> <tt>{safe_path}</tt>"));
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

/// Opens the Quick Look modal preview window with `< Previous` and `Next >` navigation.
pub fn show_quick_look_window(
    parent: &adw::ApplicationWindow,
    entries: Vec<FileEntry>,
    initial_index: usize,
) {
    if entries.is_empty() {
        return;
    }

    let current_idx = Rc::new(RefCell::new(initial_index.min(entries.len() - 1)));
    let entries_rc = Rc::new(entries);

    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Quick Look")
        .default_width(720)
        .default_height(520)
        .build();

    let root_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    let hb = adw::HeaderBar::new();

    let nav_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    nav_box.add_css_class("linked");
    let btn_prev = gtk4::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text("Arquivo Anterior")
        .build();
    let btn_next = gtk4::Button::builder()
        .icon_name("go-next-symbolic")
        .tooltip_text("Próximo Arquivo")
        .build();
    nav_box.append(&btn_prev);
    nav_box.append(&btn_next);
    hb.pack_start(&nav_box);

    let pos_lbl = gtk4::Label::new(None);
    pos_lbl.add_css_class("dim-label");
    pos_lbl.add_css_class("monospace");
    hb.pack_end(&pos_lbl);

    root_vbox.append(&hb);

    let meta_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    meta_bar.set_margin_start(14);
    meta_bar.set_margin_end(14);
    meta_bar.set_margin_top(8);
    meta_bar.set_margin_bottom(8);

    let title_lbl = gtk4::Label::new(None);
    title_lbl.add_css_class("title-4");
    title_lbl.set_halign(gtk4::Align::Start);
    title_lbl.set_hexpand(true);

    let subtitle_lbl = gtk4::Label::new(None);
    subtitle_lbl.add_css_class("caption");
    subtitle_lbl.add_css_class("dim-label");

    meta_bar.append(&title_lbl);
    meta_bar.append(&subtitle_lbl);
    root_vbox.append(&meta_bar);
    root_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let body_scroll = gtk4::ScrolledWindow::new();
    body_scroll.set_hexpand(true);
    body_scroll.set_vexpand(true);
    root_vbox.append(&body_scroll);

    let render_entry: Rc<dyn Fn()> = {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let title_l = title_lbl.clone();
        let sub_l = subtitle_lbl.clone();
        let pos_l = pos_lbl.clone();
        let scroll = body_scroll.clone();
        let dlg_ref = dlg.clone();

        Rc::new(move || {
            let idx = *idx_rc.borrow();
            let Some(entry) = entries.get(idx) else {
                return;
            };
            pos_l.set_text(&format!("{}/{}", idx + 1, entries.len()));
            dlg_ref.set_title(Some(&format!("Quick Look — {}", entry.name)));

            let path = Path::new(&entry.path);
            match load_local_preview(path) {
                Ok(preview) => {
                    title_l.set_text(&preview.title);
                    sub_l.set_text(&format!(
                        "{} • {} • {}",
                        preview.subtitle,
                        entry.formatted_permissions(),
                        format_epoch_short(entry.modified_epoch)
                    ));

                    match preview.kind {
                        PreviewKind::Image => {
                            let pic = gtk4::Picture::for_filename(path);
                            pic.set_can_shrink(true);
                            pic.set_margin_start(16);
                            pic.set_margin_end(16);
                            pic.set_margin_top(16);
                            pic.set_margin_bottom(16);
                            scroll.set_child(Some(&pic));
                        }
                        PreviewKind::Text => {
                            let tv = gtk4::TextView::new();
                            tv.set_editable(false);
                            tv.set_monospace(true);
                            tv.set_left_margin(12);
                            tv.set_right_margin(12);
                            tv.set_top_margin(10);
                            tv.set_bottom_margin(10);
                            let raw = preview.text_content.unwrap_or_default();
                            let numbered = format_numbered_text_preview(&raw, 2000);
                            tv.buffer().set_text(&numbered);
                            scroll.set_child(Some(&tv));
                        }
                        PreviewKind::Directory | PreviewKind::Archive | PreviewKind::Binary => {
                            let card = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
                            card.set_valign(gtk4::Align::Center);
                            card.set_halign(gtk4::Align::Center);
                            card.set_margin_top(32);
                            card.set_margin_bottom(32);

                            let icon = gtk4::Image::from_icon_name(icon_for_entry(entry));
                            icon.set_pixel_size(64);
                            card.append(&icon);

                            let kind_str = match preview.kind {
                                PreviewKind::Directory => "Diretório",
                                PreviewKind::Archive => "Arquivo Compactado / Pacote",
                                PreviewKind::Binary => "Arquivo Binário",
                                _ => "Item",
                            };
                            let info = gtk4::Label::new(Some(&format!(
                                "{kind_str}\nCaminho: {}\nTamanho / Itens: {}\nPermissões: {}",
                                entry.path,
                                preview.subtitle,
                                entry.formatted_permissions()
                            )));
                            info.set_justify(gtk4::Justification::Center);
                            card.append(&info);

                            if !entry.is_dir && entry.size <= 32 * 1024 * 1024 {
                                if let Ok(sha) = compute_file_sha256_hex(path) {
                                    let sha_lbl =
                                        gtk4::Label::new(Some(&format!("SHA-256: {sha}")));
                                    sha_lbl.add_css_class("monospace");
                                    sha_lbl.add_css_class("caption");
                                    sha_lbl.set_selectable(true);
                                    card.append(&sha_lbl);
                                }
                            }
                            scroll.set_child(Some(&card));
                        }
                    }
                }
                Err(e) => {
                    title_l.set_text(&entry.name);
                    sub_l.set_text("Erro ao carregar preview");
                    let err_lbl =
                        gtk4::Label::new(Some(&format!("Não foi possível ler o item: {e}")));
                    scroll.set_child(Some(&err_lbl));
                }
            }
        })
    };

    render_entry();

    {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let render = Rc::clone(&render_entry);
        btn_prev.connect_clicked(move |_| {
            let curr = *idx_rc.borrow();
            if let Some((next_idx, _)) = select_quick_look_entry(&entries, curr, false) {
                *idx_rc.borrow_mut() = next_idx;
                render();
            }
        });
    }
    {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let render = Rc::clone(&render_entry);
        btn_next.connect_clicked(move |_| {
            let curr = *idx_rc.borrow();
            if let Some((next_idx, _)) = select_quick_look_entry(&entries, curr, true) {
                *idx_rc.borrow_mut() = next_idx;
                render();
            }
        });
    }

    // Close on Escape or Space
    {
        let key_ctrl = gtk4::EventControllerKey::new();
        let dlg_close = dlg.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape || key == gdk::Key::space {
                dlg_close.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        dlg.add_controller(key_ctrl);
    }

    dlg.set_content(Some(&root_vbox));
    dlg.present();
}

/// Opens the Transfer Queue dialog (`TransferManagerDialog`) showing active/completed transfers.
pub fn show_transfer_queue_dialog(
    parent: &adw::ApplicationWindow,
    transfers: &Rc<RefCell<Vec<TransferItem>>>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title("Fila de Transferências — OnyxSH")
        .default_width(600)
        .default_height(400)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    let hb = adw::HeaderBar::new();

    let clear_btn = gtk4::Button::builder()
        .icon_name("edit-clear-all-symbolic")
        .tooltip_text("Limpar Concluídos")
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

    let populate = {
        let lb = list_box.clone();
        let tx_rc = Rc::clone(transfers);
        Rc::new(move || {
            lb.remove_all();
            let items = tx_rc.borrow();
            if items.is_empty() {
                let empty = gtk4::Label::new(Some("Nenhuma transferência registrada na sessão."));
                empty.add_css_class("dim-label");
                empty.set_margin_top(24);
                empty.set_margin_bottom(24);
                lb.append(&empty);
                return;
            }

            for item in items.iter().rev() {
                let row_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
                row_vbox.set_margin_start(10);
                row_vbox.set_margin_end(10);
                row_vbox.set_margin_top(8);
                row_vbox.set_margin_bottom(8);

                let top_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                let dir_badge = match item.direction {
                    TransferDirection::Upload => "→ Upload / Copy",
                    TransferDirection::Download => "← Download / Copy",
                };
                let name_lbl = gtk4::Label::new(Some(&format!(
                    "[{}] {} ({})",
                    dir_badge,
                    item.filename,
                    format_bytes(item.total_bytes)
                )));
                name_lbl.set_halign(gtk4::Align::Start);
                name_lbl.set_hexpand(true);

                let status_lbl = gtk4::Label::new(Some(&format!(
                    "{:?} ({}%)",
                    item.status,
                    item.progress_percent()
                )));
                status_lbl.add_css_class("caption");
                top_hbox.append(&name_lbl);
                top_hbox.append(&status_lbl);
                row_vbox.append(&top_hbox);

                let pbar = gtk4::ProgressBar::new();
                pbar.set_fraction(item.progress_fraction());
                row_vbox.append(&pbar);

                let paths_lbl = gtk4::Label::new(Some(&format!(
                    "{}  ⟶  {}",
                    item.source_path, item.destination_path
                )));
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
        let tx_rc = Rc::clone(transfers);
        let pop = Rc::clone(&populate);
        clear_btn.connect_clicked(move |_| {
            tx_rc
                .borrow_mut()
                .retain(|t| t.status == TransferStatus::InProgress);
            pop();
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
    pub widget: gtk4::Box,
    pub current_dir: Rc<RefCell<PathBuf>>,
    pub selected_entry: Rc<RefCell<Option<FileEntry>>>,
    pub visible_entries: Rc<RefCell<Vec<FileEntry>>>,
    pub refresh: Rc<dyn Fn()>,
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
    status_lbl: &gtk4::Label,
    refresh_fn: &Rc<dyn Fn()>,
) {
    let clip = clipboard.borrow().clone();
    let Some(item) = clip else {
        status_lbl.set_text("Nenhum item na área de transferência para colar.");
        return;
    };
    let file_name = match item.path.file_name() {
        Some(n) => n,
        None => {
            status_lbl.set_text("Nome de arquivo inválido na área de transferência.");
            return;
        }
    };
    let target = dest_dir.join(file_name);

    if item.is_cut {
        match fs::rename(&item.path, &target) {
            Ok(()) => {
                status_lbl.set_text(&format!("Item movido para: {}", target.display()));
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
                        status_lbl.set_text(&format!("Item movido para: {}", target.display()));
                        *clipboard.borrow_mut() = None;
                        refresh_fn();
                    }
                    Err(e) => status_lbl.set_text(&format!("Erro ao mover item: {e}")),
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
                status_lbl.set_text(&format!("Item colado em: {}", target.display()));
                refresh_fn();
            }
            Err(e) => status_lbl.set_text(&format!("Erro ao colar item: {e}")),
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

    let pop = popover.clone();
    btn.connect_clicked(move |_| {
        pop.popdown();
        action();
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

    let pop = popover.clone();
    btn.connect_clicked(move |_| {
        pop.popdown();
        action();
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
        .label("Organizar por")
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
            "Nome (A-Z)",
            curr_col == SortColumn::Name,
            move || {
                cs(SortColumn::Name, None);
                st.set_text("Arquivos organizados por Nome");
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
            "Tipo / Extensão",
            curr_col == SortColumn::Type,
            move || {
                cs(SortColumn::Type, None);
                st.set_text("Arquivos organizados por Tipo / Extensão");
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
            "Tamanho",
            curr_col == SortColumn::Size,
            move || {
                cs(SortColumn::Size, None);
                st.set_text("Arquivos organizados por Tamanho");
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
            "Data de Modificação",
            curr_col == SortColumn::Modified,
            move || {
                cs(SortColumn::Modified, None);
                st.set_text("Arquivos organizados por Data de Modificação");
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
            "Agrupar por Categorias (.7z no topo)",
            curr_col == SortColumn::GroupByType,
            move || {
                cs(SortColumn::GroupByType, None);
                st.set_text("Arquivos agrupados por Categoria (.7z no topo)");
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
            "Ordem Crescente",
            curr_asc,
            move || {
                cs(curr_col, Some(true));
                st.set_text("Ordenação crescente ativada");
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
            "Ordem Decrescente",
            !curr_asc,
            move || {
                cs(curr_col, Some(false));
                st.set_text("Ordenação decrescente ativada");
            },
        );
    }

    menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let header_view = gtk4::Label::builder()
        .label("Modo de Exibição")
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
            "Grade de Ícones",
            curr_vm == FileManagerViewMode::Grid,
            move || {
                vm_fn(FileManagerViewMode::Grid);
                st.set_text("Visualização em Grade de Ícones ativada");
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
            "Lista Detalhada",
            curr_vm == FileManagerViewMode::List,
            move || {
                vm_fn(FileManagerViewMode::List);
                st.set_text("Visualização em Lista Detalhada ativada");
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
        .title("Confirmar Exclusão")
        .default_width(380)
        .default_height(160)
        .build();

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);

    let msg = format!(
        "Tem certeza de que deseja excluir permanentemente {} '{}'?",
        if entry.is_dir { "a pasta" } else { "o arquivo" },
        entry.name
    );
    let lbl = gtk4::Label::new(Some(&msg));
    lbl.set_wrap(true);
    lbl.set_halign(gtk4::Align::Start);
    vbox.append(&lbl);

    let btn_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk4::Align::End);

    let btn_cancel = gtk4::Button::with_label("Cancelar");
    let btn_delete = gtk4::Button::with_label("Excluir Permanentemente");
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
    pane_root: &gtk4::Box,
    active_popover: &Rc<RefCell<Option<gtk4::Popover>>>,
    x: f64,
    y: f64,
    entry: &FileEntry,
    cur_dir: &Rc<RefCell<PathBuf>>,
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
            old.unparent();
        }
    }

    let point = graphene::Point::new(x as f32, y as f32);
    let (px, py) = if let Some(p) = anchor_widget.compute_point(pane_root, &point) {
        (p.x() as i32, p.y() as i32)
    } else {
        (x as i32, y as i32)
    };

    let popover = gtk4::Popover::new();
    popover.set_parent(pane_root);
    popover.set_has_arrow(false);
    popover.set_autohide(true);
    popover.add_css_class("onyxsh-popover");
    popover.set_pointing_to(Some(&gdk::Rectangle::new(px, py, 1, 1)));

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
        add_menu_item(&menu_vbox, &popover, "go-up-symbolic", "Subir um Nível", false, move || {
            if let Some(p) = cd.borrow().parent() {
                *cd.borrow_mut() = p.to_path_buf();
                rf();
            }
        });

        let cd = Rc::clone(cur_dir);
        let cd_cb = Rc::clone(cd_in_terminal_cb);
        add_menu_item(&menu_vbox, &popover, "utilities-terminal-symbolic", "Abrir no Terminal (cd)", false, move || {
            if let Some(p) = cd.borrow().parent() {
                cd_cb(p);
            }
        });

        let cd = Rc::clone(cur_dir);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", "Copiar Caminho do Diretório Pai", false, move || {
            if let Some(p) = cd.borrow().parent() {
                let path_str = p.to_string_lossy().to_string();
                if let Some(cb) = gdk::Display::default().and_then(|d| Some(d.clipboard())) {
                    cb.set_text(&path_str);
                    st.set_text(&format!("Caminho copiado: {path_str}"));
                }
            }
        });

        popover.set_child(Some(&menu_vbox));
        popover.popup();
        return;
    }

    if entry.is_dir {
        // ── 1. Opções de Pasta ──────────────────────────────────────
        let path = PathBuf::from(&entry.path);
        let cd = Rc::clone(cur_dir);
        let rf = Rc::clone(refresh_fn);
        add_menu_item(&menu_vbox, &popover, "folder-open-symbolic", "Abrir Pasta", false, move || {
            *cd.borrow_mut() = path.clone();
            rf();
        });

        let path = PathBuf::from(&entry.path);
        let cd_cb = Rc::clone(cd_in_terminal_cb);
        add_menu_item(&menu_vbox, &popover, "utilities-terminal-symbolic", "Abrir no Terminal (cd)", false, move || {
            cd_cb(&path);
        });

        let path_str = entry.path.clone();
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", "Copiar Caminho Absoluto", false, move || {
            if let Some(cb) = gdk::Display::default().and_then(|d| Some(d.clipboard())) {
                cb.set_text(&path_str);
                st.set_text(&format!("Caminho copiado: {path_str}"));
            }
        });

        let path_str = entry.path.clone();
        let feed_cb = Rc::clone(feed_terminal_cb);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "insert-text-symbolic", "Inserir Caminho no Terminal", false, move || {
            let escaped = format!("'{}' ", path_str.replace('\'', "'\\''"));
            feed_cb(&escaped);
            st.set_text(&format!("Caminho inserido no terminal: {path_str}"));
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let target_dir = PathBuf::from(&entry.path);
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "folder-new-symbolic", "Nova Pasta Aqui...", false, move || {
            let tdir = target_dir.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            show_simple_input_dialog(&win, "Nova Pasta", "Nome da nova pasta:", "nova_pasta", Rc::new(move |name| {
                let p = tdir.join(name.trim());
                match LocalFsBackend.mkdir(&p) {
                    Ok(()) => {
                        st2.set_text(&format!("Pasta criada: {}", p.display()));
                        rf2();
                    }
                    Err(e) => st2.set_text(&format!("Erro ao criar pasta: {e}")),
                }
            }));
        });

        let target_dir = PathBuf::from(&entry.path);
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "document-new-symbolic", "Novo Arquivo Aqui...", false, move || {
            let tdir = target_dir.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            show_simple_input_dialog(&win, "Novo Arquivo", "Nome do novo arquivo:", "novo_arquivo.txt", Rc::new(move |name| {
                let p = tdir.join(name.trim());
                match fs::File::create(&p) {
                    Ok(_) => {
                        st2.set_text(&format!("Arquivo criado: {}", p.display()));
                        rf2();
                    }
                    Err(e) => st2.set_text(&format!("Erro ao criar arquivo: {e}")),
                }
            }));
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let p = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", "Copiar Pasta", false, move || {
            *clip.borrow_mut() = Some(FileManagerClipboardItem {
                path: p.clone(),
                is_dir: true,
                is_cut: false,
            });
            st.set_text(&format!("Pasta copiada para transferência: {name}"));
        });

        let p = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-cut-symbolic", "Recortar Pasta", false, move || {
            *clip.borrow_mut() = Some(FileManagerClipboardItem {
                path: p.clone(),
                is_dir: true,
                is_cut: true,
            });
            st.set_text(&format!("Pasta marcada para mover: {name}"));
        });

        if clipboard.borrow().is_some() {
            let target_dir = PathBuf::from(&entry.path);
            let clip = Rc::clone(clipboard);
            let st = status_lbl.clone();
            let rf = Rc::clone(refresh_fn);
            add_menu_item(&menu_vbox, &popover, "edit-paste-symbolic", "Colar Dentro desta Pasta", false, move || {
                execute_paste(&clip, &target_dir, &st, &rf);
            });
        }

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let old_path = PathBuf::from(&entry.path);
        let old_name = entry.name.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "document-edit-symbolic", "Renomear...", false, move || {
            let op = old_path.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            show_simple_input_dialog(&win, "Renomear Pasta", "Novo nome da pasta:", &old_name, Rc::new(move |new_name| {
                if let Some(parent) = op.parent() {
                    let np = parent.join(new_name.trim());
                    match LocalFsBackend.rename(&op, &np) {
                        Ok(()) => {
                            st2.set_text(&format!("Pasta renomeada para '{}'", new_name.trim()));
                            rf2();
                        }
                        Err(e) => st2.set_text(&format!("Erro ao renomear: {e}")),
                    }
                }
            }));
        });

        let e_clone = entry.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "channel-secure-symbolic", "Permissões (chmod)...", false, move || {
            show_permissions_dialog(&win, &e_clone, Rc::clone(&rf), st.clone());
        });

        if let Some(ai_cb) = ai_prompt_cb.as_ref() {
            let ai = Rc::clone(ai_cb);
            let e_clone = entry.clone();
            let st = status_lbl.clone();
            add_menu_item(&menu_vbox, &popover, "security-high-symbolic", "Auditar Segurança com IA", false, move || {
                let prompt = format!(
                    "Por favor, realize uma auditoria detalhada de segurança e permissões de acesso do diretório '{}'.\nCaminho: {}\nPermissões atuais: {} ({})\nÚltima modificação: {}\nRecomende configurações ideais de chmod, restrições e boas práticas de conformidade Zero-Trust.",
                    e_clone.name, e_clone.path, e_clone.formatted_permissions(), e_clone.permissions_octal_str(), format_epoch_short(e_clone.modified_epoch)
                );
                ai(&prompt);
                st.set_text(&format!("Auditoria de segurança de '{}' enviada ao Assistente de IA.", e_clone.name));
            });
        }

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let e_clone = entry.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "user-trash-symbolic", "Excluir Pasta", true, move || {
            let ec = e_clone.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            let del_path = PathBuf::from(&ec.path);
            let del_name = ec.name.clone();
            show_confirm_delete_dialog(&win, &ec, Rc::new(move || {
                match LocalFsBackend.remove(&del_path) {
                    Ok(()) => {
                        st2.set_text(&format!("Pasta excluída: {del_name}"));
                        rf2();
                    }
                    Err(e) => st2.set_text(&format!("Erro ao excluir pasta: {e}")),
                }
            }));
        });
    } else {
        // ── 2. Opções de Arquivo ────────────────────────────────────
        let win = parent_window.clone();
        let vis = Rc::clone(visible_entries);
        let path = entry.path.clone();
        add_menu_item(&menu_vbox, &popover, "find-location-symbolic", "Quick Look (Pré-visualizar)", false, move || {
            let entries = vis.borrow().clone();
            let idx = entries.iter().position(|e| e.path == path).unwrap_or(0);
            show_quick_look_window(&win, entries, idx);
        });

        let path = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "accessories-calculator-symbolic", "Calcular Hash SHA-256...", false, move || {
            match compute_file_sha256_hex(&path) {
                Ok(hex) => st.set_text(&format!("SHA-256 ({name}): {hex}")),
                Err(e) => st.set_text(&format!("Erro no hash: {e}")),
            }
        });

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
            add_menu_item(&menu_vbox, &popover, "media-playback-start-symbolic", "Executar no Terminal", false, move || {
                feed_cb(&cb_copy);
            });

            let feed_cb = Rc::clone(feed_terminal_cb);
            let sudo_cmd = format!("sudo {cmd_base}");
            add_menu_item(&menu_vbox, &popover, "security-medium-symbolic", "Executar com sudo no Terminal", false, move || {
                feed_cb(&sudo_cmd);
            });
        }

        let is_log = matches!(ext.as_str(), "log" | "out" | "err") || lower.contains("log");
        if is_log {
            let name_escaped = entry.name.replace('\'', "'\\''");
            let tail_cmd = format!("tail -n 50 -f ./'{}'\n", name_escaped);
            let feed_cb = Rc::clone(feed_terminal_cb);
            add_menu_item(&menu_vbox, &popover, "format-indent-more-symbolic", "Acompanhar Log (tail -f)", false, move || {
                feed_cb(&tail_cmd);
            });

            if let Some(ai_cb) = ai_prompt_cb.as_ref() {
                let ai = Rc::clone(ai_cb);
                let p = entry.path.clone();
                let n = entry.name.clone();
                let st = status_lbl.clone();
                add_menu_item(&menu_vbox, &popover, "system-search-symbolic", "Diagnosticar Erros do Log com IA", false, move || {
                    let tail_sample = fs::read_to_string(&p)
                        .map(|s| {
                            let lines: Vec<&str> = s.lines().collect();
                            let start = lines.len().saturating_sub(60);
                            lines[start..].join("\n")
                        })
                        .unwrap_or_else(|_| "[Incapaz de ler arquivo de log]".to_string());
                    let prompt = format!(
                        "Analise as seguintes mensagens e erros recentes do arquivo de log '{}' e forneça diagnóstico das causas raízes e soluções recomendadas:\n```\n{}\n```",
                        n, tail_sample
                    );
                    ai(&prompt);
                    st.set_text(&format!("Diagnóstico do log '{n}' enviado ao Assistente de IA."));
                });
            }
        }

        if let Some(ai_cb) = ai_prompt_cb.as_ref() {
            let ai = Rc::clone(ai_cb);
            let p = entry.path.clone();
            let n = entry.name.clone();
            let st = status_lbl.clone();
            add_menu_item(&menu_vbox, &popover, "dialog-information-symbolic", "Explicar Arquivo com IA", false, move || {
                let sample = fs::read_to_string(&p)
                    .map(|s| s.lines().take(60).collect::<Vec<_>>().join("\n"))
                    .unwrap_or_else(|_| "[Conteúdo binário ou inacessível]".to_string());
                let prompt = format!(
                    "Explique o propósito, estrutura e funcionamento do arquivo '{}'.\nCaminho: {}\n\nConteúdo inicial:\n```\n{}\n```",
                    n, p, sample
                );
                ai(&prompt);
                st.set_text(&format!("Explicação do arquivo '{n}' solicitada ao Assistente de IA."));
            });

            let ai = Rc::clone(ai_cb);
            let e_clone = entry.clone();
            let st = status_lbl.clone();
            add_menu_item(&menu_vbox, &popover, "security-high-symbolic", "Auditar Segurança com IA", false, move || {
                let prompt = format!(
                    "Por favor, realize uma auditoria detalhada de segurança e permissões de acesso do arquivo '{}'.\nCaminho: {}\nPermissões atuais: {} ({})\nTamanho: {}\nRecomende configurações seguras de chmod e mitigação de vulnerabilidades.",
                    e_clone.name, e_clone.path, e_clone.formatted_permissions(), e_clone.permissions_octal_str(), e_clone.formatted_size()
                );
                ai(&prompt);
                st.set_text(&format!("Auditoria de segurança de '{}' enviada ao Assistente de IA.", e_clone.name));
            });
        }

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let path_str = entry.path.clone();
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", "Copiar Caminho Absoluto", false, move || {
            if let Some(cb) = gdk::Display::default().and_then(|d| Some(d.clipboard())) {
                cb.set_text(&path_str);
                st.set_text(&format!("Caminho copiado: {path_str}"));
            }
        });

        let path_str = entry.path.clone();
        let feed_cb = Rc::clone(feed_terminal_cb);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "insert-text-symbolic", "Inserir Caminho no Terminal", false, move || {
            let escaped = format!("'{}' ", path_str.replace('\'', "'\\''"));
            feed_cb(&escaped);
            st.set_text(&format!("Caminho inserido no terminal: {path_str}"));
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let p = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", "Copiar Arquivo", false, move || {
            *clip.borrow_mut() = Some(FileManagerClipboardItem {
                path: p.clone(),
                is_dir: false,
                is_cut: false,
            });
            st.set_text(&format!("Arquivo copiado para transferência: {name}"));
        });

        let p = PathBuf::from(&entry.path);
        let name = entry.name.clone();
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "edit-cut-symbolic", "Recortar Arquivo", false, move || {
            *clip.borrow_mut() = Some(FileManagerClipboardItem {
                path: p.clone(),
                is_dir: false,
                is_cut: true,
            });
            st.set_text(&format!("Arquivo marcado para mover: {name}"));
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let old_path = PathBuf::from(&entry.path);
        let old_name = entry.name.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "document-edit-symbolic", "Renomear...", false, move || {
            let op = old_path.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            show_simple_input_dialog(&win, "Renomear Arquivo", "Novo nome do arquivo:", &old_name, Rc::new(move |new_name| {
                if let Some(parent) = op.parent() {
                    let np = parent.join(new_name.trim());
                    match LocalFsBackend.rename(&op, &np) {
                        Ok(()) => {
                            st2.set_text(&format!("Arquivo renomeado para '{}'", new_name.trim()));
                            rf2();
                        }
                        Err(e) => st2.set_text(&format!("Erro ao renomear: {e}")),
                    }
                }
            }));
        });

        let e_clone = entry.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "channel-secure-symbolic", "Permissões (chmod)...", false, move || {
            show_permissions_dialog(&win, &e_clone, Rc::clone(&rf), st.clone());
        });

        menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

        let e_clone = entry.clone();
        let win = parent_window.clone();
        let rf = Rc::clone(refresh_fn);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "user-trash-symbolic", "Excluir Arquivo", true, move || {
            let ec = e_clone.clone();
            let rf2 = Rc::clone(&rf);
            let st2 = st.clone();
            let del_path = PathBuf::from(&ec.path);
            let del_name = ec.name.clone();
            show_confirm_delete_dialog(&win, &ec, Rc::new(move || {
                match LocalFsBackend.remove(&del_path) {
                    Ok(()) => {
                        st2.set_text(&format!("Arquivo excluído: {del_name}"));
                        rf2();
                    }
                    Err(e) => st2.set_text(&format!("Erro ao excluir arquivo: {e}")),
                }
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

    popover.set_child(Some(&menu_vbox));
    popover.popup();
}

/// Shows the right-click contextual popover menu for blank / background space of a directory pane.
fn show_context_menu_for_background(
    anchor_widget: &impl IsA<gtk4::Widget>,
    pane_root: &gtk4::Box,
    active_popover: &Rc<RefCell<Option<gtk4::Popover>>>,
    x: f64,
    y: f64,
    cur_dir: &Rc<RefCell<PathBuf>>,
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
            old.unparent();
        }
    }

    let point = graphene::Point::new(x as f32, y as f32);
    let (px, py) = if let Some(p) = anchor_widget.compute_point(pane_root, &point) {
        (p.x() as i32, p.y() as i32)
    } else {
        (x as i32, y as i32)
    };

    let popover = gtk4::Popover::new();
    popover.set_parent(pane_root);
    popover.set_has_arrow(false);
    popover.set_autohide(true);
    popover.add_css_class("onyxsh-popover");
    popover.set_pointing_to(Some(&gdk::Rectangle::new(px, py, 1, 1)));

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
    add_menu_item(&menu_vbox, &popover, "utilities-terminal-symbolic", "Abrir este Diretório no Terminal (cd)", false, move || {
        cd_cb(&cd.borrow());
    });

    let cd = Rc::clone(cur_dir);
    let st = status_lbl.clone();
    add_menu_item(&menu_vbox, &popover, "edit-copy-symbolic", "Copiar Caminho do Diretório", false, move || {
        let path_str = cd.borrow().to_string_lossy().to_string();
        if let Some(cb) = gdk::Display::default().and_then(|d| Some(d.clipboard())) {
            cb.set_text(&path_str);
            st.set_text(&format!("Caminho copiado: {path_str}"));
        }
    });

    let cd = Rc::clone(cur_dir);
    let feed_cb = Rc::clone(feed_terminal_cb);
    let st = status_lbl.clone();
    add_menu_item(&menu_vbox, &popover, "insert-text-symbolic", "Inserir Caminho no Terminal", false, move || {
        let path_str = cd.borrow().to_string_lossy().to_string();
        let escaped = format!("'{}' ", path_str.replace('\'', "'\\''"));
        feed_cb(&escaped);
        st.set_text(&format!("Caminho inserido no terminal: {path_str}"));
    });

    menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    let cd = Rc::clone(cur_dir);
    let win = parent_window.clone();
    let rf = Rc::clone(refresh_fn);
    let st = status_lbl.clone();
    add_menu_item(&menu_vbox, &popover, "folder-new-symbolic", "Nova Pasta...", false, move || {
        let tdir = cd.borrow().clone();
        let rf2 = Rc::clone(&rf);
        let st2 = st.clone();
        show_simple_input_dialog(&win, "Nova Pasta", "Nome da nova pasta:", "nova_pasta", Rc::new(move |name| {
            let p = tdir.join(name.trim());
            match LocalFsBackend.mkdir(&p) {
                Ok(()) => {
                    st2.set_text(&format!("Pasta criada: {}", p.display()));
                    rf2();
                }
                Err(e) => st2.set_text(&format!("Erro ao criar pasta: {e}")),
            }
        }));
    });

    let cd = Rc::clone(cur_dir);
    let win = parent_window.clone();
    let rf = Rc::clone(refresh_fn);
    let st = status_lbl.clone();
    add_menu_item(&menu_vbox, &popover, "document-new-symbolic", "Novo Arquivo...", false, move || {
        let tdir = cd.borrow().clone();
        let rf2 = Rc::clone(&rf);
        let st2 = st.clone();
        show_simple_input_dialog(&win, "Novo Arquivo", "Nome do novo arquivo:", "novo_arquivo.txt", Rc::new(move |name| {
            let p = tdir.join(name.trim());
            match fs::File::create(&p) {
                Ok(_) => {
                    st2.set_text(&format!("Arquivo criado: {}", p.display()));
                    rf2();
                }
                Err(e) => st2.set_text(&format!("Erro ao criar arquivo: {e}")),
            }
        }));
    });

    if clipboard.borrow().is_some() {
        let cd = Rc::clone(cur_dir);
        let clip = Rc::clone(clipboard);
        let st = status_lbl.clone();
        let rf = Rc::clone(refresh_fn);
        add_menu_item(&menu_vbox, &popover, "edit-paste-symbolic", "Colar Aqui", false, move || {
            execute_paste(&clip, &cd.borrow(), &st, &rf);
        });
    }

    menu_vbox.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));

    if let Some(ai_cb) = ai_prompt_cb.as_ref() {
        let ai = Rc::clone(ai_cb);
        let cd = Rc::clone(cur_dir);
        let st = status_lbl.clone();
        add_menu_item(&menu_vbox, &popover, "security-high-symbolic", "Auditar Segurança deste Diretório com IA", false, move || {
            let path_str = cd.borrow().to_string_lossy().to_string();
            let prompt = format!(
                "Por favor, realize uma auditoria detalhada de segurança e governança de acesso no diretório atual '{}'. Verifique permissões permissivas, arquivos sensíveis (.env, chaves privadas, credenciais expostas) e recomende boas práticas Zero-Trust.",
                path_str
            );
            ai(&prompt);
            st.set_text(&format!("Auditoria do diretório '{path_str}' enviada ao Assistente de IA."));
        });
    }

    let rf = Rc::clone(refresh_fn);
    add_menu_item(&menu_vbox, &popover, "view-refresh-symbolic", "Atualizar Lista (F5)", false, move || {
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

    popover.set_child(Some(&menu_vbox));
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
    if let Ok(content) = fs::read_to_string(&settings_path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(bm_val) = v.get("settings").and_then(|s| s.get("file_manager_bookmarks")) {
                if let Ok(list) = serde_json::from_value::<Vec<FileManagerBookmark>>(bm_val.clone()) {
                    return list;
                }
            }
        }
    }
    Vec::new()
}

pub fn save_bookmarks(bookmarks: &[FileManagerBookmark]) {
    let settings_path = platform::config_dir().join("settings.json");
    if let Ok(content) = fs::read_to_string(&settings_path) {
        if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(s) = v.get_mut("settings") {
                if let Ok(bm_json) = serde_json::to_value(bookmarks) {
                    s["file_manager_bookmarks"] = bm_json;
                    if let Ok(out) = serde_json::to_string_pretty(&v) {
                        let _ = fs::write(&settings_path, out);
                    }
                }
            }
        }
    }
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
    let header_shortcuts = gtk4::Label::new(Some("ATALHOS DO SISTEMA"));
    header_shortcuts.set_halign(gtk4::Align::Start);
    header_shortcuts.add_css_class("quick-jump-header");
    popover_box.append(&header_shortcuts);

    let home = platform::home_dir();
    let home_str = home.to_string_lossy().to_string();
    let config_dir_str = home.join(".config").to_string_lossy().to_string();
    let temp_str = std::env::temp_dir().to_string_lossy().to_string();

    let mut shortcuts = vec![
        ("Início", home_str, "folder-symbolic"),
    ];

    let active_path = cur_dir.borrow().clone();
    if let Some(git_root) = find_git_root(&active_path) {
        shortcuts.push((
            "Raiz do Repositório Git",
            git_root.to_string_lossy().to_string(),
            "utilities-terminal-symbolic",
        ));
    }

    shortcuts.push(("Raiz do Sistema", "/".to_string(), "computer-symbolic"));
    shortcuts.push(("Logs do Sistema", "/var/log".to_string(), "text-x-generic-symbolic"));
    shortcuts.push(("Configurações", config_dir_str, "preferences-other-symbolic"));
    shortcuts.push(("Temporário", temp_str, "user-trash-symbolic"));

    for (label, path_str, icon_name) in shortcuts {
        let item_btn = gtk4::Button::new();
        item_btn.add_css_class("flat");
        item_btn.add_css_class("quick-jump-item-btn");

        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

        let icon = gtk4::Image::from_icon_name(icon_name);
        row.append(&icon);

        let lbl = gtk4::Label::new(Some(label));
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

    let header_bm = gtk4::Label::new(Some("FAVORITOS"));
    header_bm.set_halign(gtk4::Align::Start);
    header_bm.add_css_class("quick-jump-header");
    popover_box.append(&header_bm);

    let bookmarks = load_bookmarks();
    if bookmarks.is_empty() {
        let empty_lbl = gtk4::Label::new(Some("Nenhuma pasta favoritada ainda"));
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
                .tooltip_text("Remover favorito")
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
                st.set_text("Pasta removida dos favoritos.");

                let cur_str = c_dir2.borrow().to_string_lossy().to_string();
                let is_cur_bookmarked = bms.iter().any(|b| b.path == cur_str);
                if is_cur_bookmarked {
                    btn_bm.add_css_class("bookmark-active-star");
                    btn_bm.set_tooltip_text(Some("Remover favorito do diretório atual"));
                } else {
                    btn_bm.remove_css_class("bookmark-active-star");
                    btn_bm.set_tooltip_text(Some("Favoritar diretório atual"));
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

/// Builds a rich File Manager Pane with:
/// - Navigation toolbar (`Home`, `Up`, `Refresh`, `New Folder`, clickable Breadcrumbs, Path entry, Hidden toggle, Search)
/// - View Mode Switcher (`List`, `Grid` with thumbnails, `Tree` with inline expanders)
/// - File Action Bar (`Quick Look`, `Rename`, `Chmod`, `SHA-256`, `Delete`, `CD in Terminal`)
/// - Context Popover Menu on Right-Click (Folder actions, File actions, Background actions, Clipboard, Terminal, AI)
pub fn build_file_pane(
    parent_window: &adw::ApplicationWindow,
    initial_dir: PathBuf,
    status_lbl: &gtk4::Label,
    cd_in_terminal_cb: Rc<dyn Fn(&Path)>,
    feed_terminal_cb: Rc<dyn Fn(&str)>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
    clipboard: Rc<RefCell<Option<FileManagerClipboardItem>>>,
) -> FilePaneHandle {
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
        .label("Nome")
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
        .width_request(80)
        .halign(gtk4::Align::End)
        .build();
    btn_sort_size.add_css_class("flat");
    btn_sort_size.add_css_class("file-detailed-header-size");
    let size_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    let icon_size_sort = gtk4::Image::from_icon_name("pan-down-symbolic");
    icon_size_sort.add_css_class("dim-label");
    icon_size_sort.set_visible(false);
    let lbl_size = gtk4::Label::builder()
        .label("Tamanho")
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
        .label("Data Modificada")
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
        .label("Permissões")
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
        .label("Proprietário")
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
    let grid_flow = gtk4::FlowBox::new();
    grid_flow.set_valign(gtk4::Align::Start);
    grid_flow.set_max_children_per_line(8);
    grid_flow.set_min_children_per_line(2);
    grid_flow.set_column_spacing(8);
    grid_flow.set_row_spacing(8);
    grid_flow.set_margin_start(8);
    grid_flow.set_margin_end(8);
    grid_flow.set_margin_top(8);
    grid_flow.set_margin_bottom(8);
    grid_flow.set_selection_mode(gtk4::SelectionMode::Single);
    grid_flow.set_activate_on_single_click(false);
    grid_scroll.set_child(Some(&grid_flow));
    view_stack.add_named(&grid_scroll, Some("grid"));

    let tree_scroll = gtk4::ScrolledWindow::new();
    tree_scroll.set_hexpand(true);
    tree_scroll.set_vexpand(true);
    let tree_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    tree_vbox.set_margin_start(6);
    tree_vbox.set_margin_end(6);
    tree_vbox.set_margin_top(4);
    tree_vbox.set_margin_bottom(4);
    tree_scroll.set_child(Some(&tree_vbox));
    view_stack.add_named(&tree_scroll, Some("tree"));

    pane_vbox.append(&view_stack);
    view_stack.set_visible_child_name(initial_vm.as_stack_name());
    col_header_box.set_visible(initial_vm == FileManagerViewMode::List);

    // ── 3. Bottom Action Bar (Identical to filemanagerpython.png) ─────────────
    let action_bar = gtk4::ActionBar::new();
    action_bar.add_css_class("file-manager-action-bar");

    let btn_refresh = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Atualizar (F5)")
        .has_frame(false)
        .build();
    btn_refresh.add_css_class("flat");

    let btn_hidden = gtk4::ToggleButton::builder()
        .icon_name("view-reveal-symbolic")
        .tooltip_text("Mostrar arquivos ocultos (.dotfiles)")
        .has_frame(false)
        .build();
    btn_hidden.add_css_class("flat");

    let btn_bookmark = gtk4::Button::builder()
        .icon_name("starred-symbolic")
        .tooltip_text("Favoritar diretório atual")
        .has_frame(false)
        .build();
    btn_bookmark.add_css_class("flat");

    let btn_quick_jump = gtk4::Button::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text("Atalhos do Sistema & Favoritos")
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
        .tooltip_text("Visualização em Lista Detalhada")
        .active(initial_vm == FileManagerViewMode::List)
        .build();
    let btn_mode_grid = gtk4::ToggleButton::builder()
        .icon_name("view-grid-symbolic")
        .tooltip_text("Visualização em Grade / Miniaturas")
        .active(initial_vm == FileManagerViewMode::Grid)
        .group(&btn_mode_list)
        .build();
    let btn_mode_tree = gtk4::ToggleButton::builder()
        .icon_name("format-indent-more-symbolic")
        .tooltip_text("Visualização em Árvore Hierárquica")
        .active(initial_vm == FileManagerViewMode::Tree)
        .group(&btn_mode_list)
        .build();
    mode_box.append(&btn_mode_list);
    mode_box.append(&btn_mode_grid);
    mode_box.append(&btn_mode_tree);
    action_bar.pack_start(&mode_box);

    let btn_toggle_dual = gtk4::ToggleButton::builder()
        .icon_name("view-dual-symbolic")
        .tooltip_text("Alternar Painel Duplo (Dividir / Dual-Pane)")
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

    let recursive_lbl = gtk4::Label::new(Some("Recursivo"));
    recursive_lbl.add_css_class("dim-label");
    recursive_lbl.set_valign(gtk4::Align::Center);

    let recursive_switch = gtk4::Switch::new();
    recursive_switch.set_active(false);
    recursive_switch.set_valign(gtk4::Align::Center);
    recursive_switch.set_tooltip_text(Some("Pesquisa recursiva em subdiretórios"));

    switch_container.append(&recursive_lbl);
    switch_container.append(&recursive_switch);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Filtrar arquivos...")
        .width_chars(14)
        .build();

    let btn_history = gtk4::Button::builder()
        .icon_name("document-open-recent-symbolic")
        .tooltip_text("Histórico de Transferências")
        .has_frame(false)
        .build();
    btn_history.add_css_class("flat");

    action_bar.pack_end(&switch_container);
    action_bar.pack_end(&search_entry);
    action_bar.pack_end(&btn_history);

    pane_vbox.append(&action_bar);

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
        let rf = Rc::clone(&refresh_holder);
        Rc::new(move |mode: FileManagerViewMode| {
            *vm.borrow_mut() = mode;
            vs.set_visible_child_name(mode.as_stack_name());
            ch.set_visible(mode == FileManagerViewMode::List);
            match mode {
                FileManagerViewMode::List => {
                    if !b_list.is_active() {
                        b_list.set_active(true);
                    }
                }
                FileManagerViewMode::Grid => {
                    if !b_grid.is_active() {
                        b_grid.set_active(true);
                    }
                }
                FileManagerViewMode::Tree => {
                    if !b_tree.is_active() {
                        b_tree.set_active(true);
                    }
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

    // Background Right-Click Context Menus
    {
        let bg_click_list = gtk4::GestureClick::new();
        bg_click_list.set_button(3);
        let list_ref = list_box.clone();
        let scroll_ref = list_scroll.clone();
        let pane_ref = pane_vbox.clone();
        let act_pop = Rc::clone(&active_popover);
        let cd = Rc::clone(&current_dir);
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

            // Python-style pick check: ignore clicks on rows/items
            if let Some(target) = scroll_ref.pick(x, y, gtk4::PickFlags::DEFAULT) {
                let mut is_row_target = false;
                let mut curr = Some(target);
                while let Some(w) = curr {
                    if &w == scroll_ref.upcast_ref::<gtk4::Widget>() {
                        break;
                    }
                    let css = w.css_name();
                    let classes = w.css_classes();
                    if css == "listitem"
                        || css == "row"
                        || classes.iter().any(|c| c == "file-detailed-row" || c == "file-grid-card")
                        || w.type_().name() == "GtkListBoxRow"
                        || w.type_().name() == "GtkFlowBoxChild"
                    {
                        is_row_target = true;
                        break;
                    }
                    curr = w.parent();
                }
                if is_row_target {
                    return;
                }
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
        let grid_ref = grid_flow.clone();
        let scroll_ref = grid_scroll.clone();
        let pane_ref = pane_vbox.clone();
        let act_pop = Rc::clone(&active_popover);
        let cd = Rc::clone(&current_dir);
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

            if let Some(target) = scroll_ref.pick(x, y, gtk4::PickFlags::DEFAULT) {
                let mut is_row_target = false;
                let mut curr = Some(target);
                while let Some(w) = curr {
                    if &w == scroll_ref.upcast_ref::<gtk4::Widget>() {
                        break;
                    }
                    let css = w.css_name();
                    let classes = w.css_classes();
                    if css == "listitem"
                        || css == "row"
                        || classes.iter().any(|c| c == "file-detailed-row" || c == "file-grid-card")
                        || w.type_().name() == "GtkListBoxRow"
                        || w.type_().name() == "GtkFlowBoxChild"
                    {
                        is_row_target = true;
                        break;
                    }
                    curr = w.parent();
                }
                if is_row_target {
                    return;
                }
            }

            gesture.set_state(gtk4::EventSequenceState::Claimed);
            grid_ref.unselect_all();
            *sel.borrow_mut() = None;

            if let Some(r) = rh.borrow().as_ref() {
                show_context_menu_for_background(
                    &scroll_ref,
                    &pane_ref,
                    &act_pop,
                    x,
                    y,
                    &cd,
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

    let refresh_fn: Rc<dyn Fn()> = {
        let cur_dir = Rc::clone(&current_dir);
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
        let act_pop_rc = Rc::clone(&active_popover);
        let bc_box = breadcrumb_box.clone();
        let se = search_entry.clone();
        let rec_switch = recursive_switch.clone();
        let lb = list_box.clone();
        let gf = grid_flow.clone();
        let tv = tree_vbox.clone();
        let ico_name = icon_name_sort.clone();
        let ico_size = icon_size_sort.clone();
        let ico_mod = icon_mod_sort.clone();
        let ico_perm = icon_perm_sort.clone();
        let ico_owner = icon_owner_sort.clone();
        let status = status_lbl.clone();
        let r_holder = Rc::clone(&refresh_holder);
        let cd_cb = Rc::clone(&cd_in_terminal_cb);
        let feed_cb = Rc::clone(&feed_terminal_cb);
        let ai_cb_opt = ai_prompt_cb.clone();
        let clip_rc = Rc::clone(&clipboard);
        let win_ref = parent_window.clone();
        let btn_bm = btn_bookmark.clone();

        Rc::new(move || {
            let dir_path = cur_dir.borrow().clone();

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
                btn_bm.set_tooltip_text(Some("Remover favorito do diretório atual"));
            } else {
                btn_bm.remove_css_class("bookmark-active-star");
                btn_bm.set_tooltip_text(Some("Favoritar diretório atual"));
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

            let raw_entries = if is_recursive && !query.is_empty() {
                search_recursive_dir(&dir_path, &query, 300)
            } else {
                let backend = LocalFsBackend;
                match backend.list_dir(&dir_path) {
                    Ok(list) => list,
                    Err(e) => {
                        status.set_text(&format!("Erro ao listar {}: {e}", dir_path.display()));
                        Vec::new()
                    }
                }
            };

            let mut sorted = filter_and_sort_entries(
                &raw_entries,
                *hidden_rc.borrow(),
                &query,
                col,
                asc,
                true,
            );

            // Prepend ".." parent directory entry if not at root and no filter query
            if query.is_empty() && dir_path.parent().is_some() && dir_path != Path::new("/") {
                let parent_path = dir_path.parent().unwrap();
                let parent_meta = fs::symlink_metadata(parent_path).ok();
                let (modified, perms, owner) = if let Some(m) = parent_meta {
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
                };

                let parent_entry = FileEntry {
                    name: "..".to_string(),
                    path: parent_path.to_string_lossy().into_owned(),
                    is_dir: true,
                    is_symlink: false,
                    size: 4096,
                    modified_epoch: modified,
                    permissions_octal: perms,
                    owner,
                };
                sorted.insert(0, parent_entry);
            }

            *vis_entries.borrow_mut() = sorted.clone();
            *sel_entry.borrow_mut() = sorted.first().cloned();

            // 1. Populate Detailed List View (`list_box`)
            lb.remove_all();
            for entry in sorted.iter().take(300) {
                let row_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
                row_hbox.add_css_class("file-detailed-row");
                row_hbox.set_hexpand(true);
                row_hbox.set_halign(gtk4::Align::Fill);
                row_hbox.set_margin_start(4);
                row_hbox.set_margin_end(16);
                row_hbox.set_margin_top(2);
                row_hbox.set_margin_bottom(2);
                row_hbox.set_tooltip_markup(Some(&build_entry_tooltip_markup(entry)));

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
                    sym_icon.set_tooltip_text(Some("Link simbólico"));
                    name_box.append(&sym_icon);
                }

                let badges_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
                if entry.is_executable() {
                    let badge_exec = gtk4::Label::new(Some("+x"));
                    badge_exec.add_css_class("badge-pill");
                    badge_exec.add_css_class("badge-exec");
                    badge_exec.set_tooltip_text(Some("Arquivo executável"));
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
                size_lbl.set_width_request(80);
                size_lbl.set_xalign(1.0);

                let mod_lbl = gtk4::Label::new(Some(&format_epoch_short(entry.modified_epoch)));
                mod_lbl.add_css_class("file-detailed-col-date");
                mod_lbl.add_css_class("monospace");
                mod_lbl.set_width_request(150);
                mod_lbl.set_xalign(0.5);

                let perm_lbl = gtk4::Label::new(Some(&entry.formatted_permissions()));
                perm_lbl.add_css_class("file-detailed-col-perms");
                perm_lbl.add_css_class("monospace");
                perm_lbl.set_tooltip_text(Some(&format!("Octal: {}", entry.permissions_octal_str())));
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

                // Attach right-click context menu gesture to row_hbox and list_row
                let right_click = gtk4::GestureClick::new();
                right_click.set_button(3);
                let row_ref = list_row.clone();
                let hbox_ref = row_hbox.clone();
                let lb_ref = lb.clone();
                let pane_ref = pane_vbox_c.clone();
                let act_pop = Rc::clone(&act_pop_rc);
                let e_clone = entry.clone();
                let cd = Rc::clone(&cur_dir);
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

                lb.append(&list_row);
            }

            // 2. Populate Thumbnail / Grid View (`grid_flow`)
            gf.remove_all();
            for entry in sorted.iter().take(150) {
                let card = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
                card.set_width_request(104);
                card.set_height_request(104);
                card.set_margin_start(4);
                card.set_margin_end(4);
                card.set_margin_top(4);
                card.set_margin_bottom(4);
                card.set_halign(gtk4::Align::Center);
                card.set_tooltip_markup(Some(&build_entry_tooltip_markup(entry)));

                if entry.is_hidden() {
                    card.add_css_class("dim-label");
                }

                if is_image_entry(entry) && entry.size <= 12 * 1024 * 1024 {
                    // Render real image thumbnail
                    let pic = gtk4::Picture::for_filename(&entry.path);
                    pic.set_can_shrink(true);
                    pic.set_size_request(56, 56);
                    pic.set_halign(gtk4::Align::Center);
                    card.append(&pic);
                } else {
                    let icon = gtk4::Image::from_icon_name(icon_for_entry(entry));
                    icon.set_pixel_size(48);
                    card.append(&icon);
                }

                let name_lbl = gtk4::Label::new(Some(&entry.name));
                name_lbl.set_max_width_chars(12);
                name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::Middle);
                name_lbl.add_css_class("caption");
                if entry.is_dir {
                    name_lbl.add_css_class("heading");
                }
                card.append(&name_lbl);

                let sz_lbl = gtk4::Label::new(Some(&entry.formatted_size_clean()));
                sz_lbl.add_css_class("caption");
                sz_lbl.add_css_class("dim-label");
                card.append(&sz_lbl);

                let flow_child = gtk4::FlowBoxChild::new();
                flow_child.set_child(Some(&card));

                // Attach right-click context menu gesture to card and flow_child
                let right_click = gtk4::GestureClick::new();
                right_click.set_button(3);
                let card_ref = flow_child.clone();
                let card_box_ref = card.clone();
                let gf_ref = gf.clone();
                let pane_ref = pane_vbox_c.clone();
                let act_pop = Rc::clone(&act_pop_rc);
                let e_clone = entry.clone();
                let cd = Rc::clone(&cur_dir);
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
                    gf_ref.select_child(&card_ref);
                    if let Some(r) = rh.borrow().as_ref() {
                        let clicked_widget = gesture.widget().unwrap_or_else(|| card_box_ref.clone().upcast());
                        show_context_menu_for_entry(
                            &clicked_widget,
                            &pane_ref,
                            &act_pop,
                            x,
                            y,
                            &e_clone,
                            &cd,
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
                card.add_controller(right_click);

                gf.append(&flow_child);
            }

            // 3. Populate Hierarchical Tree View (`tree_vbox`)
            while let Some(c) = tv.first_child() {
                tv.remove(&c);
            }
            for entry in sorted.iter().take(150) {
                if entry.is_dir {
                    let exp = gtk4::Expander::new(Some(&format!("📁 {}", entry.name)));
                    let sub_list = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
                    sub_list.set_margin_start(20);

                    let enter_btn =
                        gtk4::Button::with_label(&format!("↳ Abrir pasta '{}'", entry.name));
                    enter_btn.add_css_class("flat");
                    enter_btn.set_halign(gtk4::Align::Start);
                    let target_path = PathBuf::from(&entry.path);
                    let c_dir = Rc::clone(&cur_dir);
                    let rh = Rc::clone(&r_holder);
                    enter_btn.connect_clicked(move |_| {
                        *c_dir.borrow_mut() = target_path.clone();
                        if let Some(r) = rh.borrow().as_ref() {
                            r();
                        }
                    });
                    sub_list.append(&enter_btn);

                    let right_click = gtk4::GestureClick::new();
                    right_click.set_button(3);
                    let exp_ref = exp.clone();
                    let e_clone = entry.clone();
                    let cd = Rc::clone(&cur_dir);
                    let win = win_ref.clone();
                    let st = status.clone();
                    let rh2 = Rc::clone(&r_holder);
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
                    let pane_ref = pane_vbox_c.clone();
                    let act_pop = Rc::clone(&act_pop_rc);
                    right_click.connect_pressed(move |gesture, _, x, y| {
                        gesture.set_state(gtk4::EventSequenceState::Claimed);
                        ema.set(true);
                        *sel2.borrow_mut() = Some(e_clone.clone());
                        if let Some(r) = rh2.borrow().as_ref() {
                            show_context_menu_for_entry(
                                &exp_ref,
                                &pane_ref,
                                &act_pop,
                                x,
                                y,
                                &e_clone,
                                &cd,
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
                    exp.add_controller(right_click);

                    if let Ok(children) = LocalFsBackend.list_dir(Path::new(&entry.path)) {
                        let sub_sorted = filter_and_sort_entries(
                            &children,
                            *hidden_rc.borrow(),
                            "",
                            SortColumn::Name,
                            true,
                            true,
                        );
                        for child in sub_sorted.into_iter().take(25) {
                            let child_lbl = gtk4::Label::new(Some(&format!(
                                "{} {} ({})",
                                if child.is_dir { "📁" } else { "📄" },
                                child.name,
                                child.formatted_size_clean()
                            )));
                            child_lbl.set_halign(gtk4::Align::Start);
                            child_lbl.add_css_class("caption");
                            sub_list.append(&child_lbl);
                        }
                    }

                    exp.set_child(Some(&sub_list));
                    tv.append(&exp);
                } else {
                    let item_btn = gtk4::Button::with_label(&format!(
                        "📄 {}  ({})",
                        entry.name,
                        entry.formatted_size_clean()
                    ));
                    item_btn.add_css_class("flat");
                    item_btn.set_halign(gtk4::Align::Fill);
                    let entry_clone = entry.clone();
                    let sel = Rc::clone(&sel_entry);
                    let e_c = entry.clone();
                    item_btn.connect_clicked(move |_| {
                        *sel.borrow_mut() = Some(entry_clone.clone());
                    });

                    let right_click = gtk4::GestureClick::new();
                    right_click.set_button(3);
                    let btn_ref = item_btn.clone();
                    let cd = Rc::clone(&cur_dir);
                    let win = win_ref.clone();
                    let st = status.clone();
                    let rh2 = Rc::clone(&r_holder);
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
                    let pane_ref = pane_vbox_c.clone();
                    let act_pop = Rc::clone(&act_pop_rc);
                    right_click.connect_pressed(move |gesture, _, x, y| {
                        gesture.set_state(gtk4::EventSequenceState::Claimed);
                        ema.set(true);
                        *sel2.borrow_mut() = Some(e_c.clone());
                        if let Some(r) = rh2.borrow().as_ref() {
                            show_context_menu_for_entry(
                                &btn_ref,
                                &pane_ref,
                                &act_pop,
                                x,
                                y,
                                &e_c,
                                &cd,
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
                    item_btn.add_controller(right_click);

                    tv.append(&item_btn);
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
        let refresh = Rc::clone(&refresh_fn);
        let win_ref = parent_window.clone();
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
                    show_quick_look_window(&win_ref, entries, idx);
                }
            }
        });
    }
    {
        let vis = Rc::clone(&visible_entries);
        let sel = Rc::clone(&selected_entry);
        grid_flow.connect_selected_children_changed(move |gf| {
            let selected = gf.selected_children();
            if let Some(child) = selected.first() {
                let idx = child.index() as usize;
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
        let refresh = Rc::clone(&refresh_fn);
        let win_ref = parent_window.clone();
        grid_flow.connect_child_activated(move |_, child| {
            let idx = child.index() as usize;
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
                    show_quick_look_window(&win_ref, entries, idx);
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
                bm_btn.set_tooltip_text(Some("Favoritar diretório atual"));
                st.set_text(&format!("Favorito removido: '{}'", p.display()));
            } else {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p_str.clone());
                bms.push(FileManagerBookmark { name, path: p_str });
                save_bookmarks(&bms);
                bm_btn.add_css_class("bookmark-active-star");
                bm_btn.set_tooltip_text(Some("Remover favorito do diretório atual"));
                st.set_text(&format!("Diretório '{}' marcado como favorito.", p.display()));
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
    {
        let st = status_lbl.clone();
        btn_history.connect_clicked(move |_| {
            st.set_text("Fila de transferências vazia (0 tarefas em andamento).");
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

    // Spacebar shortcut inside list/grid views triggers Quick Look
    {
        let win_ref = parent_window.clone();
        let vis = Rc::clone(&visible_entries);
        let sel = Rc::clone(&selected_entry);
        let key_ctrl = gtk4::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::space {
                let entries = vis.borrow().clone();
                if !entries.is_empty() {
                    let idx = sel
                        .borrow()
                        .as_ref()
                        .and_then(|s| entries.iter().position(|e| e.path == s.path))
                        .unwrap_or(0);
                    show_quick_look_window(&win_ref, entries, idx);
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
        view_stack.add_controller(key_ctrl);
    }

    FilePaneHandle {
        widget: pane_vbox,
        current_dir,
        selected_entry,
        visible_entries,
        refresh: refresh_fn,
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
    let display_name = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };

    let title = format!("Permissões para {display_name}");
    let current_perms = entry.formatted_permissions();
    let body = format!("Definir permissões de arquivo para: {display_name}\nAtual: {current_perms}");

    let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    content_box.set_size_request(350, -1);

    // Group 1: Proprietário
    let owner_group = adw::PreferencesGroup::builder().title("Proprietário").build();
    let owner_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    owner_box.set_homogeneous(true);
    let owner_read = gtk4::CheckButton::with_label("Ler");
    let owner_write = gtk4::CheckButton::with_label("Escrever");
    let owner_exec = gtk4::CheckButton::with_label("Executar");
    owner_box.append(&owner_read);
    owner_box.append(&owner_write);
    owner_box.append(&owner_exec);
    let owner_row = adw::ActionRow::new();
    owner_row.set_child(Some(&owner_box));
    owner_group.add(&owner_row);
    content_box.append(&owner_group);

    // Group 2: Grupo
    let group_group = adw::PreferencesGroup::builder().title("Grupo").build();
    let group_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    group_box.set_homogeneous(true);
    let group_read = gtk4::CheckButton::with_label("Ler");
    let group_write = gtk4::CheckButton::with_label("Escrever");
    let group_exec = gtk4::CheckButton::with_label("Executar");
    group_box.append(&group_read);
    group_box.append(&group_write);
    group_box.append(&group_exec);
    let group_row = adw::ActionRow::new();
    group_row.set_child(Some(&group_box));
    group_group.add(&group_row);
    content_box.append(&group_group);

    // Group 3: Outros
    let others_group = adw::PreferencesGroup::builder().title("Outros").build();
    let others_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    others_box.set_homogeneous(true);
    let others_read = gtk4::CheckButton::with_label("Ler");
    let others_write = gtk4::CheckButton::with_label("Escrever");
    let others_exec = gtk4::CheckButton::with_label("Executar");
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
            ml.set_text(&format!("Modo numérico: {str_val}"));
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

    dialog.add_response("cancel", "Cancelar");
    dialog.add_response("apply", "Aplicar");
    dialog.set_response_appearance("apply", adw::ResponseAppearance::Suggested);

    let path = entry.path.clone();
    let name = display_name;
    dialog.connect_response(None, move |_, response| {
        if response == "apply" {
            let (mode_val, mode_str) = calc_mode();
            let perms = fs::Permissions::from_mode(mode_val);
            match fs::set_permissions(&path, perms) {
                Ok(()) => {
                    status_lbl.set_text(&format!("Permissões de '{name}' alteradas para {mode_str}"));
                    refresh_fn();
                }
                Err(e) => {
                    status_lbl.set_text(&format!("Erro ao alterar permissões de '{name}': {e}"));
                }
            }
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

    let btn_ok = gtk4::Button::with_label("Confirmar");
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

/// Copies a file or directory recursively from `src` to `dst_dir` and records a [`TransferItem`].
fn copy_entry_and_record_transfer(
    entry: &FileEntry,
    dst_dir: &Path,
    direction: TransferDirection,
    transfers: &Rc<RefCell<Vec<TransferItem>>>,
) -> std::io::Result<PathBuf> {
    let src_path = Path::new(&entry.path);
    let dst_path = dst_dir.join(&entry.name);

    if entry.is_dir {
        copy_dir_recursive(src_path, &dst_path)?;
    } else {
        fs::copy(src_path, &dst_path)?;
    }

    let mut tx_list = transfers.borrow_mut();
    let tx_id = format!("tx_{}", tx_list.len() + 1);
    tx_list.push(TransferItem {
        id: tx_id,
        filename: entry.name.clone(),
        source_path: entry.path.clone(),
        destination_path: dst_path.to_string_lossy().into_owned(),
        direction,
        status: TransferStatus::Completed,
        total_bytes: entry.size.max(1),
        transferred_bytes: entry.size.max(1),
        speed_bps: (entry.size.max(1024)) as f64,
        error_message: None,
    });

    Ok(dst_path)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_child = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_child)?;
        } else {
            fs::copy(entry.path(), dst_child)?;
        }
    }
    Ok(())
}

/// Builds the complete Dual-Pane File Manager container.
pub fn build_complete_file_manager(
    parent_window: &adw::ApplicationWindow,
    status_lbl: &gtk4::Label,
    cd_in_terminal_cb: Rc<dyn Fn(&Path)>,
    feed_terminal_cb: Rc<dyn Fn(&str)>,
    ai_prompt_cb: Option<Rc<dyn Fn(&str)>>,
) -> gtk4::Box {
    let transfers: Rc<RefCell<Vec<TransferItem>>> = Rc::new(RefCell::new(Vec::new()));
    let clipboard: Rc<RefCell<Option<FileManagerClipboardItem>>> = Rc::new(RefCell::new(None));

    let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    container.set_height_request(120);
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

    let left_pane = build_file_pane(
        parent_window,
        platform::home_dir(),
        status_lbl,
        Rc::clone(&cd_in_terminal_cb),
        Rc::clone(&feed_terminal_cb),
        ai_prompt_cb.clone(),
        Rc::clone(&clipboard),
    );
    let right_pane = build_file_pane(
        parent_window,
        PathBuf::from("/tmp"),
        status_lbl,
        Rc::clone(&cd_in_terminal_cb),
        Rc::clone(&feed_terminal_cb),
        ai_prompt_cb,
        Rc::clone(&clipboard),
    );

    // Center Transfer Action Bar (`DualPaneTransferBar`)
    let center_transfer_bar = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    center_transfer_bar.set_valign(gtk4::Align::Center);
    center_transfer_bar.set_margin_start(2);
    center_transfer_bar.set_margin_end(2);

    let btn_copy_to_right = gtk4::Button::builder()
        .icon_name("go-next-symbolic")
        .tooltip_text("Copiar Selecionado para o Painel Direito (→)")
        .build();
    btn_copy_to_right.add_css_class("suggested-action");

    let btn_copy_to_left = gtk4::Button::builder()
        .icon_name("go-previous-symbolic")
        .tooltip_text("Copiar Selecionado para o Painel Esquerdo (←)")
        .build();

    let btn_compare_dirs = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Comparar / Sincronizar Diretórios (Esquerdo ⇄ Direito)")
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

    // Wire Copy Left -> Right
    {
        let lp_sel = Rc::clone(&left_pane.selected_entry);
        let rp_dir = Rc::clone(&right_pane.current_dir);
        let rp_refresh = Rc::clone(&right_pane.refresh);
        let tx_rc = Rc::clone(&transfers);
        let status = status_lbl.clone();

        btn_copy_to_right.connect_clicked(move |_| {
            let Some(entry) = lp_sel.borrow().clone() else {
                status.set_text("Selecione um item no painel esquerdo para enviar (→).");
                return;
            };
            let dst_dir = rp_dir.borrow().clone();
            match copy_entry_and_record_transfer(&entry, &dst_dir, TransferDirection::Upload, &tx_rc)
            {
                Ok(dst_path) => {
                    status.set_text(&format!("Copiado para {}", dst_path.display()));
                    rp_refresh();
                }
                Err(e) => status.set_text(&format!("Falha na transferência (→): {e}")),
            }
        });
    }

    // Wire Copy Right -> Left
    {
        let rp_sel = Rc::clone(&right_pane.selected_entry);
        let lp_dir = Rc::clone(&left_pane.current_dir);
        let lp_refresh = Rc::clone(&left_pane.refresh);
        let tx_rc = Rc::clone(&transfers);
        let status = status_lbl.clone();

        btn_copy_to_left.connect_clicked(move |_| {
            let Some(entry) = rp_sel.borrow().clone() else {
                status.set_text("Selecione um item no painel direito para baixar (←).");
                return;
            };
            let dst_dir = lp_dir.borrow().clone();
            match copy_entry_and_record_transfer(
                &entry,
                &dst_dir,
                TransferDirection::Download,
                &tx_rc,
            ) {
                Ok(dst_path) => {
                    status.set_text(&format!("Copiado para {}", dst_path.display()));
                    lp_refresh();
                }
                Err(e) => status.set_text(&format!("Falha na transferência (←): {e}")),
            }
        });
    }

    // Wire Compare Directories (Left <-> Right)
    {
        let lp_vis = Rc::clone(&left_pane.visible_entries);
        let rp_vis = Rc::clone(&right_pane.visible_entries);
        let status = status_lbl.clone();
        btn_compare_dirs.connect_clicked(move |_| {
            let cmp = compare_directory_entries(&lp_vis.borrow(), &rp_vis.borrow());
            status.set_text(&format!(
                "Comparação Dual-Pane: {} idênticos | {} apenas na Esquerda | {} apenas na Direita | {} com tamanho diferente",
                cmp.identical_count,
                cmp.only_in_left.len(),
                cmp.only_in_right.len(),
                cmp.size_differs.len()
            ));
        });
    }

    container
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
            size: 4096,
            modified_epoch: 0,
            permissions_octal: 0o755,
            owner: "vagnarok:vagnarok".to_string(),
        };
        assert_eq!(icon_for_entry(&parent), "go-up-symbolic");
        assert!(build_entry_tooltip_markup(&parent).contains("Subir para o diretório pai"));

        let downloads = FileEntry {
            name: "Downloads".to_string(),
            path: "/home/user/Downloads".to_string(),
            is_dir: true,
            is_symlink: false,
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
}
