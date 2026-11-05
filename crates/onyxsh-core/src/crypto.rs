//! Cryptographic utilities and SecureCRT V2-compatible session password encryption.
//! Port of `src/onyxsh/utils/crypto.py`.

use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SECURECRT_V2_PREFIX: &str = "02:";
const AES_BLOCK_SIZE: usize = 16;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    #[error("Invalid encrypted password format.")]
    InvalidFormat,
    #[error("Invalid encrypted password length.")]
    InvalidLength,
    #[error("Invalid encrypted password payload.")]
    InvalidPayload,
    #[error("Encrypted password integrity check failed.")]
    IntegrityCheckFailed,
    #[error("Invalid decrypted password encoding.")]
    InvalidEncoding,
}

/// Returns `true` when `value` starts with the `02:` SecureCRT V2 prefix.
pub fn is_securecrt_v2_password(value: &str) -> bool {
    value.starts_with(SECURECRT_V2_PREFIX)
}

/// Encrypts a plaintext password into a SecureCRT Password V2-compatible string (`02:<hex>`).
pub fn encrypt_session_password(
    password: &str,
    config_passphrase: &str,
) -> Result<String, CryptoError> {
    let plain_bytes = password.as_bytes();
    let plain_len = u32::try_from(plain_bytes.len()).map_err(|_| CryptoError::InvalidPayload)?;

    let digest = Sha256::digest(plain_bytes);
    let mut payload = Vec::with_capacity(4 + plain_bytes.len() + 32 + AES_BLOCK_SIZE);
    payload.extend_from_slice(&plain_len.to_le_bytes());
    payload.extend_from_slice(plain_bytes);
    payload.extend_from_slice(&digest);

    let padding_len = AES_BLOCK_SIZE - (payload.len() % AES_BLOCK_SIZE);
    // Deterministic pseudo-random padding derived from digest + length so encryption is safe and zero-syscall
    let pad_seed = Sha256::digest(&payload);
    payload.extend_from_slice(&pad_seed[..padding_len]);

    let key: [u8; 32] = Sha256::digest(config_passphrase.as_bytes()).into();
    let iv = [0u8; AES_BLOCK_SIZE];
    let encrypted = aes256_cbc_encrypt(&key, &iv, &payload);

    Ok(format!("{SECURECRT_V2_PREFIX}{}", hex_encode(&encrypted)))
}

