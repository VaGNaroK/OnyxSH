//! Quick Look File Preview Classifier and Safe Reader (`src/onyxsh/filemanager/quick_look.py`).

use crate::FileEntry;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::Path;

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
        return Ok(QuickLookPreview {
            kind: PreviewKind::Directory,
            title,
            subtitle: format!("{count} items"),
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
}
