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
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
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
use std::cell::RefCell;
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

/// Chooses a symbolic icon name for a [`FileEntry`].
fn icon_for_entry(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        return "folder-symbolic";
    }
    let ext = Path::new(&entry.name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => {
            "image-x-generic-symbolic"
        }
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "deb" | "rpm" => {
            "package-x-generic-symbolic"
        }
        "rs" | "py" | "sh" | "js" | "ts" | "c" | "cpp" | "h" | "go" | "json" | "toml" | "yaml"
        | "yml" | "md" => "text-x-script-symbolic",
        "pdf" => "x-office-document-symbolic",
        "mp3" | "wav" | "ogg" | "flac" => "audio-x-generic-symbolic",
        "mp4" | "mkv" | "webm" => "video-x-generic-symbolic",
        _ => "text-x-generic-symbolic",
    }
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
            while let Some(child) = lb.first_child() {
                lb.remove(&child);
            }
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
}

/// Builds a rich File Manager Pane with:
/// - Navigation toolbar (`Home`, `Up`, `Refresh`, `New Folder`, clickable Breadcrumbs, Path entry, Hidden toggle, Search)
/// - View Mode Switcher (`List`, `Grid` with thumbnails, `Tree` with inline expanders)
/// - File Action Bar (`Quick Look`, `Rename`, `Chmod`, `SHA-256`, `Delete`, `CD in Terminal`)
pub fn build_file_pane(
    parent_window: &adw::ApplicationWindow,
    initial_dir: PathBuf,
    status_lbl: &gtk4::Label,
    cd_in_terminal_cb: Rc<dyn Fn(&Path)>,
) -> FilePaneHandle {
    let current_dir = Rc::new(RefCell::new(initial_dir));
    let selected_entry: Rc<RefCell<Option<FileEntry>>> = Rc::new(RefCell::new(None));
    let visible_entries: Rc<RefCell<Vec<FileEntry>>> = Rc::new(RefCell::new(Vec::new()));
    let show_hidden = Rc::new(RefCell::new(false));
    let sort_col = Rc::new(RefCell::new(SortColumn::Name));
    let sort_asc = Rc::new(RefCell::new(true));
    let view_mode = Rc::new(RefCell::new(FileManagerViewMode::List));

    let pane_vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    pane_vbox.set_hexpand(true);
    pane_vbox.set_vexpand(true);

    // ── 1. Top Navigation & View Mode Toolbar ─────────────────────────────────
    let nav_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);

    let nav_btns = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    nav_btns.add_css_class("linked");

    let btn_home = gtk4::Button::builder()
        .icon_name("go-home-symbolic")
        .tooltip_text("Pasta Pessoal (Home)")
        .build();
    let btn_up = gtk4::Button::builder()
        .icon_name("go-up-symbolic")
        .tooltip_text("Subir Nível (..)")
        .build();
    let btn_refresh = gtk4::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Atualizar (F5)")
        .build();
    let btn_mkdir = gtk4::Button::builder()
        .icon_name("folder-new-symbolic")
        .tooltip_text("Nova Pasta")
        .build();

    nav_btns.append(&btn_home);
    nav_btns.append(&btn_up);
    nav_btns.append(&btn_refresh);
    nav_btns.append(&btn_mkdir);
    nav_bar.append(&nav_btns);

    // Clickable Breadcrumbs Scrolled Box
    let breadcrumb_scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .hexpand(true)
        .build();
    let breadcrumb_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 2);
    breadcrumb_box.add_css_class("linked");
    breadcrumb_scroll.set_child(Some(&breadcrumb_box));
    nav_bar.append(&breadcrumb_scroll);

    // Hidden files toggle
    let btn_hidden = gtk4::ToggleButton::builder()
        .icon_name("view-reveal-symbolic")
        .tooltip_text("Mostrar Arquivos Ocultos (.dotfiles)")
        .build();
    nav_bar.append(&btn_hidden);

    // View Mode Switcher (List / Grid / Tree)
    let mode_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    mode_box.add_css_class("linked");
    let btn_mode_list = gtk4::ToggleButton::builder()
        .icon_name("view-list-symbolic")
        .tooltip_text("Visualização em Lista Detalhada")
        .active(true)
        .build();
    let btn_mode_grid = gtk4::ToggleButton::builder()
        .icon_name("view-grid-symbolic")
        .tooltip_text("Visualização em Grade / Miniaturas")
        .group(&btn_mode_list)
        .build();
    let btn_mode_tree = gtk4::ToggleButton::builder()
        .icon_name("view-list-tree-symbolic")
        .tooltip_text("Visualização em Árvore")
        .group(&btn_mode_list)
        .build();
    mode_box.append(&btn_mode_list);
    mode_box.append(&btn_mode_grid);
    mode_box.append(&btn_mode_tree);
    nav_bar.append(&mode_box);

    pane_vbox.append(&nav_bar);

    // ── 2. Secondary Bar: Direct Path Entry + Search Filter + File Actions ───
    let sub_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);

    let path_entry = gtk4::Entry::builder()
        .placeholder_text("/caminho/direto...")
        .hexpand(true)
        .build();
    path_entry.add_css_class("monospace");
    sub_bar.append(&path_entry);

    let search_entry = gtk4::SearchEntry::builder()
        .placeholder_text("Filtrar arquivos...")
        .width_chars(16)
        .build();
    sub_bar.append(&search_entry);

    let ops_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    ops_box.add_css_class("linked");

    let btn_quick_look = gtk4::Button::builder()
        .icon_name("system-search-symbolic")
        .tooltip_text("Quick Look Preview (Espaço)")
        .build();
    let btn_rename = gtk4::Button::builder()
        .icon_name("document-edit-symbolic")
        .tooltip_text("Renomear Selecionado")
        .build();
    let btn_chmod = gtk4::Button::builder()
        .icon_name("dialog-password-symbolic")
        .tooltip_text("Alterar Permissões (chmod)")
        .build();
    let btn_checksum = gtk4::Button::builder()
        .icon_name("security-high-symbolic")
        .tooltip_text("Calcular Checksum SHA-256")
        .build();
    let btn_cd_term = gtk4::Button::builder()
        .icon_name("utilities-terminal-symbolic")
        .tooltip_text("Abrir Pasta no Terminal Ativo (cd)")
        .build();
    let btn_delete = gtk4::Button::builder()
        .icon_name("user-trash-symbolic")
        .tooltip_text("Excluir Selecionado")
        .build();
    btn_delete.add_css_class("destructive-action");

    ops_box.append(&btn_quick_look);
    ops_box.append(&btn_rename);
    ops_box.append(&btn_chmod);
    ops_box.append(&btn_checksum);
    ops_box.append(&btn_cd_term);
    ops_box.append(&btn_delete);
    sub_bar.append(&ops_box);

    pane_vbox.append(&sub_bar);

    // ── 3. Sortable Column Header Bar (Shown in List Mode) ───────────────────
    let col_header_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    col_header_box.set_margin_start(6);
    col_header_box.set_margin_end(6);

    let btn_sort_name = gtk4::Button::builder()
        .label("Nome ↑")
        .hexpand(true)
        .halign(gtk4::Align::Fill)
        .build();
    btn_sort_name.add_css_class("flat");

    let btn_sort_size = gtk4::Button::builder()
        .label("Tamanho")
        .width_request(90)
        .build();
    btn_sort_size.add_css_class("flat");

    let btn_sort_perm = gtk4::Label::new(Some("Permissões"));
    btn_sort_perm.add_css_class("caption");
    btn_sort_perm.add_css_class("dim-label");
    btn_sort_perm.set_width_request(95);

    let btn_sort_mod = gtk4::Button::builder()
        .label("Modificado")
        .width_request(130)
        .build();
    btn_sort_mod.add_css_class("flat");

    col_header_box.append(&btn_sort_name);
    col_header_box.append(&btn_sort_size);
    col_header_box.append(&btn_sort_perm);
    col_header_box.append(&btn_sort_mod);
    pane_vbox.append(&col_header_box);

    // ── 4. `gtk4::Stack` with 3 Views: `list`, `grid`, `tree` ─────────────────
    let view_stack = gtk4::Stack::new();
    view_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    view_stack.set_hexpand(true);
    view_stack.set_vexpand(true);

    // 4.1 List View
    let list_scroll = gtk4::ScrolledWindow::new();
    list_scroll.set_hexpand(true);
    list_scroll.set_vexpand(true);
    let list_box = gtk4::ListBox::new();
    list_box.set_selection_mode(gtk4::SelectionMode::Single);
    list_box.add_css_class("boxed-list");
    list_scroll.set_child(Some(&list_box));
    view_stack.add_named(&list_scroll, Some("list"));

    // 4.2 Grid / Thumbnail View
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
    grid_scroll.set_child(Some(&grid_flow));
    view_stack.add_named(&grid_scroll, Some("grid"));

    // 4.3 Tree View
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

    // ── 5. Master Refresh Closure ─────────────────────────────────────────────
    let refresh_holder: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let refresh_fn: Rc<dyn Fn()> = {
        let cur_dir = Rc::clone(&current_dir);
        let sel_entry = Rc::clone(&selected_entry);
        let vis_entries = Rc::clone(&visible_entries);
        let hidden_rc = Rc::clone(&show_hidden);
        let scol_rc = Rc::clone(&sort_col);
        let sasc_rc = Rc::clone(&sort_asc);
        let bc_box = breadcrumb_box.clone();
        let pe = path_entry.clone();
        let se = search_entry.clone();
        let lb = list_box.clone();
        let gf = grid_flow.clone();
        let tv = tree_vbox.clone();
        let b_name = btn_sort_name.clone();
        let b_size = btn_sort_size.clone();
        let b_mod = btn_sort_mod.clone();
        let status = status_lbl.clone();
        let r_holder = Rc::clone(&refresh_holder);

        Rc::new(move || {
            let dir_path = cur_dir.borrow().clone();
            pe.set_text(&dir_path.to_string_lossy());

            // Update sort column button labels
            let col = *scol_rc.borrow();
            let asc = *sasc_rc.borrow();
            let arrow = if asc { "↑" } else { "↓" };
            let name_txt = if col == SortColumn::Name {
                format!("Nome {arrow}")
            } else {
                "Nome".to_string()
            };
            b_name.set_label(&name_txt);

            let size_txt = if col == SortColumn::Size {
                format!("Tamanho {arrow}")
            } else {
                "Tamanho".to_string()
            };
            b_size.set_label(&size_txt);

            let mod_txt = if col == SortColumn::Modified {
                format!("Modificado {arrow}")
            } else {
                "Modificado".to_string()
            };
            b_mod.set_label(&mod_txt);

            // Rebuild Breadcrumbs
            while let Some(c) = bc_box.first_child() {
                bc_box.remove(&c);
            }
            for (seg_name, seg_target) in build_breadcrumb_segments(&dir_path) {
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
            }

            // Load entries from LocalFsBackend
            let backend = LocalFsBackend;
            let raw_entries = match backend.list_dir(&dir_path) {
                Ok(list) => list,
                Err(e) => {
                    status.set_text(&format!("Erro ao listar {}: {e}", dir_path.display()));
                    Vec::new()
                }
            };

            let query = se.text().to_string();
            let sorted = filter_and_sort_entries(
                &raw_entries,
                *hidden_rc.borrow(),
                &query,
                col,
                asc,
                true,
            );
            *vis_entries.borrow_mut() = sorted.clone();
            *sel_entry.borrow_mut() = sorted.first().cloned();

            // 1. Populate Detailed List View (`list_box`)
            while let Some(c) = lb.first_child() {
                lb.remove(&c);
            }
            for entry in sorted.iter().take(300) {
                let row_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
                row_hbox.set_margin_start(8);
                row_hbox.set_margin_end(8);
                row_hbox.set_margin_top(4);
                row_hbox.set_margin_bottom(4);

                let icon = gtk4::Image::from_icon_name(icon_for_entry(entry));
                let name_lbl = gtk4::Label::new(Some(&entry.name));
                name_lbl.set_halign(gtk4::Align::Start);
                name_lbl.set_hexpand(true);
                name_lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                if entry.is_dir {
                    name_lbl.add_css_class("heading");
                }

                let size_lbl = gtk4::Label::new(Some(&entry.formatted_size()));
                size_lbl.add_css_class("monospace");
                size_lbl.add_css_class("dim-label");
                size_lbl.set_width_request(85);
                size_lbl.set_halign(gtk4::Align::End);

                let perm_lbl = gtk4::Label::new(Some(&entry.formatted_permissions()));
                perm_lbl.add_css_class("monospace");
                perm_lbl.add_css_class("caption");
                perm_lbl.add_css_class("dim-label");
                perm_lbl.set_width_request(95);

                let mod_lbl = gtk4::Label::new(Some(&format_epoch_short(entry.modified_epoch)));
                mod_lbl.add_css_class("monospace");
                mod_lbl.add_css_class("caption");
                mod_lbl.add_css_class("dim-label");
                mod_lbl.set_width_request(125);

                row_hbox.append(&icon);
                row_hbox.append(&name_lbl);
                row_hbox.append(&size_lbl);
                row_hbox.append(&perm_lbl);
                row_hbox.append(&mod_lbl);

                if entry.is_dir {
                    let open_btn = gtk4::Button::builder()
                        .icon_name("go-next-symbolic")
                        .tooltip_text("Entrar na pasta")
                        .build();
                    open_btn.add_css_class("flat");
                    let target_path = PathBuf::from(&entry.path);
                    let c_dir = Rc::clone(&cur_dir);
                    let rh = Rc::clone(&r_holder);
                    open_btn.connect_clicked(move |_| {
                        *c_dir.borrow_mut() = target_path.clone();
                        if let Some(r) = rh.borrow().as_ref() {
                            r();
                        }
                    });
                    row_hbox.append(&open_btn);
                }

                lb.append(&row_hbox);
            }

            // 2. Populate Thumbnail / Grid View (`grid_flow`)
            while let Some(c) = gf.first_child() {
                gf.remove(&c);
            }
            for entry in sorted.iter().take(150) {
                let card = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
                card.set_width_request(104);
                card.set_height_request(104);
                card.set_margin_start(4);
                card.set_margin_end(4);
                card.set_margin_top(4);
                card.set_margin_bottom(4);
                card.set_halign(gtk4::Align::Center);

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
                card.append(&name_lbl);

                let sz_lbl = gtk4::Label::new(Some(&entry.formatted_size()));
                sz_lbl.add_css_class("caption");
                sz_lbl.add_css_class("dim-label");
                card.append(&sz_lbl);

                gf.append(&card);
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

                    if let Ok(children) = backend.list_dir(Path::new(&entry.path)) {
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
                                child.formatted_size()
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
                        entry.formatted_size()
                    ));
                    item_btn.add_css_class("flat");
                    item_btn.set_halign(gtk4::Align::Fill);
                    let entry_clone = entry.clone();
                    let sel = Rc::clone(&sel_entry);
                    item_btn.connect_clicked(move |_| {
                        *sel.borrow_mut() = Some(entry_clone.clone());
                    });
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
                if entry.is_dir {
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
        let cur_dir = Rc::clone(&current_dir);
        let refresh = Rc::clone(&refresh_fn);
        grid_flow.connect_child_activated(move |_, child| {
            let idx = child.index() as usize;
            let entries = vis.borrow().clone();
            if let Some(entry) = entries.get(idx) {
                *sel.borrow_mut() = Some(entry.clone());
                if entry.is_dir {
                    *cur_dir.borrow_mut() = PathBuf::from(&entry.path);
                    refresh();
                }
            }
        });
    }

    // ── 7. Wire Navigation & View Mode Controls ───────────────────────────────
    {
        let cur_dir = Rc::clone(&current_dir);
        let refresh = Rc::clone(&refresh_fn);
        btn_home.connect_clicked(move |_| {
            *cur_dir.borrow_mut() = platform::home_dir();
            refresh();
        });
    }
    {
        let cur_dir = Rc::clone(&current_dir);
        let refresh = Rc::clone(&refresh_fn);
        btn_up.connect_clicked(move |_| {
            let parent = cur_dir.borrow().parent().map(Path::to_path_buf);
            if let Some(p) = parent {
                *cur_dir.borrow_mut() = p;
                refresh();
            }
        });
    }
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
        let refresh = Rc::clone(&refresh_fn);
        search_entry.connect_search_changed(move |_| refresh());
    }
    {
        let cur_dir = Rc::clone(&current_dir);
        let refresh = Rc::clone(&refresh_fn);
        let status = status_lbl.clone();
        path_entry.connect_activate(move |entry| {
            let candidate = PathBuf::from(entry.text().trim());
            if candidate.is_dir() {
                *cur_dir.borrow_mut() = candidate;
                refresh();
            } else {
                status.set_text(&format!("Diretório não encontrado: {}", candidate.display()));
            }
        });
    }

    // View Mode Switching (`list` / `grid` / `tree`)
    {
        let vs = view_stack.clone();
        let ch = col_header_box.clone();
        let vm = Rc::clone(&view_mode);
        btn_mode_list.connect_toggled(move |btn| {
            if btn.is_active() {
                *vm.borrow_mut() = FileManagerViewMode::List;
                vs.set_visible_child_name(FileManagerViewMode::List.as_stack_name());
                ch.set_visible(true);
            }
        });
    }
    {
        let vs = view_stack.clone();
        let ch = col_header_box.clone();
        let vm = Rc::clone(&view_mode);
        btn_mode_grid.connect_toggled(move |btn| {
            if btn.is_active() {
                *vm.borrow_mut() = FileManagerViewMode::Grid;
                vs.set_visible_child_name(FileManagerViewMode::Grid.as_stack_name());
                ch.set_visible(false);
            }
        });
    }
    {
        let vs = view_stack.clone();
        let ch = col_header_box.clone();
        let vm = Rc::clone(&view_mode);
        btn_mode_tree.connect_toggled(move |btn| {
            if btn.is_active() {
                *vm.borrow_mut() = FileManagerViewMode::Tree;
                vs.set_visible_child_name(FileManagerViewMode::Tree.as_stack_name());
                ch.set_visible(false);
            }
        });
    }

    // Sort Column Header Clicks
    {
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        let refresh = Rc::clone(&refresh_fn);
        btn_sort_name.connect_clicked(move |_| {
            if *scol.borrow() == SortColumn::Name {
                let next = !*sasc.borrow();
                *sasc.borrow_mut() = next;
            } else {
                *scol.borrow_mut() = SortColumn::Name;
                *sasc.borrow_mut() = true;
            }
            refresh();
        });
    }
    {
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        let refresh = Rc::clone(&refresh_fn);
        btn_sort_size.connect_clicked(move |_| {
            if *scol.borrow() == SortColumn::Size {
                let next = !*sasc.borrow();
                *sasc.borrow_mut() = next;
            } else {
                *scol.borrow_mut() = SortColumn::Size;
                *sasc.borrow_mut() = false;
            }
            refresh();
        });
    }
    {
        let scol = Rc::clone(&sort_col);
        let sasc = Rc::clone(&sort_asc);
        let refresh = Rc::clone(&refresh_fn);
        btn_sort_mod.connect_clicked(move |_| {
            if *scol.borrow() == SortColumn::Modified {
                let next = !*sasc.borrow();
                *sasc.borrow_mut() = next;
            } else {
                *scol.borrow_mut() = SortColumn::Modified;
                *sasc.borrow_mut() = false;
            }
            refresh();
        });
    }

    // ── 8. Wire File Operations (New Folder, Quick Look, Rename, Chmod, SHA-256, Delete, CD) ──
    {
        let win_ref = parent_window.clone();
        let vis = Rc::clone(&visible_entries);
        let sel = Rc::clone(&selected_entry);
        let open_ql = move || {
            let entries = vis.borrow().clone();
            if entries.is_empty() {
                return;
            }
            let idx = sel
                .borrow()
                .as_ref()
                .and_then(|s| entries.iter().position(|e| e.path == s.path))
                .unwrap_or(0);
            show_quick_look_window(&win_ref, entries, idx);
        };
        let open_ql_rc = Rc::new(open_ql);
        let ql1 = Rc::clone(&open_ql_rc);
        btn_quick_look.connect_clicked(move |_| ql1());

        // Spacebar shortcut inside list/grid views triggers Quick Look
        let key_ctrl = gtk4::EventControllerKey::new();
        let ql2 = Rc::clone(&open_ql_rc);
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::space {
                ql2();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        view_stack.add_controller(key_ctrl);
    }

    // New Folder dialog
    {
        let win_ref = parent_window.clone();
        let cur_dir = Rc::clone(&current_dir);
        let refresh = Rc::clone(&refresh_fn);
        let status = status_lbl.clone();
        btn_mkdir.connect_clicked(move |_| {
            show_simple_input_dialog(
                &win_ref,
                "Nova Pasta",
                "Nome da nova pasta:",
                "nova_pasta",
                {
                    let cur_dir = Rc::clone(&cur_dir);
                    let refresh = Rc::clone(&refresh);
                    let status = status.clone();
                    Rc::new(move |folder_name| {
                        let target = cur_dir.borrow().join(folder_name.trim());
                        match LocalFsBackend.mkdir(&target) {
                            Ok(()) => {
                                status.set_text(&format!("Pasta criada: {}", target.display()));
                                refresh();
                            }
                            Err(e) => {
                                status.set_text(&format!("Erro ao criar pasta: {e}"));
                            }
                        }
                    })
                },
            );
        });
    }

    // Rename dialog
    {
        let win_ref = parent_window.clone();
        let sel = Rc::clone(&selected_entry);
        let refresh = Rc::clone(&refresh_fn);
        let status = status_lbl.clone();
        btn_rename.connect_clicked(move |_| {
            let Some(entry) = sel.borrow().clone() else {
                status.set_text("Selecione um arquivo ou pasta para renomear.");
                return;
            };
            show_simple_input_dialog(
                &win_ref,
                "Renomear Item",
                "Novo nome:",
                &entry.name,
                {
                    let refresh = Rc::clone(&refresh);
                    let status = status.clone();
                    Rc::new(move |new_name| {
                        let old_path = PathBuf::from(&entry.path);
                        let Some(parent) = old_path.parent() else {
                            return;
                        };
                        let new_path = parent.join(new_name.trim());
                        match LocalFsBackend.rename(&old_path, &new_path) {
                            Ok(()) => {
                                status.set_text(&format!("Renomeado para '{}'", new_name.trim()));
                                refresh();
                            }
                            Err(e) => status.set_text(&format!("Erro ao renomear: {e}")),
                        }
                    })
                },
            );
        });
    }

    // Chmod dialog
    {
        let win_ref = parent_window.clone();
        let sel = Rc::clone(&selected_entry);
        let refresh = Rc::clone(&refresh_fn);
        let status = status_lbl.clone();
        btn_chmod.connect_clicked(move |_| {
            let Some(entry) = sel.borrow().clone() else {
                status.set_text("Selecione um item para alterar permissões.");
                return;
            };
            let current_octal = format!("{:o}", entry.permissions_octal & 0o777);
            show_simple_input_dialog(
                &win_ref,
                "Alterar Permissões (chmod)",
                "Modo Octal (ex: 755, 644, 600):",
                &current_octal,
                {
                    let refresh = Rc::clone(&refresh);
                    let status = status.clone();
                    Rc::new(move |octal_str| {
                        if let Ok(mode) = u32::from_str_radix(octal_str.trim(), 8) {
                            let perms = fs::Permissions::from_mode(mode);
                            match fs::set_permissions(&entry.path, perms) {
                                Ok(()) => {
                                    status.set_text(&format!(
                                        "Permissões de '{}' alteradas para {:o}",
                                        entry.name, mode
                                    ));
                                    refresh();
                                }
                                Err(e) => status.set_text(&format!("Erro no chmod: {e}")),
                            }
                        } else {
                            status.set_text("Modo octal inválido (use ex: 755 ou 644).");
                        }
                    })
                },
            );
        });
    }

    // SHA-256 Checksum action
    {
        let sel = Rc::clone(&selected_entry);
        let status = status_lbl.clone();
        btn_checksum.connect_clicked(move |_| {
            let Some(entry) = sel.borrow().clone() else {
                status.set_text("Selecione um arquivo para calcular o SHA-256.");
                return;
            };
            if entry.is_dir {
                status.set_text("Selecione um arquivo regular para calcular o SHA-256.");
                return;
            }
            match compute_file_sha256_hex(Path::new(&entry.path)) {
                Ok(hex) => {
                    status.set_text(&format!("SHA-256 ({}): {hex}", entry.name));
                }
                Err(e) => status.set_text(&format!("Erro ao calcular SHA-256: {e}")),
            }
        });
    }

    // CD in Active Terminal
    {
        let cur_dir = Rc::clone(&current_dir);
        let cd_cb = Rc::clone(&cd_in_terminal_cb);
        btn_cd_term.connect_clicked(move |_| {
            let dir = cur_dir.borrow().clone();
            cd_cb(&dir);
        });
    }

    // Delete item
    {
        let sel = Rc::clone(&selected_entry);
        let refresh = Rc::clone(&refresh_fn);
        let status = status_lbl.clone();
        btn_delete.connect_clicked(move |_| {
            let Some(entry) = sel.borrow().clone() else {
                status.set_text("Selecione um item para excluir.");
                return;
            };
            match LocalFsBackend.remove(Path::new(&entry.path)) {
                Ok(()) => {
                    status.set_text(&format!("Item removido: {}", entry.name));
                    refresh();
                }
                Err(e) => status.set_text(&format!("Erro ao remover '{}': {e}", entry.name)),
            }
        });
    }

    FilePaneHandle {
        widget: pane_vbox,
        current_dir,
        selected_entry,
        visible_entries,
        refresh: refresh_fn,
    }
}

/// Helper to display a small modal text input dialog for Rename, New Folder, or Chmod.
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

/// Builds the complete Phase 12 Dual-Pane File Manager container.
pub fn build_complete_file_manager(
    parent_window: &adw::ApplicationWindow,
    status_lbl: &gtk4::Label,
    cd_in_terminal_cb: Rc<dyn Fn(&Path)>,
) -> gtk4::Box {
    let transfers: Rc<RefCell<Vec<TransferItem>>> = Rc::new(RefCell::new(Vec::new()));

    let container = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    container.set_height_request(290);
    container.set_margin_start(8);
    container.set_margin_end(8);
    container.set_margin_top(4);
    container.set_margin_bottom(4);

    // ── Header Bar of the File Manager Panel ──────────────────────────────────
    let top_bar = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);

    let title_lbl = gtk4::Label::new(Some(
        "Gerenciador de Arquivos Dual-Pane (Lista / Grade de Miniaturas / Árvore & Quick Look)",
    ));
    title_lbl.add_css_class("heading");
    title_lbl.set_halign(gtk4::Align::Start);
    title_lbl.set_hexpand(true);
    top_bar.append(&title_lbl);

    let btn_toggle_dual = gtk4::ToggleButton::builder()
        .icon_name("view-dual-symbolic")
        .label("Dual-Pane")
        .tooltip_text("Alternar Painel Duplo (Local <-> Secundário/SFTP)")
        .active(true)
        .build();
    top_bar.append(&btn_toggle_dual);

    let btn_transfers = gtk4::Button::builder()
        .icon_name("folder-download-symbolic")
        .label("Transferências (0)")
        .tooltip_text("Abrir Fila de Transferências")
        .build();
    top_bar.append(&btn_transfers);

    container.append(&top_bar);

    // ── Left Pane + Center DualPaneTransferBar + Right Pane ───────────────────
    let main_hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    main_hbox.set_hexpand(true);
    main_hbox.set_vexpand(true);

    let left_pane = build_file_pane(
        parent_window,
        platform::home_dir(),
        status_lbl,
        Rc::clone(&cd_in_terminal_cb),
    );
    let right_pane = build_file_pane(
        parent_window,
        PathBuf::from("/tmp"),
        status_lbl,
        Rc::clone(&cd_in_terminal_cb),
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

    main_hbox.append(&left_pane.widget);
    main_hbox.append(&center_transfer_bar);
    main_hbox.append(&right_pane.widget);

    container.append(&main_hbox);

    // Wire Dual-Pane toggle
    {
        let rp = right_pane.widget.clone();
        let ctb = center_transfer_bar.clone();
        btn_toggle_dual.connect_toggled(move |btn| {
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
        let btn_tx = btn_transfers.clone();
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
                    btn_tx.set_label(&format!("Transferências ({})", tx_rc.borrow().len()));
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
        let btn_tx = btn_transfers.clone();
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
                    btn_tx.set_label(&format!("Transferências ({})", tx_rc.borrow().len()));
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

    // Wire Transfer Queue button
    {
        let win_ref = parent_window.clone();
        let tx_rc = Rc::clone(&transfers);
        btn_transfers.connect_clicked(move |_| {
            show_transfer_queue_dialog(&win_ref, &tx_rc);
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
            },
            FileEntry {
                name: "changed.cfg".to_string(),
                path: "/l/changed.cfg".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 200,
                modified_epoch: 0,
                permissions_octal: 0o644,
            },
            FileEntry {
                name: "left_only.sh".to_string(),
                path: "/l/left_only.sh".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 50,
                modified_epoch: 0,
                permissions_octal: 0o755,
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
            },
            FileEntry {
                name: "changed.cfg".to_string(),
                path: "/r/changed.cfg".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 350,
                modified_epoch: 0,
                permissions_octal: 0o644,
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
}
