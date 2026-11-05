//! RFC 1350 TFTP protocol parser, packet builder, and path confinement guard.
//! Port of `src/onyxsh/filemanager/tftp_server.py`.

use std::path::{Component, Path, PathBuf};

pub const OP_RRQ: u16 = 1;
pub const OP_WRQ: u16 = 2;
pub const OP_DATA: u16 = 3;
pub const OP_ACK: u16 = 4;
pub const OP_ERROR: u16 = 5;

pub const ERR_UNDEFINED: u16 = 0;
pub const ERR_NOT_FOUND: u16 = 1;
pub const ERR_ACCESS: u16 = 2;
pub const ERR_DISK_FULL: u16 = 3;
pub const ERR_ILLEGAL_OP: u16 = 4;
pub const ERR_UNKNOWN_ID: u16 = 5;
pub const ERR_EXISTS: u16 = 6;

pub const TFTP_BLOCK_SIZE: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TftpRequest {
    pub opcode: u16,
    pub filename: String,
    pub mode: String,
}

/// Parses an incoming RFC 1350 RRQ/WRQ packet (`[opcode:2][filename\0][mode\0]`).
pub fn parse_request_packet(data: &[u8]) -> Result<TftpRequest, &'static str> {
    if data.len() < 4 {
        return Err("Packet too short");
    }
    let opcode = u16::from_be_bytes([data[0], data[1]]);
    if opcode != OP_RRQ && opcode != OP_WRQ {
        return Err("Illegal TFTP request opcode");
    }
    let mut parts = data[2..].split(|&b| b == 0);
    let raw_file = parts.next().ok_or("Missing filename")?;
    let raw_mode = parts.next().ok_or("Missing mode")?;
    if raw_file.is_empty() || raw_mode.is_empty() {
        return Err("Malformed request");
    }
    let filename = std::str::from_utf8(raw_file)
        .map_err(|_| "Invalid UTF-8 filename")?
        .to_string();
    let mode = std::str::from_utf8(raw_mode)
        .map_err(|_| "Invalid ASCII mode")?
        .to_lowercase();
    if mode != "octet" && mode != "netascii" {
        return Err("Unsupported transfer mode");
    }
    Ok(TftpRequest {
        opcode,
        filename,
        mode,
    })
}

/// Encodes an RFC 1350 DATA packet (`[0x00, 0x03, block_hi, block_lo, payload...]`).
pub fn encode_data_packet(block_num: u16, payload: &[u8]) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(4 + payload.len());
    pkt.extend_from_slice(&OP_DATA.to_be_bytes());
    pkt.extend_from_slice(&block_num.to_be_bytes());
    pkt.extend_from_slice(payload);
    pkt
}

/// Encodes an RFC 1350 ACK packet (`[0x00, 0x04, block_hi, block_lo]`).
pub fn encode_ack_packet(block_num: u16) -> [u8; 4] {
    let op = OP_ACK.to_be_bytes();
    let blk = block_num.to_be_bytes();
    [op[0], op[1], blk[0], blk[1]]
}

/// Encodes an RFC 1350 ERROR packet (`[0x00, 0x05, err_hi, err_lo, msg..., 0x00]`).
pub fn encode_error_packet(error_code: u16, message: &str) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(5 + message.len());
    pkt.extend_from_slice(&OP_ERROR.to_be_bytes());
    pkt.extend_from_slice(&error_code.to_be_bytes());
    pkt.extend_from_slice(message.as_bytes());
    pkt.push(0);
    pkt
}

/// Resolves `requested` filename strictly inside `root`, rejecting any `..` traversal.
pub fn resolve_child_safe(root: &Path, requested: &str) -> Option<PathBuf> {
    let normalized = requested.replace('\\', "/");
    let clean = normalized.trim_start_matches('/');
    if clean.is_empty() || clean.contains('\0') {
        return None;
    }
    let rel = Path::new(clean);
    for comp in rel.components() {
        match comp {
            Component::Normal(_) => {}
            _ => return None, // Reject `..`, `.`, RootDir, Prefix
        }
    }
    Some(root.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tftp_packet_parsing_and_encoding() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&OP_RRQ.to_be_bytes());
        raw.extend_from_slice(b"router-backup.cfg\0octet\0");

        let req = parse_request_packet(&raw).expect("valid RRQ");
        assert_eq!(req.opcode, OP_RRQ);
        assert_eq!(req.filename, "router-backup.cfg");
        assert_eq!(req.mode, "octet");

        let ack = encode_ack_packet(1);
        assert_eq!(ack, [0x00, 0x04, 0x00, 0x01]);
    }

    #[test]
    fn test_tftp_path_confinement_blocks_traversal() {
        let root = Path::new("/srv/tftp");
        assert_eq!(
            resolve_child_safe(root, "switch/startup-config"),
            Some(PathBuf::from("/srv/tftp/switch/startup-config"))
        );
        assert!(resolve_child_safe(root, "../../etc/shadow").is_none());
        assert!(resolve_child_safe(root, "/../secret").is_none());
    }
}
