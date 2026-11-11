//! Phase 12 / Checksum Dialog: Modern Libadwaita Dialog for cryptographic checksum
//! calculation and hash integrity verification.
//!
//! Ports `src/onyxsh/ui/dialogs/checksum_dialog.py` and `src/onyxsh/utils/checksum_utils.py`.
//!
//! Provides:
//! - Multialgorithm single-pass streaming hashing: SHA-256, SHA-512, MD5, SHA-1
//! - Background worker thread with cancellation support on dialog close
//! - Responsive progress bar for large files
//! - File metadata header card (icon, name, size, path)
//! - Cryptographic hashes section with monospace selectable labels and individual copy buttons
//! - Real-time integrity comparison with match/mismatch visual banners and row accenting
//! - "Copiar Relatório Completo" (formatted Markdown table)
//! - "Inserir sha256sum no Terminal" direct command feed

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use md5::Md5;
use onyxsh_core::i18n::{tr, tr_fmt};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

/// Computed cryptographic digests for a target file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileChecksums {
    pub sha256: String,
    pub sha512: String,
    pub md5: String,
    pub sha1: String,
}

impl FileChecksums {
    pub fn get(&self, algo: &str) -> Option<&str> {
        match algo.to_ascii_lowercase().as_str() {
            "sha256" | "sha-256" => Some(&self.sha256),
            "sha512" | "sha-512" => Some(&self.sha512),
            "md5" => Some(&self.md5),
            "sha1" | "sha-1" => Some(&self.sha1),
            _ => None,
        }
    }
}

/// Message emitted from background hashing worker to main GTK thread.
#[derive(Debug)]
pub enum HashWorkerMsg {
    Progress { bytes_read: u64, total_bytes: u64 },
    Done(Result<FileChecksums, String>),
}

/// Converts a byte slice to a lowercase hexadecimal string.
fn to_hex_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Computes SHA-256, SHA-512, MD5, and SHA-1 in a single chunked stream pass.
pub fn calculate_file_checksums_stream<F>(
    path: &Path,
    cancel_flag: Arc<AtomicBool>,
    mut progress_cb: F,
) -> std::io::Result<FileChecksums>
where
    F: FnMut(u64, u64),
{
    let mut file = File::open(path)?;
    let total_size = file.metadata().map(|m| m.len()).unwrap_or(0);

    let mut h_sha256 = Sha256::new();
    let mut h_sha512 = Sha512::new();
    let mut h_md5 = Md5::new();
    let mut h_sha1 = Sha1::new();

    let mut buffer = [0u8; 65536]; // 64 KB chunks
    let mut bytes_read: u64 = 0;
    let mut last_reported: u64 = 0;

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                tr("Cálculo de checksum cancelado pelo usuário."),
            ));
        }

        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }

        let chunk = &buffer[..n];
        h_sha256.update(chunk);
        h_sha512.update(chunk);
        h_md5.update(chunk);
        h_sha1.update(chunk);

        bytes_read += n as u64;

        if bytes_read.saturating_sub(last_reported) >= 1024 * 1024 || bytes_read == total_size {
            progress_cb(bytes_read, total_size);
            last_reported = bytes_read;
        }
    }

    progress_cb(bytes_read, total_size);

    Ok(FileChecksums {
        sha256: to_hex_string(&h_sha256.finalize()),
        sha512: to_hex_string(&h_sha512.finalize()),
        md5: to_hex_string(&h_md5.finalize()),
        sha1: to_hex_string(&h_sha1.finalize()),
    })
}

/// Detects likely hash algorithm based on hex string length.
pub fn detect_hash_type(input: &str) -> Option<&'static str> {
    let cleaned = input.trim().trim_matches(|c| c == '\'' || c == '"');
    if cleaned.is_empty() || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match cleaned.len() {
        32 => Some("md5"),
        40 => Some("sha1"),
        64 => Some("sha256"),
        128 => Some("sha512"),
        _ => None,
    }
}

/// Compares user-provided input string against computed hashes.
pub fn compare_hash(
    computed: &FileChecksums,
    input: &str,
) -> (bool, Option<&'static str>) {
    let cleaned = input
        .trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .to_ascii_lowercase();
    if cleaned.is_empty() || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return (false, None);
    }

    if cleaned == computed.sha256 {
        (true, Some("sha256"))
    } else if cleaned == computed.sha512 {
        (true, Some("sha512"))
    } else if cleaned == computed.md5 {
        (true, Some("md5"))
    } else if cleaned == computed.sha1 {
        (true, Some("sha1"))
    } else {
        (false, None)
    }
}

