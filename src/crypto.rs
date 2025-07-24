/* 
            iksemel - XML parser for Rust
          Copyright (C) 2024 Süleyman Poyraz
 This code is free software; you can redistribute it and/or
 modify it under the terms of the Affero General Public License
 as published by the Free Software Foundation; either version 3
 of the License, or (at your option) any later version.
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 Affero General Public License for more details.
*/

use sha1::{Sha1, Digest};
use crate::{IksError, Result};

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
}
