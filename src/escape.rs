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

//! XML kaçışının tek çekirdeği.
//!
//! C `iksemel` kaçış için tek bir fonksiyon kullanır ve hem metin hem
//! attribute değerine aynı kuralları uygular. Bu modül o sözleşmeyi
//! kopyalar; çağıranlar yalnızca çıktıyı nasıl tükettiklerinde ayrışır:
//! tahsis eden `escape_to_string`, akıtan `write_escaped`.

// Task 4 çağrı yerlerini bu çekirdeğe taşıyınca bu iz kaldırılabilir. O ana
// kadar modülün API'si kullanılmadığından dead_code uyarısı üretmesini engeller.
#![allow(dead_code)]

use std::io::{self, Write};

/// Bir karakter için kaçış kararı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EscapeOut {
    /// Karakter olduğu gibi geçer.
    Literal,
    /// Karakter sabit bir varlık dizisiyle değişir.
    LiteralSlice(&'static str),
    /// Sayısal referans: tampon ve geçerli bayt sayısı.
    Numeric([u8; 10], u8),
}

/// Bu bayt, kaçış gerektirmeyen bir ASCII karakterine karşılık gelir mi?
///
/// C'nin `isprint(c) || c == '\t' || c == '\n' || c == '\r'` testinin bayt
/// düzeyindeki karşılığı. Çok baytlı bir karakterin *hiçbir* baytı bu testten
/// geçemez (hepsi ≥ 0x80), dolayısıyla bayt taraması bir dizinin ortasından
/// ilerleyemez.
pub(crate) fn is_literal_byte(b: u8) -> bool {
    ((0x20..=0x7E).contains(&b) && !matches!(b, b'&' | b'\'' | b'"' | b'<' | b'>'))
        || matches!(b, b'\t' | b'\n' | b'\r')
}

/// Bir karakterin kaçış çıktısı. `None` → karakter düşürülür.
pub(crate) fn escape_char(c: char) -> Option<EscapeOut> {
    match c {
        '&' => Some(EscapeOut::LiteralSlice("&amp;")),
        '<' => Some(EscapeOut::LiteralSlice("&lt;")),
        '>' => Some(EscapeOut::LiteralSlice("&gt;")),
        '\'' => Some(EscapeOut::LiteralSlice("&apos;")),
        '"' => Some(EscapeOut::LiteralSlice("&quot;")),
        '\t' | '\n' | '\r' => Some(EscapeOut::Literal),
        // C `U+0000` için hiçbir şey yazmaz.
        '\0' => None,
        // C'nin iki baytlık maske hatası (0xE8) kopyalanmaz: her zaman doğru
        // kod noktası yazılır (spec D9).
        c => {
            let cp = c as u32;
            if (0x20..=0x7E).contains(&cp) {
                Some(EscapeOut::Literal)
            } else {
                Some(EscapeOut::numeric(cp))
            }
        }
    }
}

impl EscapeOut {
    /// `snprintf("&#x%02x;", cp)` biçimini tahsis yapmadan üretir.
    fn numeric(cp: u32) -> EscapeOut {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut buf = [0u8; 10];
        buf[0] = b'&';
        buf[1] = b'#';
        buf[2] = b'x';

        // En fazla 5 hane gerekir (U+10FFFF); 8 baytlık yer fazlasıyla yeter.
        let mut digits = [0u8; 8];
        let mut n = 0usize;
        let mut v = cp;
        loop {
            digits[n] = HEX[(v & 0xF) as usize];
            v >>= 4;
            n += 1;
            if v == 0 {
                break;
            }
        }
        // En az iki hane.
        if n < 2 {
            digits[n] = b'0';
            n += 1;
        }

        let mut pos = 3usize;
        for k in (0..n).rev() {
            buf[pos] = digits[k];
            pos += 1;
        }
        buf[pos] = b';';
        pos += 1;

        EscapeOut::Numeric(buf, pos as u8)
    }
}