/// Formats calculated hashes into a Markdown report.
pub fn format_checksum_report(
    file_name: &str,
    file_path: &str,
    file_size_str: &str,
    hashes: &FileChecksums,
) -> String {
    let now_str = glib::DateTime::now_local()
        .ok()
        .and_then(|dt| dt.format("%Y-%m-%d %H:%M:%S").ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "N/A".to_string());

    format!(
        "### 🔐 OnyxSH Checksum Report\n\
        - **File:** `{file_name}`\n\
        - **Path:** `{file_path}`\n\
        - **Size:** {file_size_str}\n\
        - **Generated:** {now_str}\n\n\
        | Algorithm | Hash Value |\n\
        | :--- | :--- |\n\
        | **SHA256** | `{}` |\n\
        | **SHA512** | `{}` |\n\
        | **MD5** | `{}` |\n\
        | **SHA1** | `{}` |",
        hashes.sha256, hashes.sha512, hashes.md5, hashes.sha1
    )
}

/// Displays the Libadwaita Checksum Dialog for a selected file.
pub fn show_checksum_dialog<W: IsA<gtk4::Window>>(
    parent: &W,
    file_path: &Path,
    file_name: &str,
    file_size_str: &str,
    feed_terminal_cb: &Rc<dyn Fn(&str)>,
) {
    let dlg = adw::Window::builder()
        .transient_for(parent)
        .modal(true)
        .title(&tr("Checksums & Verificação de Integridade — OnyxSH"))
        .default_width(700)
        .default_height(620)
        .build();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let flag = Arc::clone(&cancel_flag);
        dlg.connect_close_request(move |_| {
            flag.store(true, Ordering::Relaxed);
            glib::Propagation::Proceed
        });
    }

    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let btn_close = gtk4::Button::with_label(&tr("Close"));
    {
        let d = dlg.clone();
        btn_close.connect_clicked(move |_| d.close());
    }
    header.pack_end(&btn_close);
    toolbar_view.add_top_bar(&header);

    let toast_overlay = adw::ToastOverlay::new();

    let clamp = adw::Clamp::builder()
        .maximum_size(680)
        .tightening_threshold(540)
        .build();

    let main_box = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    main_box.set_margin_top(16);
    main_box.set_margin_bottom(16);
    main_box.set_margin_start(16);
    main_box.set_margin_end(16);

    // 1. File Information Header Card
    let file_group = adw::PreferencesGroup::new();
    let file_row = adw::ActionRow::builder()
        .title(file_name)
        .subtitle(&format!("{file_size_str} • {}", file_path.display()))
        .build();
    file_row.add_prefix(&gtk4::Image::from_icon_name("document-properties-symbolic"));
    file_group.add(&file_row);
    main_box.append(&file_group);

    // 2. Progress Indicator (active while calculating)
    let progress_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let progress_bar = gtk4::ProgressBar::builder()
        .show_text(true)
        .text(&tr("Calculating hashes..."))
        .fraction(0.0)
        .build();
    progress_box.append(&progress_bar);
    main_box.append(&progress_box);

    // 3. Cryptographic Hashes Group
    let hashes_group = adw::PreferencesGroup::builder()
        .title(&tr("Cryptographic Hashes"))
        .description(&tr(
            "Resumos criptográficos padrão calculados sobre o conteúdo do arquivo:",
        ))
        .build();

    let algos = [
        ("sha256", "SHA-256 (SHA256)", tr("Modern standard for integrity and downloads")),
        ("sha512", "SHA-512 (SHA512)", tr("High security 512-bit cryptographic digest")),
        ("md5", "MD5 (MD5)", tr("Legacy checksum for fast verification")),
        ("sha1", "SHA-1 (SHA1)", tr("Compatibilidade com repositórios Git e legado")),
    ];

    let computed_hashes_rc = Rc::new(RefCell::new(None::<FileChecksums>));
    let hash_labels: Rc<RefCell<HashMap<String, gtk4::Label>>> = Rc::new(RefCell::new(HashMap::new()));
    let hash_rows: Rc<RefCell<HashMap<String, adw::ActionRow>>> = Rc::new(RefCell::new(HashMap::new()));

    for (algo_id, title, desc) in algos {
        let row = adw::ActionRow::builder()
            .title(title)
            .subtitle(&desc)
            .build();

        let lbl = gtk4::Label::builder()
            .label(&tr("Calculating..."))
            .selectable(true)
            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
            .xalign(0.0)
            .width_chars(28)
            .build();
        lbl.add_css_class("monospace");

        let copy_btn = gtk4::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text(&tr("Copy hash to clipboard"))
            .build();
        copy_btn.add_css_class("flat");

        {
            let computed = Rc::clone(&computed_hashes_rc);
            let algo_str = algo_id.to_string();
            let overlay = toast_overlay.clone();
            copy_btn.connect_clicked(move |_| {
                if let Some(ch) = computed.borrow().as_ref() {
                    if let Some(val) = ch.get(&algo_str) {
                        if let Some(display) = gdk::Display::default() {
                            display.clipboard().set_text(val);
                            overlay.add_toast(adw::Toast::new(&tr_fmt(
                                "Hash {algo} copiado para a área de transferência",
                                &[("algo", &algo_str.to_ascii_uppercase())],
                            )));
                        }
                    }
                }
            });
        }

        row.add_suffix(&lbl);
        row.add_suffix(&copy_btn);

        hash_labels.borrow_mut().insert(algo_id.to_string(), lbl);
        hash_rows.borrow_mut().insert(algo_id.to_string(), row.clone());
        hashes_group.add(&row);
    }
    main_box.append(&hashes_group);

    // 4. Integrity Matcher & Comparison Group
    let matcher_group = adw::PreferencesGroup::builder()
        .title(&tr("Comparador de Integridade"))
        .description(&tr(
            "Paste the expected hash provided by the author to verify authenticity:",
        ))
        .build();

    let match_entry = adw::EntryRow::builder()
        .title(&tr("Expected Hash"))
        .build();
    matcher_group.add(&match_entry);

    let status_banner_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    status_banner_box.set_margin_top(8);
    status_banner_box.set_margin_bottom(4);
    status_banner_box.add_css_class("card");
    status_banner_box.set_margin_start(2);
    status_banner_box.set_margin_end(2);

    let status_icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
    status_icon.set_margin_start(12);
    status_icon.set_margin_top(10);
    status_icon.set_margin_bottom(10);

    let status_label = gtk4::Label::builder()
        .label(&tr(
            "Cole um hash MD5, SHA-1, SHA-256 ou SHA-512 acima para verificar autenticidade.",
        ))
        .wrap(true)
        .xalign(0.0)
        .margin_end(12)
        .margin_top(10)
        .margin_bottom(10)
        .build();

    status_banner_box.append(&status_icon);
    status_banner_box.append(&status_label);
    matcher_group.add(&status_banner_box);
    main_box.append(&matcher_group);

    // Live update function for hash matcher
    let evaluate_matching = {
        let entry_ref = match_entry.clone();
        let computed_rc = Rc::clone(&computed_hashes_rc);
        let s_icon = status_icon.clone();
        let s_lbl = status_label.clone();
        let rows_map = Rc::clone(&hash_rows);

        Rc::new(move || {
            let text = entry_ref.text().trim().to_string();

            // Clear highlights from all rows
            for row in rows_map.borrow().values() {
                row.remove_css_class("accent");
            }

            if text.is_empty() {
                s_icon.set_icon_name(Some("dialog-information-symbolic"));
                s_lbl.set_text("Cole um hash MD5, SHA-1, SHA-256 ou SHA-512 acima para verificar autenticidade.");
                return;
            }

            let computed_borrow = computed_rc.borrow();
            let computed = match computed_borrow.as_ref() {
                Some(c) => c,
                None => {
                    s_icon.set_icon_name(Some("dialog-information-symbolic"));
                    s_lbl.set_text(&tr("Calculando hashes em segundo plano..."));
                    return;
                }
            };

            let detected_algo = detect_hash_type(&text);
            let (is_match, matched_algo) = compare_hash(computed, &text);

            if is_match && matched_algo.is_some() {
                let algo = matched_algo.unwrap();
                s_icon.set_icon_name(Some("emblem-ok-symbolic"));
                s_lbl.set_text(&tr_fmt(
                    "✅ HASH CORRESPONDE ({algo})! O arquivo é autêntico e íntegro.",
                    &[("algo", &algo.to_ascii_uppercase())],
                ));
                if let Some(row) = rows_map.borrow().get(algo) {
                    row.add_css_class("accent");
                }
            } else {
                let hint = detected_algo
                    .map(|d| format!(" ({})", d.to_ascii_uppercase()))
                    .unwrap_or_default();
                s_icon.set_icon_name(Some("dialog-warning-symbolic"));
                s_lbl.set_text(&tr_fmt(
                    "❌ HASH NÃO CORRESPONDE! O valor colado{hint} difere dos hashes calculados do arquivo. O arquivo pode estar corrompido ou modificado.",
                    &[("hint", &hint)],
                ));
            }
        })
    };

    {
        let ev = Rc::clone(&evaluate_matching);
        match_entry.connect_changed(move |_| {
            ev();
        });
    }

    // 5. Actions Box (Copiar Relatório Completo, Inserir no Terminal)
    let actions_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
    actions_box.set_halign(gtk4::Align::End);
    actions_box.set_margin_top(10);

    let copy_report_btn = gtk4::Button::builder()
        .label(&tr("Copy Full Report"))
        .icon_name("edit-copy-symbolic")
        .tooltip_text(&tr("Copiar relatório detalhado em formato Markdown"))
        .build();

    {
        let computed_rc = Rc::clone(&computed_hashes_rc);
        let overlay = toast_overlay.clone();
        let fname = file_name.to_string();
        let fpath = file_path.display().to_string();
        let fsize = file_size_str.to_string();

        copy_report_btn.connect_clicked(move |_| {
            if let Some(ch) = computed_rc.borrow().as_ref() {
                let report = format_checksum_report(&fname, &fpath, &fsize, ch);
                if let Some(display) = gdk::Display::default() {
                    display.clipboard().set_text(&report);
                    overlay.add_toast(adw::Toast::new(&tr(
                        "Relatório completo de hashes copiado para a área de transferência",
                    )));
                }
            }
        });
    }

    let terminal_btn = gtk4::Button::builder()
        .label(&tr("Insert sha256sum into Terminal"))
        .icon_name("utilities-terminal-symbolic")
        .tooltip_text(&tr("Enviar comando sha256sum para a aba ativa do terminal"))
        .build();

    {
        let fpath = file_path.display().to_string();
        let feed_cb = Rc::clone(feed_terminal_cb);
        let dlg_ref = dlg.clone();
        terminal_btn.connect_clicked(move |_| {
            let escaped = fpath.replace('\'', "'\\''");
            let cmd = format!("sha256sum '{escaped}'\n");
            feed_cb(&cmd);
            dlg_ref.close();
        });
    }

    actions_box.append(&copy_report_btn);
    actions_box.append(&terminal_btn);
    main_box.append(&actions_box);

    clamp.set_child(Some(&main_box));

    let scroller = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .child(&clamp)
        .build();

    toast_overlay.set_child(Some(&scroller));
    toolbar_view.set_content(Some(&toast_overlay));
    dlg.set_content(Some(&toolbar_view));

    // 6. Spawn Background Hashing Thread
    let (tx, rx) = mpsc::channel::<HashWorkerMsg>();
    let path_buf = file_path.to_path_buf();
    let thread_cancel = Arc::clone(&cancel_flag);

    std::thread::spawn(move || {
        let res = calculate_file_checksums_stream(&path_buf, thread_cancel, |read, total| {
            let _ = tx.send(HashWorkerMsg::Progress {
                bytes_read: read,
                total_bytes: total,
            });
        });

        let _ = tx.send(HashWorkerMsg::Done(res.map_err(|e| e.to_string())));
    });

    {
        let pb_box = progress_box;
        let pb = progress_bar;
        let labels_map = Rc::clone(&hash_labels);
        let computed_store = Rc::clone(&computed_hashes_rc);
        let eval_fn = Rc::clone(&evaluate_matching);
        let s_icon = status_icon;
        let s_lbl = status_label;

        glib::timeout_add_local(Duration::from_millis(50), move || {
            let mut should_continue = true;
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    HashWorkerMsg::Progress { bytes_read, total_bytes } => {
                        let fraction = if total_bytes > 0 {
                            (bytes_read as f64 / total_bytes as f64).min(1.0)
                        } else {
                            1.0
                        };
                        pb.set_fraction(fraction);
                        let read_mb = bytes_read as f64 / (1024.0 * 1024.0);
                        let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
                        let pct = (fraction * 100.0) as u32;
                        pb.set_text(Some(&format!("{read_mb:.1} MB / {total_mb:.1} MB ({pct}%)")));
                    }
                    HashWorkerMsg::Done(res) => {
                        pb_box.set_visible(false);
                        match res {
                            Ok(hashes) => {
                                let map = labels_map.borrow();
                                if let Some(lbl) = map.get("sha256") {
                                    lbl.set_text(&hashes.sha256);
                                }
                                if let Some(lbl) = map.get("sha512") {
                                    lbl.set_text(&hashes.sha512);
                                }
                                if let Some(lbl) = map.get("md5") {
                                    lbl.set_text(&hashes.md5);
                                }
                                if let Some(lbl) = map.get("sha1") {
                                    lbl.set_text(&hashes.sha1);
                                }
                                *computed_store.borrow_mut() = Some(hashes);
                                eval_fn();
                            }
                            Err(e) => {
                                for lbl in labels_map.borrow().values() {
                                    lbl.set_text(&tr("Erro ao calcular hash"));
                                }
                                s_icon.set_icon_name(Some("dialog-error-symbolic"));
                                s_lbl.set_text(&tr_fmt(
                                    "Falha ao calcular checksums: {err}",
                                    &[("err", &e)],
                                ));
                            }
                        }
                        should_continue = false;
                        break;
                    }
                }
            }
            if should_continue {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });
    }

    dlg.present();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_calculate_file_checksums_stream() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("onyxsh_test_checksum.txt");
        let content = b"Hello, OnyxSH Checksum Test!";
        {
            let mut f = File::create(&test_file).unwrap();
            f.write_all(content).unwrap();
        }

        let cancel = Arc::new(AtomicBool::new(false));
        let mut progress_called = false;
        let hashes = calculate_file_checksums_stream(&test_file, cancel, |_read, _total| {
            progress_called = true;
        })
        .unwrap();

        assert!(progress_called);
        assert_eq!(hashes.sha256.len(), 64);
        assert_eq!(hashes.sha512.len(), 128);
        assert_eq!(hashes.md5.len(), 32);
        assert_eq!(hashes.sha1.len(), 40);

        // Known SHA-256 for "Hello, OnyxSH Checksum Test!"
        let mut expected_h = Sha256::new();
        expected_h.update(content);
        let expected_hex = to_hex_string(&expected_h.finalize());
        assert_eq!(hashes.sha256, expected_hex);

        let _ = std::fs::remove_file(test_file);
    }

    #[test]
    fn test_detect_hash_type() {
        assert_eq!(detect_hash_type("d41d8cd98f00b204e9800998ecf8427e"), Some("md5"));
        assert_eq!(
            detect_hash_type("da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            Some("sha1")
        );
        assert_eq!(
            detect_hash_type(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            ),
            Some("sha256")
        );
        assert_eq!(
            detect_hash_type(&"a".repeat(128)),
            Some("sha512")
        );
        assert_eq!(detect_hash_type("not-a-hash"), None);
        assert_eq!(detect_hash_type(""), None);
        assert_eq!(detect_hash_type("12345"), None);
    }

    #[test]
    fn test_compare_hash() {
        let hashes = FileChecksums {
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            sha512: "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e".to_string(),
            md5: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
            sha1: "da39a3ee5e6b4b0d3255bfef95601890afd80709".to_string(),
        };

        // Match SHA-256 case insensitive
        let (matched, algo) = compare_hash(
            &hashes,
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        );
        assert!(matched);
        assert_eq!(algo, Some("sha256"));

        // Match MD5 with quotes
        let (matched, algo) = compare_hash(&hashes, "\"d41d8cd98f00b204e9800998ecf8427e\"");
        assert!(matched);
        assert_eq!(algo, Some("md5"));

        // Mismatch
        let (matched, algo) = compare_hash(&hashes, "00000000000000000000000000000000");
        assert!(!matched);
        assert_eq!(algo, None);
    }

    #[test]
    fn test_format_checksum_report() {
        let hashes = FileChecksums {
            sha256: "abc256".to_string(),
            sha512: "def512".to_string(),
            md5: "123md5".to_string(),
            sha1: "456sha1".to_string(),
        };

        let report = format_checksum_report("test.iso", "/tmp/test.iso", "4.2 GB", &hashes);
        assert!(report.contains("### 🔐 OnyxSH Checksum Report"));
        assert!(report.contains("**File:** `test.iso`"));
        assert!(report.contains("**Path:** `/tmp/test.iso`"));
        assert!(report.contains("**Size:** 4.2 GB"));
        assert!(report.contains("| **SHA256** | `abc256` |"));
        assert!(report.contains("| **SHA512** | `def512` |"));
        assert!(report.contains("| **MD5** | `123md5` |"));
        assert!(report.contains("| **SHA1** | `456sha1` |"));
    }
}
