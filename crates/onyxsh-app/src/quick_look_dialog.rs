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
use onyxsh_core::platform;
use onyxsh_filemanager::quick_look::{load_local_preview, select_quick_look_entry, PreviewKind};
use onyxsh_filemanager::FileEntry;
use std::cell::RefCell;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

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

/// Helper function to execute elevated root file write using pkexec.
fn save_file_as_root(path: &Path, content: &str) -> Result<(), String> {
    let path_str = path.to_string_lossy().to_string();
    let content_bytes = content.as_bytes();

    let mut cmd = if platform::is_flatpak_sandbox() {
        let mut c = std::process::Command::new("flatpak-spawn");
        c.args(["--host", "pkexec", "tee", &path_str]);
        c
    } else {
        let mut c = std::process::Command::new("pkexec");
        c.args(["tee", &path_str]);
        c
    };

    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| tr_fmt("Falha ao iniciar processo privilegiado pkexec: {err}", &[("err", &e.to_string())]))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content_bytes)
            .map_err(|e| tr_fmt("Falha ao escrever dados no stdin de pkexec: {err}", &[("err", &e.to_string())]))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| tr_fmt("Falha ao aguardar processo pkexec: {err}", &[("err", &e.to_string())]))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(if stderr.trim().is_empty() {
            tr("Operação cancelada ou negada pelo usuário.")
        } else {
            stderr.trim().to_string()
        })
    }
}