/// Kaçışlanmış uzunluğu döndürür.
///
/// **Sözleşme:** dönen değer `write_escaped`'in yazacağı bayt sayısına her
/// zaman eşittir. C'de bu eşitliğin bozulması heap taşmasına yol açar.
pub(crate) fn escaped_len(s: &str) -> usize {
    let mut n = 0usize;
    for c in s.chars() {
        n += match escape_char(c) {
            Some(EscapeOut::Literal) => c.len_utf8(),
            Some(EscapeOut::LiteralSlice(lit)) => lit.len(),
            Some(EscapeOut::Numeric(_, len)) => len as usize,
            None => 0,
        };
    }
    n
}

/// `s`'i kaçışlayarak `w`'ye yazar. Ara tahsis yapmaz.
pub(crate) fn write_escaped<W: Write>(w: &mut W, s: &str) -> io::Result<()> {
    // Hızlı yol: kaçılacak ya da ASCII dışı hiçbir şey yok.
    if s.bytes().all(is_literal_byte) {
        return w.write_all(s.as_bytes());
    }

    let bytes = s.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if is_literal_byte(bytes[i]) {
            i += 1;
            continue;
        }

        // Buradan itibaren ya kaçılacak bir ASCII karakteri ya da çok baytlı
        // bir karakterin başlangıcındayız.
        let c = s[i..].chars().next().expect("geçerli UTF-8 sınırı");

        if i > start {
            w.write_all(&bytes[start..i])?;
        }

        match escape_char(c) {
            Some(EscapeOut::LiteralSlice(lit)) => w.write_all(lit.as_bytes())?,
            Some(EscapeOut::Numeric(buf, len)) => w.write_all(&buf[..len as usize])?,
            Some(EscapeOut::Literal) => {
                // Çok baytlı karakterler daima `Numeric` döner; buraya ancak
                // tek baytlık bir karakter düşebilir.
                let mut tmp = [0u8; 4];
                w.write_all(c.encode_utf8(&mut tmp).as_bytes())?;
            }
            None => {}
        }

        i += c.len_utf8();
        start = i;
    }

    if start < bytes.len() {
        w.write_all(&bytes[start..])?;
    }
    Ok(())
}