/// Decrypts a SecureCRT Password V2-compatible value (`02:<hex>` or raw `<hex>`).
pub fn decrypt_session_password(
    value: &str,
    config_passphrase: &str,
) -> Result<String, CryptoError> {
    let cipher_hex = value.strip_prefix(SECURECRT_V2_PREFIX).unwrap_or(value);
    if cipher_hex.is_empty() {
        return Ok(String::new());
    }

    let ciphered_bytes = hex_decode(cipher_hex)?;
    if ciphered_bytes.is_empty() || ciphered_bytes.len() % AES_BLOCK_SIZE != 0 {
        return Err(CryptoError::InvalidLength);
    }

    let key: [u8; 32] = Sha256::digest(config_passphrase.as_bytes()).into();
    let iv = [0u8; AES_BLOCK_SIZE];
    let padded_plain = aes256_cbc_decrypt(&key, &iv, &ciphered_bytes);

    if padded_plain.len() < 4 + 32 {
        return Err(CryptoError::InvalidPayload);
    }

    let plain_len = u32::from_le_bytes([
        padded_plain[0],
        padded_plain[1],
        padded_plain[2],
        padded_plain[3],
    ]) as usize;

    let end_plain = 4usize
        .checked_add(plain_len)
        .ok_or(CryptoError::InvalidPayload)?;
    let end_digest = end_plain
        .checked_add(32)
        .ok_or(CryptoError::InvalidPayload)?;

    if end_digest > padded_plain.len() {
        return Err(CryptoError::InvalidPayload);
    }

    let plain_bytes = &padded_plain[4..end_plain];
    let digest = &padded_plain[end_plain..end_digest];

    let expected_digest = Sha256::digest(plain_bytes);
    if expected_digest.as_slice() != digest {
        return Err(CryptoError::IntegrityCheckFailed);
    }

    String::from_utf8(plain_bytes.to_vec()).map_err(|_| CryptoError::InvalidEncoding)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, CryptoError> {
    if hex.len() % 2 != 0 {
        return Err(CryptoError::InvalidFormat);
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let s = std::str::from_utf8(chunk).map_err(|_| CryptoError::InvalidFormat)?;
        let val = u8::from_str_radix(s, 16).map_err(|_| CryptoError::InvalidFormat)?;
        out.push(val);
    }
    Ok(out)
}

// --- Standard FIPS-197 AES-256 + CBC Mode implementation ---

const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const INV_SBOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

const RCON: [u8; 11] = [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

fn expand_key_256(key: &[u8; 32]) -> [[u8; 16]; 15] {
    let mut w = [[0u8; 4]; 60];
    for i in 0..8 {
        w[i] = [key[4 * i], key[4 * i + 1], key[4 * i + 2], key[4 * i + 3]];
    }
    for i in 8..60 {
        let mut temp = w[i - 1];
        if i % 8 == 0 {
            temp = [
                SBOX[temp[1] as usize] ^ RCON[i / 8],
                SBOX[temp[2] as usize],
                SBOX[temp[3] as usize],
                SBOX[temp[0] as usize],
            ];
        } else if i % 8 == 4 {
            temp = [
                SBOX[temp[0] as usize],
                SBOX[temp[1] as usize],
                SBOX[temp[2] as usize],
                SBOX[temp[3] as usize],
            ];
        }
        w[i] = [
            w[i - 8][0] ^ temp[0],
            w[i - 8][1] ^ temp[1],
            w[i - 8][2] ^ temp[2],
            w[i - 8][3] ^ temp[3],
        ];
    }
    let mut round_keys = [[0u8; 16]; 15];
    for r in 0..15 {
        for c in 0..4 {
            round_keys[r][4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]);
        }
    }
    round_keys
}

#[inline]
fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if (b & 1) != 0 {
            p ^= a;
        }
        let hi = (a & 0x80) != 0;
        a <<= 1;
        if hi {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    p
}

fn aes256_encrypt_block(block: &[u8; 16], rk: &[[u8; 16]; 15]) -> [u8; 16] {
    let mut s = *block;
    for i in 0..16 {
        s[i] ^= rk[0][i];
    }
    for round in 1..14 {
        for b in &mut s {
            *b = SBOX[*b as usize];
        }
        s = [
            s[0], s[5], s[10], s[15], s[4], s[9], s[14], s[3], s[8], s[13], s[2], s[7], s[12],
            s[1], s[6], s[11],
        ];
        for c in 0..4 {
            let i = 4 * c;
            let (s0, s1, s2, s3) = (s[i], s[i + 1], s[i + 2], s[i + 3]);
            s[i] = gmul(s0, 2) ^ gmul(s1, 3) ^ s2 ^ s3;
            s[i + 1] = s0 ^ gmul(s1, 2) ^ gmul(s2, 3) ^ s3;
            s[i + 2] = s0 ^ s1 ^ gmul(s2, 2) ^ gmul(s3, 3);
            s[i + 3] = gmul(s0, 3) ^ s1 ^ s2 ^ gmul(s3, 2);
        }
        for i in 0..16 {
            s[i] ^= rk[round][i];
        }
    }
    for b in &mut s {
        *b = SBOX[*b as usize];
    }
    s = [
        s[0], s[5], s[10], s[15], s[4], s[9], s[14], s[3], s[8], s[13], s[2], s[7], s[12], s[1],
        s[6], s[11],
    ];
    for i in 0..16 {
        s[i] ^= rk[14][i];
    }
    s
}

fn aes256_decrypt_block(block: &[u8; 16], rk: &[[u8; 16]; 15]) -> [u8; 16] {
    let mut s = *block;
    for i in 0..16 {
        s[i] ^= rk[14][i];
    }
    for round in (1..14).rev() {
        s = [
            s[0], s[13], s[10], s[7], s[4], s[1], s[14], s[11], s[8], s[5], s[2], s[15], s[12],
            s[9], s[6], s[3],
        ];
        for b in &mut s {
            *b = INV_SBOX[*b as usize];
        }
        for i in 0..16 {
            s[i] ^= rk[round][i];
        }
        for c in 0..4 {
            let i = 4 * c;
            let (s0, s1, s2, s3) = (s[i], s[i + 1], s[i + 2], s[i + 3]);
            s[i] = gmul(s0, 0x0e) ^ gmul(s1, 0x0b) ^ gmul(s2, 0x0d) ^ gmul(s3, 0x09);
            s[i + 1] = gmul(s0, 0x09) ^ gmul(s1, 0x0e) ^ gmul(s2, 0x0b) ^ gmul(s3, 0x0d);
            s[i + 2] = gmul(s0, 0x0d) ^ gmul(s1, 0x09) ^ gmul(s2, 0x0e) ^ gmul(s3, 0x0b);
            s[i + 3] = gmul(s0, 0x0b) ^ gmul(s1, 0x0d) ^ gmul(s2, 0x09) ^ gmul(s3, 0x0e);
        }
    }
    s = [
        s[0], s[13], s[10], s[7], s[4], s[1], s[14], s[11], s[8], s[5], s[2], s[15], s[12], s[9],
        s[6], s[3],
    ];
    for b in &mut s {
        *b = INV_SBOX[*b as usize];
    }
    for i in 0..16 {
        s[i] ^= rk[0][i];
    }
    s
}

fn aes256_cbc_encrypt(key: &[u8; 32], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let rk = expand_key_256(key);
    let mut out = Vec::with_capacity(data.len());
    let mut prev = *iv;
    for chunk in data.chunks_exact(16) {
        let mut block = [0u8; 16];
        for i in 0..16 {
            block[i] = chunk[i] ^ prev[i];
        }
        let enc = aes256_encrypt_block(&block, &rk);
        out.extend_from_slice(&enc);
        prev = enc;
    }
    out
}

fn aes256_cbc_decrypt(key: &[u8; 32], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let rk = expand_key_256(key);
    let mut out = Vec::with_capacity(data.len());
    let mut prev = *iv;
    for chunk in data.chunks_exact(16) {
        let mut block = [0u8; 16];
        block.copy_from_slice(chunk);
        let mut dec = aes256_decrypt_block(&block, &rk);
        for i in 0..16 {
            dec[i] ^= prev[i];
        }
        out.extend_from_slice(&dec);
        prev = block;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_securecrt_v2_encrypt_and_decrypt_roundtrip() {
        let secret = "My$uperS3cretP@ssw0rd!2026";
        let encrypted = encrypt_session_password(secret, "").expect("encryption should succeed");
        assert!(is_securecrt_v2_password(&encrypted));

        let decrypted = decrypt_session_password(&encrypted, "").expect("decryption should succeed");
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn test_decrypts_tampered_ciphertext_with_error() {
        let encrypted = encrypt_session_password("hunter2", "").unwrap();
        let mut tampered = encrypted.clone();
        tampered.replace_range(3..5, "ff");
        assert!(decrypt_session_password(&tampered, "").is_err());
    }

    #[test]
    fn test_cross_compatibility_with_python_pycryptodomex_vector() {
        let py_cipher = "02:04f3fcff53bb2134dcac727d4061c6233b57e6a9cc41f23d8236a2ef86455a7b70f50608813a8f45674d5522983c598b65de1c54045dcbccc87408062f3837d4";
        let plain = decrypt_session_password(py_cipher, "").expect("must decrypt Python vector");
        assert_eq!(plain, "OnyxSH#2026!");
    }
}