/// Opens the modern Quick Look modal preview and in-place editor dialog.
pub fn show_quick_look_dialog(
    parent: &adw::ApplicationWindow,
    entries: Vec<FileEntry>,
    initial_index: usize,
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

    // Packing into headerbar end (order renders: [edit] [save] [save_sudo] [menu] [close])
    header_bar.pack_end(&menu_btn);
    header_bar.pack_end(&save_sudo_btn);
    header_bar.pack_end(&save_btn);
    header_bar.pack_end(&edit_toggle_btn);

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

    // Page: Image View
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
    body_stack.add_named(&image_scroller, Some("image"));

    // Page: Binary / Directory / Metadata Card
    let binary_scroller = gtk4::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    let binary_card = gtk4::Box::new(gtk4::Orientation::Vertical, 14);
    binary_card.set_valign(gtk4::Align::Center);
    binary_card.set_halign(gtk4::Align::Center);
    binary_card.set_margin_top(32);
    binary_card.set_margin_bottom(32);
    binary_scroller.set_child(Some(&binary_card));
    body_stack.add_named(&binary_scroller, Some("binary"));

    toast_overlay.set_child(Some(&body_stack));
    toolbar_view.set_content(Some(&toast_overlay));
    dlg.set_content(Some(&toolbar_view));

    // ── 5. State Tracking ───────────────────────────────────────────────────
    let is_editing_rc = Rc::new(RefCell::new(false));
    let is_dirty_rc = Rc::new(RefCell::new(false));
    let original_content_rc = Rc::new(RefCell::new(String::new()));
    let is_loading_buffer = Rc::new(RefCell::new(false));

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

    // Helper: Perform file save (Normal or Root)
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

        Rc::new(move |as_root: bool, on_success: Option<Rc<dyn Fn()>>| {
            let idx = *idx_rc.borrow();
            let Some(entry) = entries.get(idx) else {
                return;
            };
            let path = Path::new(&entry.path);

            let buffer = tv.buffer();
            let (start, end) = (buffer.start_iter(), buffer.end_iter());
            let current_text = buffer.text(&start, &end, true).to_string();

            if as_root {
                match save_file_as_root(path, &current_text) {
                    Ok(()) => {
                        *dirty_ref.borrow_mut() = false;
                        *orig_ref.borrow_mut() = current_text;
                        sb.set_sensitive(false);
                        up_hdr();
                        if let Some(ref rf) = refresh_cb {
                            rf();
                        }
                        toast(&tr(
                            "Arquivo salvo com sucesso com privilégios de Administrador (Root)!",
                        ));
                        if let Some(ref cb) = on_success {
                            cb();
                        }
                    }
                    Err(e) => {
                        let err_dlg = adw::MessageDialog::builder()
                            .transient_for(&dlg_parent)
                            .heading(&tr("Falha ao Salvar como Root"))
                            .body(&e)
                            .close_response("ok")
                            .build();
                        err_dlg.add_response("ok", &tr("OK"));
                        err_dlg.present();
                    }
                }
            } else {
                match fs::write(path, current_text.as_bytes()) {
                    Ok(()) => {
                        *dirty_ref.borrow_mut() = false;
                        *orig_ref.borrow_mut() = current_text;
                        sb.set_sensitive(false);
                        up_hdr();
                        if let Some(ref rf) = refresh_cb {
                            rf();
                        }
                        toast(&tr("Arquivo salvo com sucesso!"));
                        if let Some(ref cb) = on_success {
                            cb();
                        }
                    }
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::PermissionDenied {
                            let perm_dlg = adw::MessageDialog::builder()
                                .transient_for(&dlg_parent)
                                .heading(&tr("Permissão Negada"))
                                .body(&tr_fmt(
                                    "Você não possui permissão para modificar '{name}'. Deseja salvar utilizando privilégios de Administrador (Root)?",
                                    &[("name", &entry.name)],
                                ))
                                .close_response("cancel")
                                .build();
                            perm_dlg.add_response("cancel", &tr("Cancelar"));
                            perm_dlg.add_response("root_save", &tr("Save as Root"));
                            perm_dlg.set_response_appearance("root_save", adw::ResponseAppearance::Suggested);

                            let root_save_path = path.to_path_buf();
                            let root_text = current_text.clone();
                            let d_ref = Rc::clone(&dirty_ref);
                            let o_ref = Rc::clone(&orig_ref);
                            let sb_c = sb.clone();
                            let up_c = Rc::clone(&up_hdr);
                            let t_c = Rc::clone(&toast);
                            let rf_c = refresh_cb.clone();
                            let parent_w = dlg_parent.clone();

                            perm_dlg.connect_response(None, move |_, response| {
                                if response == "root_save" {
                                    match save_file_as_root(&root_save_path, &root_text) {
                                        Ok(()) => {
                                            *d_ref.borrow_mut() = false;
                                            *o_ref.borrow_mut() = root_text.clone();
                                            sb_c.set_sensitive(false);
                                            up_c();
                                            if let Some(ref rf) = rf_c {
                                                rf();
                                            }
                                            t_c(&tr(
                                                "Arquivo salvo com privilégios de Administrador (Root)!",
                                            ));
                                        }
                                        Err(err_msg) => {
                                            let ed = adw::MessageDialog::builder()
                                                .transient_for(&parent_w)
                                                .heading(&tr("Falha ao Salvar como Root"))
                                                .body(&err_msg)
                                                .close_response("ok")
                                                .build();
                                            ed.add_response("ok", &tr("OK"));
                                            ed.present();
                                        }
                                    }
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
        let discard_dlg = Rc::clone(&show_discard_dialog);

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
                let pop_c = pop.clone();
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
                    let uri = format!("file://{}", p.display());
                    let ok = gio::AppInfo::launch_default_for_uri(&uri, gio::AppLaunchContext::NONE).is_ok();
                    if !ok {
                        let _ = std::process::Command::new("xdg-open").arg(&p).spawn();
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

            // 5. Recarregar do Disco
            {
                let pop_c = pop.clone();
                let p = path_buf.clone();
                let tv_c = tv.clone();
                let is_d = Rc::clone(&dirty_ref);
                let disc_dlg = Rc::clone(&discard_dlg);
                let toast_c = Rc::clone(&toast);

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

                    let reload_action = Rc::new(move || {
                        if let Ok(fresh) = fs::read_to_string(&p2) {
                            tv2.buffer().set_text(&fresh);
                            t2(&tr("Arquivo recarregado do disco."));
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

    // Helper: Render active entry into UI
    let render_entry: Rc<dyn Fn()> = {
        let entries = Rc::clone(&entries_rc);
        let idx_rc = Rc::clone(&current_idx);
        let pos_l = pos_lbl.clone();
        let up_hdr = Rc::clone(&update_title_and_header);
        let stack = body_stack.clone();
        let pic = picture.clone();
        let tv = text_view.clone();
        let bin_card = binary_card.clone();
        let orig_c = Rc::clone(&original_content_rc);
        let dirty_c = Rc::clone(&is_dirty_rc);
        let is_loading = Rc::clone(&is_loading_buffer);
        let info_lbl = text_info_label.clone();
        let badge_lbl = mode_badge.clone();
        let edit_btn = edit_toggle_btn.clone();
        let save_b = save_btn.clone();
        let save_s = save_sudo_btn.clone();
        let ro_banner = readonly_banner.clone();
        let is_ed = Rc::clone(&is_editing_rc);
        let build_menu = Rc::clone(&populate_more_options_menu);

        Rc::new(move || {
            let idx = *idx_rc.borrow();
            let Some(entry) = entries.get(idx) else {
                return;
            };

            pos_l.set_text(&format!("{}/{}", idx + 1, entries.len()));
            *dirty_c.borrow_mut() = false;
            save_b.set_sensitive(false);
            up_hdr();
            build_menu();

            let path = Path::new(&entry.path);
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
                save_b.set_visible(false);
                save_s.set_visible(false);
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
            } else if is_img {
                edit_btn.set_visible(false);
                save_b.set_visible(false);
                save_s.set_visible(false);
                pic.set_filename(Some(path));
                stack.set_visible_child_name("image");
            } else {
                match load_local_preview(path) {
                    Ok(preview) => match preview.kind {
                        PreviewKind::Image => {
                            edit_btn.set_visible(false);
                            save_b.set_visible(false);
                            save_s.set_visible(false);
                            pic.set_filename(Some(path));
                            stack.set_visible_child_name("image");
                        }
                        PreviewKind::Text => {
                            edit_btn.set_visible(true);
                            let editing = *is_ed.borrow();
                            save_b.set_visible(editing);
                            save_s.set_visible(editing);

                            let raw_text = fs::read_to_string(path).unwrap_or_else(|_| {
                                preview.text_content.clone().unwrap_or_default()
                            });

                            *orig_c.borrow_mut() = raw_text.clone();
                            *is_loading.borrow_mut() = true;
                            tv.buffer().set_text(&raw_text);
                            *is_loading.borrow_mut() = false;

                            // Check read-only status
                            let is_readonly = fs::OpenOptions::new()
                                .write(true)
                                .open(path)
                                .is_err();
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
                            save_b.set_visible(false);
                            save_s.set_visible(false);
                            while let Some(child) = bin_card.first_child() {
                                bin_card.remove(&child);
                            }
                            let icon = gtk4::Image::from_icon_name("application-x-executable-symbolic");
                            icon.set_pixel_size(64);
                            bin_card.append(&icon);
                            let info = gtk4::Label::new(Some(&tr_fmt(
                                "Arquivo Binário: {name}\nCaminho: {path}\nTamanho: {size}\nPermissões: {perms}",
                                &[
                                    ("name", &entry.name),
                                    ("path", &entry.path),
                                    ("size", &entry.formatted_size_clean()),
                                    ("perms", &entry.formatted_permissions()),
                                ],
                            )));
                            info.set_justify(gtk4::Justification::Center);
                            bin_card.append(&info);
                            stack.set_visible_child_name("binary");
                        }
                    },
                    Err(_) => {
                        edit_btn.set_visible(false);
                        save_b.set_visible(false);
                        save_s.set_visible(false);
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
        })
    };

    // ── 6. Wire Buffer Change (Dirty State Tracking) ─────────────────────────
    {
        let tv_buf = text_view.buffer();
        let orig_c = Rc::clone(&original_content_rc);
        let dirty_c = Rc::clone(&is_dirty_rc);
        let is_loading = Rc::clone(&is_loading_buffer);
        let sb = save_btn.clone();
        let up_hdr = Rc::clone(&update_title_and_header);

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
}
