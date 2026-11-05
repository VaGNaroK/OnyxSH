//! FileSystem Backends: Native `LocalFsBackend`, `SftpCliParser`, and Live `UdpTftpServer`
//! (`src/onyxsh/filemanager/local_backend.py`, `sftp_backend.py`, `tftp_server.py`).

use crate::tftp::{
    encode_data_packet, encode_error_packet, parse_request_packet, resolve_child_safe, ERR_ACCESS,
    ERR_ILLEGAL_OP, ERR_NOT_FOUND, OP_RRQ, OP_WRQ, TFTP_BLOCK_SIZE,
};
use crate::FileEntry;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

/// Common interface implemented by Local and SFTP file manager backends.
pub trait FileSystemBackend {
    fn list_dir(&self, path: &Path) -> std::io::Result<Vec<FileEntry>>;
    fn mkdir(&self, path: &Path) -> std::io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
    fn remove(&self, path: &Path) -> std::io::Result<()>;
}

/// Native local filesystem backend (`src/onyxsh/filemanager/local_backend.py`).
#[derive(Debug, Default, Clone)]
pub struct LocalFsBackend;

impl FileSystemBackend for LocalFsBackend {
    fn list_dir(&self, path: &Path) -> std::io::Result<Vec<FileEntry>> {
        let mut entries = Vec::new();
        for dir_entry in fs::read_dir(path)? {
            let dir_entry = dir_entry?;
            let symlink_meta = dir_entry.metadata()?;
            let is_symlink = dir_entry.file_type()?.is_symlink();
            let is_dir = symlink_meta.is_dir();
            let size = if is_dir { 0 } else { symlink_meta.len() };
            let modified_epoch = symlink_meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let permissions_octal = (symlink_meta.permissions().mode() & 0o777) as u16;

            entries.push(FileEntry {
                name: dir_entry.file_name().to_string_lossy().into_owned(),
                path: dir_entry.path().to_string_lossy().into_owned(),
                is_dir,
                is_symlink,
                size,
                modified_epoch,
                permissions_octal,
            });
        }
        Ok(entries)
    }

