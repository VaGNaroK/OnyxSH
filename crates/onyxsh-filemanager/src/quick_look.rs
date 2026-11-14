//! Quick Look File Preview Classifier and Safe Reader (`src/onyxsh/filemanager/quick_look.py`).

use crate::FileEntry;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

/// Maximum bytes read for text preview in Quick Look (256 KiB).
pub const MAX_PREVIEW_BYTES: usize = 256 * 1024;
/// Number of initial bytes inspected to detect binary files via null-byte heuristic.
const BINARY_SNIFF_BYTES: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreviewKind {
    Directory,
    Image,
    Archive,
    Text,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickLookPreview {
    pub kind: PreviewKind,
    pub title: String,
    pub subtitle: String,
    pub text_content: Option<String>,
    pub truncated: bool,
}

/// Classifies a path/extension into a [`PreviewKind`], inspecting initial bytes if needed.
pub fn classify_preview_kind(path: &Path, is_dir: bool, sample_bytes: &[u8]) -> PreviewKind {
    if is_dir {
        return PreviewKind::Directory;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => {
            return PreviewKind::Image;
        }
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "deb" | "rpm" => {
            return PreviewKind::Archive;
        }
        _ => {}
    }

    let sniff_len = sample_bytes.len().min(BINARY_SNIFF_BYTES);
    if sample_bytes[..sniff_len].contains(&0) {
        PreviewKind::Binary
    } else {
        PreviewKind::Text
    }
}

/// Generates a [`QuickLookPreview`] for a local file or directory safely bounded by [`MAX_PREVIEW_BYTES`].
pub fn load_local_preview(path: &Path) -> std::io::Result<QuickLookPreview> {
    let meta = std::fs::metadata(path)?;
    let title = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    if meta.is_dir() {
        let count = std::fs::read_dir(path).map(|it| it.count()).unwrap_or(0);
        let item_word = onyxsh_core::i18n::tr_n("item", "itens", count as u64);
        return Ok(QuickLookPreview {
            kind: PreviewKind::Directory,
            title,
            subtitle: format!("{count} {item_word}"),
            text_content: None,
            truncated: false,
        });
    }

    let file_size = meta.len();
    let f = File::open(path)?;
    let mut buf = Vec::with_capacity((file_size as usize).min(MAX_PREVIEW_BYTES + 1));
    f.take((MAX_PREVIEW_BYTES + 1) as u64).read_to_end(&mut buf)?;

    let truncated = buf.len() > MAX_PREVIEW_BYTES;
    if truncated {
        buf.truncate(MAX_PREVIEW_BYTES);
    }

    let kind = classify_preview_kind(path, false, &buf);
    let text_content = if kind == PreviewKind::Text {
        Some(String::from_utf8_lossy(&buf).into_owned())
    } else {
        None
    };

    Ok(QuickLookPreview {
        kind,
        title,
        subtitle: crate::format_bytes(file_size),
        text_content,
        truncated,
    })
}

/// Computes the next or previous index in the Quick Look item list with wrap-around.
pub fn step_quick_look_index(current: usize, total: usize, forward: bool) -> Option<usize> {
    if total == 0 {
        return None;
    }
    if forward {
        Some((current + 1) % total)
    } else if current == 0 {
        Some(total - 1)
    } else {
        Some(current - 1)
    }
}

/// Resolves the active [`FileEntry`] when navigating Quick Look items.
pub fn select_quick_look_entry(
    entries: &[FileEntry],
    current: usize,
    forward: bool,
) -> Option<(usize, &FileEntry)> {
    let next_idx = step_quick_look_index(current, entries.len(), forward)?;
    entries.get(next_idx).map(|e| (next_idx, e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogSeverity {
    Error,
    Warning,
    Info,
    Normal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub line_index: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub fg_hex: String,
    pub bold: bool,
    pub italic: bool,
}

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn find_syntax_for_path(path: &Path, text: &str) -> Option<&'static SyntaxReference> {
    let ss = syntax_set();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if let Some(syn) = ss.find_syntax_by_extension(ext) {
            return Some(syn);
        }
    }
    if let Some(first_line) = text.lines().next() {
        if let Some(syn) = ss.find_syntax_by_first_line(first_line) {
            return Some(syn);
        }
    }
    None
}

fn active_theme(dark: bool) -> &'static Theme {
    let ts = theme_set();
    let preferred = if dark {
        "base16-ocean.dark"
    } else {
        "InspiredGitHub"
    };
    ts.themes
        .get(preferred)
        .or_else(|| ts.themes.values().next())
        .expect("syntect default themes non-empty")
}

/// Highlights `text` using `syntect` for `path`'s language and returns byte-offset color spans per line.
pub fn highlight_code_spans(path: &Path, text: &str, dark_theme: bool) -> Vec<StyledSpan> {
    let Some(syntax) = find_syntax_for_path(path, text) else {
        return Vec::new();
    };
    let mut highlighter = HighlightLines::new(syntax, active_theme(dark_theme));
    let mut spans = Vec::new();
    let ss = syntax_set();

    for (line_index, line) in LinesWithEndings::from(text).enumerate() {
        let Ok(ranges) = highlighter.highlight_line(line, ss) else {
            continue;
        };
        let mut cursor = 0usize;
        for (style, token) in ranges {
            let len = token.len();
            if len > 0 {
                let fg_hex = format!(
                    "#{:02x}{:02x}{:02x}",
                    style.foreground.r, style.foreground.g, style.foreground.b
                );
                let bold = style.font_style.contains(syntect::highlighting::FontStyle::BOLD);
                let italic = style.font_style.contains(syntect::highlighting::FontStyle::ITALIC);
                spans.push(StyledSpan {
                    line_index,
                    start_byte: cursor,
                    end_byte: cursor + len,
                    fg_hex,
                    bold,
                    italic,
                });
                cursor += len;
            }
        }
    }

    spans
}

/// Classifies a log line's severity (`ERROR`/`FATAL`, `WARN`, `INFO`, or `Normal`).
pub fn classify_log_line_severity(line: &str) -> LogSeverity {
    let upper = line.to_ascii_uppercase();
    if upper.contains("FATAL")
        || upper.contains("PANIC")
        || upper.contains("CRITICAL")
        || upper.contains("ERROR")
        || upper.contains("[ERR]")
    {
        LogSeverity::Error
    } else if upper.contains("WARNING") || upper.contains("WARN") {
        LogSeverity::Warning
    } else if upper.contains("[INFO]") || upper.contains(" INFO ") {
        LogSeverity::Info
    } else {
        LogSeverity::Normal
    }
}

/// Builds a [`QuickLookPreview`] from raw sampled bytes (unified for Local and SFTP backends).
pub fn load_preview_from_bytes(name: &str, total_size: u64, sample: &[u8]) -> QuickLookPreview {
    let bounded_len = sample.len().min(MAX_PREVIEW_BYTES);
    let slice = &sample[..bounded_len];
    let truncated = (total_size as usize) > bounded_len || sample.len() > MAX_PREVIEW_BYTES;
    let kind = classify_preview_kind(Path::new(name), false, slice);
    let text_content = if kind == PreviewKind::Text {
        Some(String::from_utf8_lossy(slice).into_owned())
    } else {
        None
    };
    QuickLookPreview {
        kind,
        title: name.to_string(),
        subtitle: crate::format_bytes(total_size),
        text_content,
        truncated,
    }
}

/// Formats `bytes` as a canonical hex + ASCII dump (`00000000: 7f 45 4c ... | .ELF... |`).
pub fn format_hex_dump_page(bytes: &[u8], base_offset: usize, bytes_per_line: usize) -> String {
    let width = bytes_per_line.max(8);
    let mut out = String::new();
    for (row_idx, chunk) in bytes.chunks(width).enumerate() {
        let offset = base_offset + row_idx * width;
        out.push_str(&format!("{offset:08x}: "));
        for idx in 0..width {
            if idx < chunk.len() {
                out.push_str(&format!("{:02x} ", chunk[idx]));
            } else {
                out.push_str("   ");
            }
            if idx + 1 == width / 2 {
                out.push(' ');
            }
        }
        out.push_str(" |");
        for &b in chunk {
            if b.is_ascii_graphic() || b == b' ' {
                out.push(b as char);
            } else {
                out.push('.');
            }
        }
        out.push_str("|\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_look_classification_and_navigation() {
        assert_eq!(
            classify_preview_kind(Path::new("diagram.png"), false, b"\x89PNG\r\n\x1a\n\x00"),
            PreviewKind::Image
        );
        assert_eq!(
            classify_preview_kind(Path::new("backup.tar.gz"), false, &[0x1f, 0x8b, 0x00]),
            PreviewKind::Archive
        );
        assert_eq!(
            classify_preview_kind(Path::new("script.sh"), false, b"#!/bin/bash\necho ok\n"),
            PreviewKind::Text
        );
        assert_eq!(
            classify_preview_kind(Path::new("elf.bin"), false, b"\x7fELF\x02\x01\x01\x00"),
            PreviewKind::Binary
        );

        assert_eq!(step_quick_look_index(0, 3, false), Some(2));
        assert_eq!(step_quick_look_index(2, 3, true), Some(0));
    }

    #[test]
    fn test_syntax_highlighting_and_log_severity_classification() {
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let spans = highlight_code_spans(Path::new("main.rs"), code, true);
        assert!(!spans.is_empty());
        assert!(spans.iter().any(|s| s.fg_hex.starts_with('#') && s.fg_hex.len() == 7));
        assert!(highlight_code_spans(Path::new("unknown.xyzbin"), "raw", true).is_empty());

        assert_eq!(
            classify_log_line_severity("2026-11-12 ERROR nginx[12]: bind failed"),
            LogSeverity::Error
        );
        assert_eq!(
            classify_log_line_severity("WARN: low disk space"),
            LogSeverity::Warning
        );
        assert_eq!(
            classify_log_line_severity("[INFO] backup completed"),
            LogSeverity::Info
        );
        assert_eq!(
            classify_log_line_severity("plain output line"),
            LogSeverity::Normal
        );
    }

    #[test]
    fn test_hex_dump_page_and_byte_preview() {
        let bin_preview = load_preview_from_bytes("fw.bin", 1024, b"\x7fELF\x00\x01AB");
        assert_eq!(bin_preview.kind, PreviewKind::Binary);
        assert!(bin_preview.text_content.is_none());

        let dump = format_hex_dump_page(b"\x7fELF\x00\x01AB", 0x20, 16);
        assert!(dump.starts_with("00000020: 7f 45 4c 46 00 01 41 42"));
        assert!(dump.contains("|.ELF..AB|"));
    }
}
