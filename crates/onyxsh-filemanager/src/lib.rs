//! Dual-pane file manager models, sorting, filtering, TFTP server protocol, and transfer queue.
//! Port of `src/onyxsh/filemanager/*`.

pub mod backend;
pub mod quick_look;
pub mod tftp;
pub mod transfer;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Size,
    Modified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified_epoch: u64,
    pub permissions_octal: u16,
}

impl FileEntry {
    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }

    pub fn formatted_size(&self) -> String {
        if self.is_dir {
            return "<DIR>".to_string();
        }
        format_bytes(self.size)
    }

    pub fn formatted_permissions(&self) -> String {
        let prefix = if self.is_dir {
            'd'
        } else if self.is_symlink {
            'l'
        } else {
            '-'
        };
        let mode = self.permissions_octal & 0o777;
        let triplet = |shift: u16| -> [char; 3] {
            let bits = (mode >> shift) & 0b111;
            [
                if bits & 0b100 != 0 { 'r' } else { '-' },
                if bits & 0b010 != 0 { 'w' } else { '-' },
                if bits & 0b001 != 0 { 'x' } else { '-' },
            ]
        };
        let u = triplet(6);
        let g = triplet(3);
        let o = triplet(0);
        format!(
            "{prefix}{}{}{}{}{}{}{}{}{}",
            u[0], u[1], u[2], g[0], g[1], g[2], o[0], o[1], o[2]
        )
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    format!("{size:.1} {}", UNITS[unit_idx])
}

/// Filters and sorts directory entries (always keeping directories first when `dirs_first` is enabled).
pub fn filter_and_sort_entries(
    entries: &[FileEntry],
    show_hidden: bool,
    search_query: &str,
    sort_by: SortColumn,
    ascending: bool,
    dirs_first: bool,
) -> Vec<FileEntry> {
    let query_lower = search_query.trim().to_lowercase();
    let mut filtered: Vec<FileEntry> = entries
        .iter()
        .filter(|e| {
            if !show_hidden && e.is_hidden() {
                return false;
            }
            if !query_lower.is_empty() && !e.name.to_lowercase().contains(&query_lower) {
                return false;
            }
            true
        })
        .cloned()
        .collect();

    filtered.sort_by(|a, b| {
        if dirs_first && a.is_dir != b.is_dir {
            return if a.is_dir {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let ord = match sort_by {
            SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortColumn::Size => a.size.cmp(&b.size),
            SortColumn::Modified => a.modified_epoch.cmp(&b.modified_epoch),
        };
        if ascending {
            ord
        } else {
            ord.reverse()
        }
    });

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_entry_formatting_and_sorting() {
        let items = vec![
            FileEntry {
                name: ".bashrc".to_string(),
                path: "/home/user/.bashrc".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 512,
                modified_epoch: 100,
                permissions_octal: 0o644,
            },
            FileEntry {
                name: "zebra.txt".to_string(),
                path: "/home/user/zebra.txt".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 2048,
                modified_epoch: 200,
                permissions_octal: 0o644,
            },
            FileEntry {
                name: "apps".to_string(),
                path: "/home/user/apps".to_string(),
                is_dir: true,
                is_symlink: false,
                size: 4096,
                modified_epoch: 150,
                permissions_octal: 0o755,
            },
        ];

        let visible = filter_and_sort_entries(&items, false, "", SortColumn::Name, true, true);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].name, "apps"); // Dir first
        assert_eq!(visible[1].name, "zebra.txt");
    }
}