    fn mkdir(&self, path: &Path) -> std::io::Result<()> {
        fs::create_dir_all(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()> {
        fs::rename(from, to)
    }

    fn remove(&self, path: &Path) -> std::io::Result<()> {
        let meta = fs::symlink_metadata(path)?;
        if meta.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        }
    }
}

/// Parses a standard POSIX `ls -l` / SFTP `ls -l` line into a [`FileEntry`].
pub fn parse_sftp_ls_line(parent_dir: &str, line: &str) -> Option<FileEntry> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("total ") {
        return None;
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() < 9 {
        return None;
    }
    let perm_str = parts[0];
    let first_char = perm_str.chars().next()?;
    let is_dir = first_char == 'd';
    let is_symlink = first_char == 'l';
    let size = parts[4].parse::<u64>().unwrap_or(0);
    let name = parts[8..].join(" ");
    if name == "." || name == ".." {
        return None;
    }

    let mut octal = 0u16;
    let chars: Vec<char> = perm_str.chars().skip(1).take(9).collect();
    if chars.len() == 9 {
        for (idx, c) in chars.iter().enumerate() {
            if *c != '-' {
                octal |= 1 << (8 - idx);
            }
        }
    }

    let full_path = if parent_dir.ends_with('/') {
        format!("{parent_dir}{name}")
    } else {
        format!("{parent_dir}/{name}")
    };

    Some(FileEntry {
        name,
        path: full_path,
        is_dir,
        is_symlink,
        size,
        modified_epoch: 0,
        permissions_octal: octal,
    })
}

/// Read-only UDP TFTP server (`src/onyxsh/filemanager/tftp_server.py`) with strict directory confinement.
pub struct UdpTftpServer {
    socket: UdpSocket,
    root_dir: PathBuf,
}

impl UdpTftpServer {
    /// Binds a read-only UDP TFTP server to `bind_addr` (e.g. `127.0.0.1:0` or `0.0.0.0:6969`).
    pub fn bind(bind_addr: &str, root_dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_read_timeout(Some(Duration::from_millis(250)))?;
        Ok(Self {
            socket,
            root_dir: root_dir.into(),
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Processes a single incoming UDP datagram and replies according to RFC 1350.
    pub fn handle_next_datagram(&self) -> std::io::Result<bool> {
        let mut buf = [0u8; 1024];
        let (len, peer) = match self.socket.recv_from(&mut buf) {
            Ok(res) => res,
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                return Ok(false);
            }
            Err(e) => return Err(e),
        };

        let Ok(req) = parse_request_packet(&buf[..len]) else {
            let err = encode_error_packet(ERR_ILLEGAL_OP, "Illegal TFTP operation");
            let _ = self.socket.send_to(&err, peer);
            return Ok(true);
        };

        if req.opcode == OP_WRQ {
            // Read-only server policy: reject WRQ with Access Violation (code 2)
            let err = encode_error_packet(ERR_ACCESS, "Read-only TFTP server");
            self.socket.send_to(&err, peer)?;
        } else if req.opcode == OP_RRQ {
            match resolve_child_safe(&self.root_dir, &req.filename) {
                Some(target) if target.is_file() => {
                    let bytes = fs::read(&target)?;
                    let first_chunk = &bytes[..bytes.len().min(TFTP_BLOCK_SIZE)];
                    let data_pkt = encode_data_packet(1, first_chunk);
                    self.socket.send_to(&data_pkt, peer)?;
                }
                _ => {
                    let err = encode_error_packet(ERR_NOT_FOUND, "File not found or access denied");
                    self.socket.send_to(&err, peer)?;
                }
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tftp::OP_DATA;

    #[test]
    fn test_local_fs_backend_and_sftp_ls_parser() {
        let dir = std::env::temp_dir().join(format!("onyxsh_fm_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let backend = LocalFsBackend;

        let sub = dir.join("configs");
        backend.mkdir(&sub).unwrap();
        fs::write(dir.join("router.conf"), b"hostname core-sw1\n").unwrap();

        let entries = backend.list_dir(&dir).unwrap();
        assert_eq!(entries.len(), 2);
        let _ = fs::remove_dir_all(&dir);

        let parsed = parse_sftp_ls_line(
            "/var/log",
            "-rw-r--r-- 1 root root 4096 Nov 4 12:00 syslog",
        )
        .expect("parsed ls line");
        assert_eq!(parsed.name, "syslog");
        assert_eq!(parsed.path, "/var/log/syslog");
        assert_eq!(parsed.permissions_octal, 0o644);
    }

    #[test]
    fn test_live_udp_tftp_server_serves_file_and_blocks_wrq() {
        let dir = std::env::temp_dir().join(format!("onyxsh_tftp_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("firmware.bin"), b"ONYX-FW-V1").unwrap();

        let server = UdpTftpServer::bind("127.0.0.1:0", &dir).unwrap();
        let server_addr = server.local_addr().unwrap();

        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();

        // 1. Send RRQ for firmware.bin
        let mut rrq = vec![0x00, 0x01];
        rrq.extend_from_slice(b"firmware.bin\0octet\0");
        client.send_to(&rrq, server_addr).unwrap();

        assert!(server.handle_next_datagram().unwrap());

        let mut resp = [0u8; 600];
        let (n, _) = client.recv_from(&mut resp).unwrap();
        assert_eq!(u16::from_be_bytes([resp[0], resp[1]]), OP_DATA);
        assert_eq!(u16::from_be_bytes([resp[2], resp[3]]), 1);
        assert_eq!(&resp[4..n], b"ONYX-FW-V1");
        let _ = fs::remove_dir_all(&dir);
    }
}
