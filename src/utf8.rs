/*
            iksemel - XML parser for Rust
          Copyright (C) 2026 Süleyman Poyraz
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

/// Ağdan gelen bayt parçalarını artımlı olarak UTF-8'e çözer.
///
/// Bir okuma çok baytlı bir karakterin ortasında bitebilir (TCP parça
/// sınırları keyfîdir). Yarım kalan dizi bir sonraki çağrıya taşınır;
/// yalnızca *kesinlikle* geçersiz baytlar hata verir.
///
/// Bu, `std::str::from_utf8`'in `Utf8Error::error_len()` ayrımına dayanır:
/// `None` → "girdi erken bitti, kuyruğu tamponla"; `Some(_)` → "kesin
/// geçersiz, reddet".
pub(crate) struct Utf8Carry {
    /// Bir önceki çağrıdan devredilen yarım dizinin baytları (en fazla 3).
    pending: [u8; 3],
    pending_len: usize,
    /// `pending` dolu iken birleştirme için kullanılan tampon.
    staging: Vec<u8>,
}

impl Utf8Carry {
    pub(crate) fn new() -> Self {
        Utf8Carry {
            pending: [0u8; 3],
            pending_len: 0,
            staging: Vec::new(),
        }
    }

    /// Taşınan yarım diziyi temizler (yeni akış / StartTLS sonrası).
    pub(crate) fn reset(&mut self) {
        self.pending_len = 0;
        self.staging.clear();
    }

    /// `chunk`'ı besler ve geçerli UTF-8 olan en uzun öneki döndürür.
    ///
    /// Kesinlikle geçersiz bir bayt dizisi varsa `IksError::BadXml`.
    pub(crate) fn feed<'a>(&'a mut self, chunk: &'a [u8]) -> Result<&'a str> {
        // Kuyruk yoksa doğrudan `chunk` üzerinde çalış: kopyasız yol.
        if self.pending_len == 0 {
            let (n, tail_len) = Self::valid_prefix(chunk)?;
            self.pending[..tail_len].copy_from_slice(&chunk[n..]);
            self.pending_len = tail_len;
            return Ok(std::str::from_utf8(&chunk[..n])
                .expect("valid_prefix geçerli bir UTF-8 öneki döndürdü"));
        }

        // Kuyruk varsa birleştirilmiş tampon üzerinde çalış.
        let carried = self.pending_len;
        self.staging.clear();
        self.staging.extend_from_slice(&self.pending[..carried]);
        self.staging.extend_from_slice(chunk);

        // `valid_prefix` ya kesin geçersizde `Err` döner ya da artakalan
        // kuyruk tamamlanmamış geçerli bir dizi önekidir (en fazla 3 bayt);
        // bu yüzden kuyruk her zaman `pending` tamponuna sığar. Taşan kuyruk
        // zaten `valid_prefix` içinde `tail_len > 3` kontrolüyle reddedilir.
        //
        // Not: kuyruk yeni parçayla birleşince hâlâ yarım kalabilir; bu
        // durumda `valid_up_to()` `carried`'dan küçük olur (birleşik dizi
        // baştan itibaren tamamlanmamıştır) ve bu bir hata değildir.
        let (n, tail_len) = Self::valid_prefix(&self.staging)?;
        self.pending[..tail_len].copy_from_slice(&self.staging[n..]);
        self.pending_len = tail_len;

        Ok(std::str::from_utf8(&self.staging[..n])
            .expect("valid_prefix geçerli bir UTF-8 öneki döndürdü"))
    }

    /// Geçerli bayt önekinin uzunluğunu ve artakalan kuyruk uzunluğunu döndürür.
    fn valid_prefix(bytes: &[u8]) -> Result<(usize, usize)> {
        match std::str::from_utf8(bytes) {
            Ok(s) => Ok((s.len(), 0)),
            Err(e) => match e.error_len() {
                // Kesin geçersiz dizi: hemen reddet.
                Some(_) => Err(IksError::BadXml),
                // Girdi erken bitti: kuyruk tamamlanmamış bir dizidir.
                None => {
                    let n = e.valid_up_to();
                    let tail_len = bytes.len() - n;
                    if tail_len > 3 {
                        // `error_len() == None` için bu asla olmaz; savunma amaçlı.
                        return Err(IksError::BadXml);
                    }
                    Ok((n, tail_len))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `pieces` dilimlerini sırayla besler ve birleşik çıktıyı döndürür.
    fn feed_in_pieces(carry: &mut Utf8Carry, pieces: &[&[u8]]) -> Result<String> {
        let mut out = String::new();
        for p in pieces {
            out.push_str(carry.feed(p)?);
        }
        Ok(out)
    }

    #[test]
    fn euro_split_inside_sequence_is_reassembled() {
        // '€' = E2 82 AC
        let euro = "€";
        assert_eq!(euro.as_bytes(), &[0xE2, 0x82, 0xAC]);

        for pieces in [
            vec![&[0xE2, 0x82][..], &[0xAC][..]],
            vec![&[0xE2][..], &[0x82, 0xAC][..]],
            vec![&[0xE2][..], &[0x82][..], &[0xAC][..]],
        ] {
            let mut carry = Utf8Carry::new();
            let out = feed_in_pieces(&mut carry, &pieces).unwrap();
            assert_eq!(out, euro, "parçalar: {:?}", pieces);
        }
    }

    #[test]
    fn four_byte_emoji_split_every_way() {
        // '😀' = F0 9F 98 80
        let emoji = "😀";
        let b = emoji.as_bytes();
        for cut in 1..4usize {
            let mut carry = Utf8Carry::new();
            let out = feed_in_pieces(&mut carry, &[&b[..cut], &b[cut..]]).unwrap();
            assert_eq!(out, emoji, "kesim noktası {}", cut);
        }
    }

    #[test]
    fn partial_tail_is_buffered_not_emitted() {
        // Yalnızca yarım dizi geldi: hiçbir şey yayınlanmamalı.
        let mut carry = Utf8Carry::new();
        assert_eq!(carry.feed(&[0xE2, 0x82]).unwrap(), "");
        // Tam ASCII öneki yayınlanır, yarım dizi kuyrukta kalır.
        let mut carry = Utf8Carry::new();
        assert_eq!(carry.feed(b"ab\xE2").unwrap(), "ab");
        assert_eq!(carry.feed(&[0x82, 0xAC]).unwrap(), "€");
    }

    #[test]
    fn definitely_invalid_is_rejected_immediately() {
        // 0xFF geçerli UTF-8'de bulunmaz → error_len() == Some(1).
        let mut carry = Utf8Carry::new();
        assert!(matches!(carry.feed(&[0xFF]), Err(IksError::BadXml)));

        // E2'den sonra 28 gelirse dizi kesin geçersizdir.
        let mut carry = Utf8Carry::new();
        assert!(matches!(carry.feed(&[0xE2, 0x28]), Err(IksError::BadXml)));

        // Geçersiz bayt, yarım dizinin tamamlanması sırasında da yakalanır.
        let mut carry = Utf8Carry::new();
        assert_eq!(carry.feed(&[0xE2]).unwrap(), "");
        assert!(matches!(carry.feed(&[0x28]), Err(IksError::BadXml)));

        // Fazla uzun kodlama (overlong) da reddedilir.
        let mut carry = Utf8Carry::new();
        assert!(matches!(carry.feed(&[0xC0, 0xAF]), Err(IksError::BadXml)));
    }

    #[test]
    fn reset_clears_pending_tail() {
        let mut carry = Utf8Carry::new();
        assert_eq!(carry.feed(&[0xE2, 0x82]).unwrap(), "");

        // Sıfırlama olmadan devam edilirse dizi tamamlanır.
        let mut still_pending = Utf8Carry::new();
        assert_eq!(still_pending.feed(&[0xE2, 0x82]).unwrap(), "");
        assert_eq!(still_pending.feed(&[0xAC]).unwrap(), "€");

        // Sıfırlandıktan sonra kuyruk boşalır: AC tek başına geçersiz bir
        // başlangıç baytıdır.
        carry.reset();
        assert!(matches!(carry.feed(&[0xAC]), Err(IksError::BadXml)));
    }

    #[test]
    fn ascii_and_empty_chunks_pass_through() {
        let mut carry = Utf8Carry::new();
        assert_eq!(carry.feed(b"").unwrap(), "");
        assert_eq!(carry.feed(b"<a/>").unwrap(), "<a/>");
        assert_eq!(carry.feed(b"").unwrap(), "");
        assert_eq!(carry.feed(b"x").unwrap(), "x");
    }
}
