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
    Permissions,
    Owner,
    Type,
    GroupByType,
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
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub symlink_target: Option<String>,
}

impl FileEntry {
    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.') && self.name != ".."
    }

    pub fn is_executable(&self) -> bool {
        if self.is_dir {
            return false;
        }
        (self.permissions_octal & 0o111) != 0
    }

    pub fn permissions_octal_str(&self) -> String {
        format!("{:04o}", self.permissions_octal & 0o7777)
    }

    pub fn formatted_size(&self) -> String {
        if self.is_dir {
            return "<DIR>".to_string();
        }
        format_bytes(self.size)
    }

    pub fn formatted_size_clean(&self) -> String {
        if self.is_dir {
            let s = if self.size == 0 { 4096 } else { self.size };
            format_bytes(s)
        } else {
            format_bytes(self.size)
        }
    }

    pub fn file_type_badge(&self) -> Option<(&'static str, &'static str)> {
        if self.is_dir {
            return None;
        }
        let lower = self.name.to_lowercase();
        let ext = std::path::Path::new(&self.name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "py" => Some(("PY", "badge-py")),
            "sh" | "bash" | "zsh" => Some(("SH", "badge-sh")),
            "rs" => Some(("RS", "badge-rs")),
            "log" => Some(("LOG", "badge-log")),
            "json" => Some(("JSON", "badge-json")),
            "yaml" | "yml" => Some(("YAML", "badge-yaml")),
            "toml" => Some(("TOML", "badge-yaml")),
            "md" => Some(("MD", "badge-py")),
            _ if lower.starts_with("dockerfile") => Some(("DOCKER", "badge-docker")),
            _ if self.is_executable() => Some(("EXEC", "badge-exec")),
            _ => None,
        }
    }

    /// Returns lowercase file extension (without leading dot).
    pub fn extension(&self) -> String {
        std::path::Path::new(&self.name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    }

    /// Numerical group priority for intelligent category clustering.
    /// Priority 1 clusters archives (.7z, .zip, etc.) at the very top of files.
    pub fn group_priority(&self) -> u8 {
        if self.name == ".." || self.is_dir {
            return 0;
        }
        let ext = self.extension();
        match ext.as_str() {
            // Priority 1: Archives & Compressed Files (.7z at the top, .zip, .tar, .gz, etc.)
            "7z" | "zip" | "tar" | "gz" | "tgz" | "xz" | "bz2" | "rar" | "zst" | "iso" => 1,
            // Priority 2: Scripts and Source Code
            "py" | "sh" | "bash" | "zsh" | "rs" | "js" | "ts" | "c" | "cpp" | "h" | "hpp"
            | "go" | "java" | "html" | "css" | "scss" | "lua" | "php" | "rb" => 2,
            // Priority 3: Packages & Binaries
            "deb" | "rpm" | "apk" | "appimage" | "flatpakref" => 3,
            _ if self.is_executable() => 3,
            // Priority 4: Documents & Plain Text
            "pdf" | "md" | "txt" | "doc" | "docx" | "odt" | "csv" | "xlsx" | "pptx" => 4,
            // Priority 5: Images & Visual Media
            "png" | "jpg" | "jpeg" | "svg" | "webp" | "gif" | "ico" | "bmp" => 5,
            // Priority 6: Audio & Video
            "mp3" | "wav" | "flac" | "ogg" | "mp4" | "mkv" | "avi" | "webm" => 6,
            // Priority 7: Configuration, Environment and Data Files
            "json" | "yaml" | "yml" | "toml" | "xml" | "ini" | "env" | "conf" | "sql" => 7,
            // Priority 8: General / Other Files
            _ => 8,
        }
    }

    pub fn type_description(&self) -> &'static str {
        if self.name == ".." {
            return "Diretório Pai";
        }
        if self.is_dir {
            return "Pasta / Diretório";
        }
        if self.is_symlink {
            return "Link Simbólico";
        }
        let ext = std::path::Path::new(&self.name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "py" => "Script Python",
            "sh" | "bash" | "zsh" => "Script Shell",
            "rs" => "Código Rust",
            "js" => "JavaScript",
            "ts" => "TypeScript",
            "c" => "Código Fonte C",
            "cpp" => "Código Fonte C++",
            "h" | "hpp" => "Cabeçalho C/C++",
            "go" => "Código Fonte Go",
            "json" => "Documento JSON",
            "yaml" | "yml" => "Documento YAML",
            "toml" => "Documento TOML",
            "md" => "Documento Markdown",
            "txt" => "Documento de Texto",
            "log" => "Arquivo de Log",
            "pdf" => "Documento PDF",
            "png" | "jpg" | "jpeg" | "webp" | "svg" | "gif" => "Imagem",
            "mp3" | "wav" | "ogg" | "flac" => "Arquivo de Áudio",
            "mp4" | "mkv" | "webm" => "Arquivo de Vídeo",
            "zip" | "tar" | "gz" | "xz" | "7z" => "Arquivo Compactado",
            "deb" | "rpm" => "Pacote de Instalação",
            _ if self.is_executable() => "Programa Executável",
            _ => "Arquivo de Dados",
        }
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
            SortColumn::Permissions => a.permissions_octal.cmp(&b.permissions_octal),
            SortColumn::Owner => a.owner.to_lowercase().cmp(&b.owner.to_lowercase()),
            SortColumn::Type => {
                let ext_cmp = a.extension().cmp(&b.extension());
                if ext_cmp == Ordering::Equal {
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                } else {
                    ext_cmp
                }
            }
            SortColumn::GroupByType => {
                let grp_cmp = a.group_priority().cmp(&b.group_priority());
                if grp_cmp == Ordering::Equal {
                    let ext_cmp = a.extension().cmp(&b.extension());
                    if ext_cmp == Ordering::Equal {
                        a.name.to_lowercase().cmp(&b.name.to_lowercase())
                    } else {
                        ext_cmp
                    }
                } else {
                    grp_cmp
                }
            }
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
                owner: "user:user".to_string(),
                symlink_target: None,
            },
            FileEntry {
                name: "zebra.txt".to_string(),
                path: "/home/user/zebra.txt".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 2048,
                modified_epoch: 200,
                permissions_octal: 0o644,
                owner: "user:user".to_string(),
                symlink_target: None,
            },
            FileEntry {
                name: "apps".to_string(),
                path: "/home/user/apps".to_string(),
                is_dir: true,
                is_symlink: false,
                size: 4096,
                modified_epoch: 150,
                permissions_octal: 0o755,
                owner: "user:user".to_string(),
                symlink_target: None,
            },
        ];

        let visible = filter_and_sort_entries(&items, false, "", SortColumn::Name, true, true);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].name, "apps"); // Dir first
        assert_eq!(visible[1].name, "zebra.txt");

        assert_eq!(items[2].formatted_size_clean(), "4.0 KB");
        assert_eq!(items[1].formatted_size_clean(), "2.0 KB");
        assert_eq!(items[2].permissions_octal_str(), "0755");
        assert!(!items[1].is_executable());
        assert_eq!(items[2].type_description(), "Pasta / Diretório");
        assert_eq!(items[1].type_description(), "Documento de Texto");

        let script = FileEntry {
            name: "build.sh".to_string(),
            path: "/home/user/build.sh".to_string(),
            is_dir: false,
            is_symlink: false,
            size: 1024,
            modified_epoch: 300,
            permissions_octal: 0o755,
            owner: "user:user".to_string(),
            symlink_target: None,
        };
        assert!(script.is_executable());
        assert_eq!(script.file_type_badge(), Some(("SH", "badge-sh")));

        let py = FileEntry {
            name: "main.py".to_string(),
            path: "/home/user/main.py".to_string(),
            is_dir: false,
            is_symlink: false,
            size: 1024,
            modified_epoch: 300,
            permissions_octal: 0o644,
            owner: "user:user".to_string(),
            symlink_target: None,
        };
        assert!(!py.is_executable());
        assert_eq!(py.file_type_badge(), Some(("PY", "badge-py")));

        let parent = FileEntry {
            name: "..".to_string(),
            path: "/home".to_string(),
            is_dir: true,
            is_symlink: false,
            size: 4096,
            modified_epoch: 0,
            permissions_octal: 0o755,
            owner: "root:root".to_string(),
            symlink_target: None,
        };
        assert!(!parent.is_hidden());
        assert_eq!(parent.formatted_size_clean(), "4.0 KB");
    }

    #[test]
    fn test_file_grouping_and_type_sorting() {
        let entries = vec![
            FileEntry {
                name: "script.py".to_string(),
                path: "/tmp/script.py".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 100,
                modified_epoch: 1,
                permissions_octal: 0o644,
                owner: "u:u".to_string(),
                symlink_target: None,
            },
            FileEntry {
                name: "backup.7z".to_string(),
                path: "/tmp/backup.7z".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 500,
                modified_epoch: 2,
                permissions_octal: 0o644,
                owner: "u:u".to_string(),
                symlink_target: None,
            },
            FileEntry {
                name: "readme.txt".to_string(),
                path: "/tmp/readme.txt".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 50,
                modified_epoch: 3,
                permissions_octal: 0o644,
                owner: "u:u".to_string(),
                symlink_target: None,
            },
            FileEntry {
                name: "archive.zip".to_string(),
                path: "/tmp/archive.zip".to_string(),
                is_dir: false,
                is_symlink: false,
                size: 300,
                modified_epoch: 4,
                permissions_octal: 0o644,
                owner: "u:u".to_string(),
                symlink_target: None,
            },
            FileEntry {
                name: "docs".to_string(),
                path: "/tmp/docs".to_string(),
                is_dir: true,
                is_symlink: false,
                size: 4096,
                modified_epoch: 5,
                permissions_octal: 0o755,
                owner: "u:u".to_string(),
                symlink_target: None,
            },
        ];

        // GroupByType: Dirs first, then Priority 1 (Archives: .7z at the very top, .zip), then Scripts (.py), then Docs (.txt)
        let grouped = filter_and_sort_entries(&entries, false, "", SortColumn::GroupByType, true, true);
        assert_eq!(grouped[0].name, "docs"); // Directory first
        assert_eq!(grouped[1].name, "backup.7z"); // Archives group: .7z at the top
        assert_eq!(grouped[2].name, "archive.zip"); // Archives group: .zip
        assert_eq!(grouped[3].name, "script.py"); // Code/Scripts group
        assert_eq!(grouped[4].name, "readme.txt"); // Documents group

        // Test priority 1 for .7z
        let e_7z = &entries[1];
        assert_eq!(e_7z.extension(), "7z");
        assert_eq!(e_7z.group_priority(), 1);

        // Sort by Type (alphabetical by extension):
        // extensions: .7z (backup.7z), .py (script.py), .txt (readme.txt), .zip (archive.zip)
        let by_type = filter_and_sort_entries(&entries, false, "", SortColumn::Type, true, true);
        assert_eq!(by_type[0].name, "docs"); // Directory first
        assert_eq!(by_type[1].name, "backup.7z"); // .7z
        assert_eq!(by_type[2].name, "script.py"); // .py
        assert_eq!(by_type[3].name, "readme.txt"); // .txt
        assert_eq!(by_type[4].name, "archive.zip"); // .zip
    }
}