/// `s`'i kaçışlayıp yeni bir `String` döndürür.
///
/// `String` `std::io::Write` uygulamadığı için `write_escaped` doğrudan
/// kullanılamaz; bu yardımcı tek bir `Vec<u8>` üzerinden çalışır.
pub(crate) fn escape_to_string(s: &str) -> String {
    let mut buf = Vec::with_capacity(escaped_len(s));
    write_escaped(&mut buf, s).expect("Vec'e yazmak başarısız olamaz");
    // Girdi geçerli UTF-8'tir ve kaçış çıktıları ASCII'dir.
    String::from_utf8(buf).expect("kaçış çıktısı daima geçerli UTF-8'tir")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn esc(s: &str) -> String {
        escape_to_string(s)
    }

    /// C oracle çıktısı: `café ü` → `caf&#xe9; &#xfc;`.
    #[test]
    fn matches_c_oracle_for_latin1() {
        assert_eq!(esc("café ü"), "caf&#xe9; &#xfc;");
    }

    /// C tek fonksiyon kullanır: beş varlık **her iki bağlamda** kaçılır.
    #[test]
    fn all_five_entities_are_escaped_in_both_contexts() {
        assert_eq!(esc("&<>'\""), "&amp;&lt;&gt;&apos;&quot;");
        assert_eq!(esc(r#"Say "hi""#), "Say &quot;hi&quot;");
        assert_eq!(esc("it's"), "it&apos;s");
    }

    /// C: `isprint(c) || \t \n \r` birebir geçer.
    #[test]
    fn tab_newline_cr_are_literal() {
        assert_eq!(esc("a\tb\nc\rd"), "a\tb\nc\rd");
    }

    /// C: kontrol karakterleri ve DEL → `&#x%02x;` (küçük harf, ≥ 2 hane).
    #[test]
    fn ascii_controls_use_two_digit_lowercase_hex() {
        assert_eq!(esc("\u{01}"), "&#x01;");
        assert_eq!(esc("\u{0b}"), "&#x0b;");
        assert_eq!(esc("\u{1f}"), "&#x1f;");
        assert_eq!(esc("\u{7f}"), "&#x7f;");
        // Boşluk yazdırılabilir sayılır.
        assert_eq!(esc(" "), " ");
    }

    /// ASCII dışı: kod noktası yazılır, UTF-8 baytları değil.
    #[test]
    fn non_ascii_uses_code_point_not_bytes() {
        assert_eq!(esc("\u{80}"), "&#x80;");
        assert_eq!(esc("\u{9f}"), "&#x9f;");
        assert_eq!(esc("\u{a0}"), "&#xa0;");
        assert_eq!(esc("€"), "&#x20ac;");
        assert_eq!(esc("😀"), "&#x1f600;");
        assert_eq!(esc("\u{10FFFF}"), "&#x10ffff;");
    }

    /// C: `U+0000` için hiçbir şey yazılmaz.
    #[test]
    fn nul_is_dropped() {
        assert_eq!(esc("a\0b"), "ab");
        assert_eq!(esc("\0"), "");
        assert_eq!(escape_char('\0'), None);
    }

    /// `escaped_len` her zaman `write_escaped`'in yazdığı bayt sayısına eşit.
    /// C'de bu eşitliğin bozulması tampon taşmasının kaynağıdır (spec §1.4).
    #[test]
    fn escaped_len_always_matches_actual_output() {
        let cases = [
            "",
            "plain ascii",
            "&<>'\"",
            "a\tb\nc\rd",
            "\u{01}\u{7f}",
            "café ü",
            "€ 😀 ğüşıöç ĞÜŞİÖÇ",
            "\u{10FFFF}",
            "a\0b",
            "karışık & café <\u{7f}> \"tırnak\"",
        ];
        for s in cases {
            let mut buf = Vec::new();
            write_escaped(&mut buf, s).unwrap();
            assert_eq!(escaped_len(s), buf.len(), "escaped_len uyuşmuyor: {:?}", s);
            assert_eq!(String::from_utf8(buf).unwrap(), esc(s));
        }
    }

    /// Sabit bayt öneki: yazdırılabilir ASCII + \t \n \r, beş varlık hariç.
    #[test]
    fn literal_bytes_cover_ascii_printable_plus_whitespace() {
        for b in [b'a', b'~', b' ', b'\t', b'\n', b'\r', b'0', b'Z'] {
            assert!(is_literal_byte(b), "literal olmalı: {:?}", b as char);
        }
        for b in [b'&', b'\'', b'"', b'<', b'>', 0x1F, 0x7F, 0x80, 0xC3, 0x00] {
            assert!(!is_literal_byte(b), "literal olmamalı: {:#04x}", b);
        }
    }

    /// Çok baytlı karakterlerin ortasından bayt bayt geçilmez.
    #[test]
    fn multibyte_chars_are_never_split_by_the_byte_scan() {
        assert_eq!(esc("a€b"), "a&#x20ac;b");
        assert_eq!(esc("€€"), "&#x20ac;&#x20ac;");
    }

    /// `Numeric` tamponu en büyük kod noktasında taşmaz.
    #[test]
    fn numeric_buffer_holds_the_largest_code_point() {
        match escape_char('\u{10FFFF}') {
            Some(EscapeOut::Numeric(buf, len)) => {
                assert_eq!(&buf[..len as usize], b"&#x10ffff;");
                assert!(buf.len() >= len as usize);
            }
            other => panic!("Numeric beklenirdi: {:?}", other),
        }
    }
}
