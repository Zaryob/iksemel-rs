/*
            iksemel - XML parser for Rust
          Copyright (C) 2024 Süleyman Poyraz
 This code is free software; you can redistribute it and/or
 modify it under the terms of the GNU Lesser General Public License
 as published by the Free Software Foundation; either version 2.1
 of the License, or (at your option) any later version.
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU Lesser General Public License for more details.
*/

use crate::{IksError, Result};
use hmac::digest::KeyInit;
use hmac::{Hmac, Mac};
use sha1::{Digest, Sha1};
use sha2::Sha256;

pub type HmacSha1 = Hmac<Sha1>;
pub type HmacSha256 = Hmac<Sha256>;

const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes binary data into a Base64 string according to RFC 4648.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        let idx0 = (b0 >> 2) as usize;
        let idx1 = (((b0 & 0x03) << 4) | (b1 >> 4)) as usize;
        let idx2 = (((b1 & 0x0f) << 2) | (b2 >> 6)) as usize;
        let idx3 = (b2 & 0x3f) as usize;

        out.push(B64_CHARS[idx0] as char);
        out.push(B64_CHARS[idx1] as char);

        if chunk.len() > 1 {
            out.push(B64_CHARS[idx2] as char);
        } else {
            out.push('=');
        }

        if chunk.len() > 2 {
            out.push(B64_CHARS[idx3] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Decodes a Base64 encoded string into bytes according to RFC 4648.
pub fn base64_decode(s: &str) -> Result<Vec<u8>> {
    let clean: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !clean.len().is_multiple_of(4) {
        return Err(IksError::BadBase64);
    }

    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    for chunk in clean.chunks(4) {
        let mut vals = [0u8; 4];
        let mut pad_count = 0;

        for (i, &b) in chunk.iter().enumerate() {
            if b == b'=' {
                pad_count += 1;
                vals[i] = 0;
            } else {
                if pad_count > 0 {
                    return Err(IksError::BadBase64);
                }
                vals[i] = decode_b64_byte(b).ok_or(IksError::BadBase64)?;
            }
        }

        let b0 = (vals[0] << 2) | (vals[1] >> 4);
        out.push(b0);

        if pad_count < 2 {
            let b1 = ((vals[1] & 0x0f) << 4) | (vals[2] >> 2);
            out.push(b1);
        }
        if pad_count < 1 {
            let b2 = ((vals[2] & 0x03) << 6) | vals[3];
            out.push(b2);
        }
    }

    Ok(out)
}

fn decode_b64_byte(b: u8) -> Option<u8> {
    match b {
        b'A'..=b'Z' => Some(b - b'A'),
        b'a'..=b'z' => Some(b - b'a' + 26),
        b'0'..=b'9' => Some(b - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Computes the SHA-1 digest of the given data.
pub fn sha1_hash(data: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 20];
    out.copy_from_slice(&result);
    out
}

/// Computes the SHA-1 digest and formats it as a lowercase hex string.
pub fn sha1_hex(data: &[u8]) -> String {
    hex::encode(sha1_hash(data))
}

/// Computes the SHA-256 digest of the given data.
pub fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Computes the SHA-256 digest and formats it as a lowercase hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha256_hash(data))
}

/// Computes HMAC-SHA-1 according to RFC 2104.
pub fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    let mut mac =
        <HmacSha1 as KeyInit>::new_from_slice(key).expect("HMAC supports arbitrary key length");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 20];
    out.copy_from_slice(&result);
    out
}

/// Computes HMAC-SHA-256 according to RFC 2104.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac =
        <HmacSha256 as KeyInit>::new_from_slice(key).expect("HMAC supports arbitrary key length");
    mac.update(data);
    let result = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// PBKDF2 Key Derivation Function with HMAC-SHA-1 according to RFC 2898 / RFC 5802.
pub fn pbkdf2_hmac_sha1(password: &[u8], salt: &[u8], iterations: u32, out: &mut [u8]) {
    pbkdf2_generic::<HmacSha1>(password, salt, iterations, out);
}

/// PBKDF2 Key Derivation Function with HMAC-SHA-256 according to RFC 2898 / RFC 7677.
pub fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32, out: &mut [u8]) {
    pbkdf2_generic::<HmacSha256>(password, salt, iterations, out);
}

fn pbkdf2_generic<M>(password: &[u8], salt: &[u8], iterations: u32, out: &mut [u8])
where
    M: Mac + KeyInit + Clone,
{
    let prf_mac =
        <M as KeyInit>::new_from_slice(password).expect("HMAC supports arbitrary key length");
    let mut block_idx = 1u32;
    let mut offset = 0;

    while offset < out.len() {
        let mut u_prev = {
            let mut mac = prf_mac.clone();
            mac.update(salt);
            mac.update(&block_idx.to_be_bytes());
            mac.finalize().into_bytes()
        };
        let mut u_xor = u_prev.clone();

        for _ in 1..iterations {
            let mut mac = prf_mac.clone();
            mac.update(&u_prev);
            u_prev = mac.finalize().into_bytes();
            for (x, u) in u_xor.iter_mut().zip(u_prev.iter()) {
                *x ^= u;
            }
        }

        let take = (out.len() - offset).min(u_xor.len());
        out[offset..offset + take].copy_from_slice(&u_xor[..take]);
        offset += take;
        block_idx += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");

        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("Zg==").unwrap(), b"f");
        assert_eq!(base64_decode("Zm8=").unwrap(), b"fo");
        assert_eq!(base64_decode("Zm9v").unwrap(), b"foo");
        assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
    }

    #[test]
    fn test_base64_invalid() {
        assert!(base64_decode("Zg=").is_err()); // wrong padding len
        assert!(base64_decode("Zg====").is_err());
        assert!(base64_decode("Z@==").is_err()); // invalid char
    }

    #[test]
    fn test_sha1() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            sha1_hex(b"The quick brown fox jumps over the lazy dog"),
            "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
        );
    }

    #[test]
    fn test_sha256() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"The quick brown fox jumps over the lazy dog"),
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592"
        );
    }

    #[test]
    fn test_hmac_sha1_and_sha256() {
        let key = b"key";
        let data = b"The quick brown fox jumps over the lazy dog";

        let mac1 = hmac_sha1(key, data);
        assert_eq!(
            hex::encode(mac1),
            "de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9"
        );

        let mac256 = hmac_sha256(key, data);
        assert_eq!(
            hex::encode(mac256),
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }

    #[test]
    fn test_pbkdf2_rfc6070_vectors() {
        // RFC 6070 PBKDF2-HMAC-SHA1 test vectors
        let mut out = [0u8; 20];

        pbkdf2_hmac_sha1(b"password", b"salt", 1, &mut out);
        assert_eq!(hex::encode(out), "0c60c80f961f0e71f3a9b524af6012062fe037a6");

        pbkdf2_hmac_sha1(b"password", b"salt", 2, &mut out);
        assert_eq!(hex::encode(out), "ea6c014dc72d6f8ccd1ed92ace1d41f0d8de8957");

        pbkdf2_hmac_sha1(b"password", b"salt", 4096, &mut out);
        assert_eq!(hex::encode(out), "4b007901b765489abead49d926f721d065a429c1");

        // PBKDF2-HMAC-SHA256 test vector
        let mut out32 = [0u8; 32];
        pbkdf2_hmac_sha256(b"password", b"salt", 4096, &mut out32);
        assert_eq!(
            hex::encode(out32),
            "c5e478d59288c841aa530db6845c4c8d962893a001ce4e11a4963873aa98134a"
        );
    }
}
