# Alt Proje A — Parser, Transport ve Serileştirme Doğruluğu: Uygulama Planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** iksemel-rs'in ayrıştırma, ağdan okuma ve serileştirme davranışını C `iksemel` (commit `5abdbce`) ile bayt-birebir hizalamak; böylece C kütüphanesini kullanan uygulamalar Rust sürümüne taşınabilir.

**Architecture:** Beş bağımsız doğruluk ekseni (A1 artımlı UTF-8, A2 DOM tamamlanma, A3 metin doğruluğu, A4 limitler + yasak bayt, A5 kaçış birleştirme). İki yeni crate-içi modül (`utf8`, `escape`) çekirdek mantığı barındırır; mevcut modüller bu çekirdeklere delege eder. C kaynağı ve derlenmiş oracle her davranışın referansıdır.

**Tech Stack:** Rust 2021, `cargo test`, `tokio` (async transport testleri), `std::net::TcpListener` (senkron transport testleri), `cc` ile derlenen C oracle.

**Spec:** `docs/superpowers/specs/2026-10-09-iksemel-rs-A-parser-transport-serialization.md`

## Global Constraints

- `#![forbid(unsafe_code)]` — hiçbir görev `unsafe` ekleyemez.
- Genel API imzaları **değişmez**. Yalnızca davranış değişir. Tek yeni genel API: `Parser::finish()` ve `SaxHandler::on_finish()` (varsayılan gövdeli olmak zorunda).
- `SaxHandler`'ı uygulayan tüm mevcut tipler değişmeden derlenmeye devam etmeli: `tools/iksperf.rs`, `tools/ikslint.rs`, `benches/comparison.rs`, `tests/parser_stress.rs`, `src/stream.rs`, `src/dom.rs`.
- `IksNode` alanları (`name`/`content`/`children`/`attributes`) **crate kökünde private**. Crate içindeki birim testleri (`src/**`) alanlara doğrudan erişebilir; `tests/**` entegrasyon testleri **erişemez** ve erişimcileri kullanmalıdır: `.name()`, `.content()`, `.children()`, `.attributes()`, `.node_type()`.
- `String` `std::io::Write` **uygulamaz** (yalnızca `fmt::Write`). `io::Write` isteyen yerlere `Vec<u8>` verilir; `String` üreten yollar `escape::escape_to_string` kullanır.
- Tüm kod, yorum ve commit mesajları **Türkçe**.
- Commit mesajları `Co-Authored-By: Claude Code <noreply@anthropic.com>` ile biter.
- Başlangıç durumu: `cargo test --all-features` **yeşil** (95 test + 3 doctest). Hiçbir görev bu sayının altına düşemez.
- C reposuna (`~/Development/iksemel`) hiçbir görev yazmaz; oracle olarak derlenir ve çalıştırılır.
- Oracle derleme: `mkdir -p /tmp/iksprobe && cc -w -I/Users/zaryob/Development/iksemel/include -o /tmp/iksprobe/<out> <prog>.c /Users/zaryob/Development/iksemel/src/{iks,sax,dom,ikstack,utility}.c`

**Bilinen ve kasıtlı sapmalar (spec §8) — düzeltilecek hata değil, test istisnasıdır:**
- `U+0200`–`U+03FF` ve `U+0600`–`U+07FF` (D9): Rust doğru kod noktasını üretir, C `0xE8` maske hatası yüzünden çöp üretir.
- `U+0000`: Görev 5'ten sonra iki taraf da reddeder.
- Kapanmamış belge: C `IKS_OK` + boş kök verir, Rust `BadXml` verir (D1 — `finish`'e anlam kazandırma kararı).

## Review Focus

Spec'in ima ettiği ama hiçbir görevin testinin kendiliğinden kapsamadığı beş girdi sınıfı. Her biri, sahibi olan göreve test olarak yazıldı:

1. **Chunk sınırında birleşen çok baytlı karakter.** Bir okuma `0xE2 0x82` ile bitip sonraki `0xAC` ile başlarsa, artımlı çözücü olmadan ikinci okuma geçersiz UTF-8 sayılır ve bağlantı düşer. → Görev 2, dört `*_reassembles_*` testi.
2. **Kapanmamış belge ve sarkan kapanış etiketi.** `<r><c/>` ya da `<a/></b>` girdisinde kökü hiçbir zaman teslim etmemek; `parse_str`'ın `Option` yerine hata döndürmesi gerekir. → Görev 10, `unfinished_document_is_an_error` ve `stray_closing_tag_is_an_error`.
3. **Karma içerikli CDATA'nın pretty-print'te kırpılması.** `" ab "` içeriği `trim()` ile `"ab"`ye dönüşürse veri sessizce kaybolur. → Görev 9, `pretty_mode_does_not_trim_cdata_content`.
4. **Pretty-printed girdide `find_cdata`.** İlk çocuk boşluk CDATA'sı olduğunda C onu **boşluklarıyla** döndürür; çağıranın kırpılmış varsayması sessiz bir sözleşme hatasıdır. → Görev 8, `find_cdata_returns_whitespace_cdata_verbatim`.
5. **Parça parça gelen metnin birleşmemesi.** `<body>ab` + `cd</body>` iki `parse_chunk` çağrısıyla gelirse C tek CDATA düğümü tutar; iki düğüm tutulursa `text()` ve `find_cdata` farklı sonuç verir. → Görev 7, `text_across_chunks_merges_into_one_node`.

---

## Görev 1: `Utf8Carry` çekirdeği (A1a)

**Files:**
- Create: `src/utf8.rs`
- Modify: `src/lib.rs` — modül bildirimleri (`mod helper;` satırının yanına `mod utf8;`)

**Interfaces:**
- Consumes: `crate::{IksError, Result}`
- Produces:
  - `pub(crate) struct Utf8Carry`
  - `pub(crate) fn Utf8Carry::new() -> Utf8Carry`
  - `pub(crate) fn Utf8Carry::reset(&mut self)`
  - `pub(crate) fn Utf8Carry::feed<'a>(&'a mut self, chunk: &'a [u8]) -> Result<&'a str>`

- [ ] **Adım 1: Başarısız testleri yaz**

`src/utf8.rs` sonuna:

```rust
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
```

- [ ] **Adım 2: Testin başarısız olduğunu doğrula**

Run: `cargo test --lib utf8 2>&1 | tail -20`
Expected: FAIL — `error[E0433]: failed to resolve: use of undeclared crate or module 'utf8'` (modül henüz bildirilmedi).

- [ ] **Adım 3: Implementasyonu yaz**

`src/utf8.rs` (test modülünün üstüne):

```rust
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
            return Ok(&chunk[..n]);
        }

        // Kuyruk varsa birleştirilmiş tampon üzerinde çalış.
        let carried = self.pending_len;
        self.staging.clear();
        self.staging.extend_from_slice(&self.pending[..carried]);
        self.staging.extend_from_slice(chunk);

        let (n, _) = Self::valid_prefix(&self.staging)?;

        // `pending` daima geçerli bir dizinin önekidir, dolayısıyla
        // `valid_up_to()` ya `carried`'a eşit ya da ondan büyüktür. Bu
        // kontrol, kuyruğun 3 baytlık tamponu taşırmasını yapısal olarak
        // imkânsız kılar.
        if n < carried {
            self.pending_len = 0;
            return Err(IksError::BadXml);
        }

        let tail_len = self.staging.len() - n;
        self.pending[..tail_len].copy_from_slice(&self.staging[n..]);
        self.pending_len = tail_len;

        Ok(&self.staging[..n])
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
```

`src/lib.rs` modül bildirimlerine ekle (`mod helper;` satırının yanına):

```rust
mod utf8;
```

- [ ] **Adım 4: Testlerin geçtiğini doğrula**

Run: `cargo test --lib utf8 2>&1 | tail -20`
Expected: PASS — 6 test.

- [ ] **Adım 5: Clippy ve tam paket**

Run: `cargo clippy --all-targets --all-features 2>&1 | tail -20 && cargo test --all-features 2>&1 | grep -E "test result|FAILED"`
Expected: clippy uyarısız; hiç `FAILED` yok.

- [ ] **Adım 6: Commit**

```bash
git add src/utf8.rs src/lib.rs
git commit -m "feat(parser): artımlı UTF-8 çözücü Utf8Carry ekle

Ağdan gelen parçalar çok baytlı bir karakterin ortasında bitebilir.
from_utf8'in error_len() ayrımıyla yarım kalan dizi tamponlanır, kesin
geçersizi ise anında reddedilir.

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 2: `Utf8Carry`'yi üç transport yoluna bağla (A1b)

**Files:**
- Modify: `src/net.rs:56-64` (struct), `src/net.rs:74-110` (`connect`), `src/net.rs:111-122` (`from_tcp_stream`), `src/net.rs:181-198` (`start_stream`), `src/net.rs:200-238` (`recv_event`), `src/net.rs:250-283` (`start_tls`)
- Modify: `src/async_net.rs:45-50` (`AsyncReceiver`), `src/async_net.rs:54-79` (`AsyncReceiver::recv_event`), `src/async_net.rs:147-155` (`AsyncConnection`), `src/async_net.rs:176-184` (`connect`), `src/async_net.rs:226-235` (`start_stream`), `src/async_net.rs:243-274` (`recv_event`), `src/async_net.rs:288-303` (`split`), `src/async_net.rs:368` (`start_tls`)
- Test: `tests/utf8_chunk_boundaries.rs` (yeni)

**Interfaces:**
- Consumes: `crate::utf8::Utf8Carry` — `new()`, `reset()`, `feed<'a>(&'a mut self, &'a [u8]) -> Result<&'a str>` (Görev 1)
- Produces: değişen genel API yok; üç struct iç alan kazanır.

- [ ] **Adım 1: Başarısız entegrasyon testlerini yaz**

`tests/utf8_chunk_boundaries.rs`:

```rust
//! Çok baytlı bir karakterin ağ paketi sınırına denk gelmesi.
//!
//! Her transport yolu için ayrı test: regresyonun hangi yolda olduğunu
//! göstermelidir.

use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use iksemel::{AsyncConnection, Connection, StreamEvent};

/// `<stream:stream ...>` başlığı ve gövdesinde '€' bulunan tek bir mesaj.
const STREAM: &[u8] = b"<stream:stream xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams' to='example.com' version='1.0'><message type='chat'><body>fiyat: \xE2\x82\xAC</body></message>";

/// '€' dizisinin ilk baytının konumu.
fn euro_start() -> usize {
    STREAM.iter().position(|&b| b == 0xE2).expect("euro")
}

/// Sunucu, `payload`'ı `splits` konumlarından bölerek gönderir.
fn serve_split(payload: &'static [u8], splits: Vec<usize>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("accept");
        let mut prev = 0;
        for s in splits.iter().chain(std::iter::once(&payload.len())) {
            sock.write_all(&payload[prev..*s]).expect("write");
            sock.flush().expect("flush");
            // Parça sınırının gerçekten ayrı bir okuma olması için bekle.
            std::thread::sleep(Duration::from_millis(20));
            prev = *s;
        }
        std::thread::sleep(Duration::from_millis(300));
    });
    port
}

fn first_stanza_body(events: impl Iterator<Item = StreamEvent>) -> String {
    let stanza = events
        .find_map(|e| match e {
            StreamEvent::Stanza(s) => Some(s),
            _ => None,
        })
        .expect("stanza gelmeli");
    stanza
        .find_cdata("body")
        .expect("body metni bulunmalı")
}

#[test]
fn sync_connection_reassembles_split_euro() {
    // E2 | 82 AC — dizi ilk bayttan sonra bölünür.
    let port = serve_split(STREAM, vec![euro_start() + 1]);

    let mut conn = Connection::connect("127.0.0.1", port, "example.com", None).expect("connect");
    let mut events = Vec::new();
    for _ in 0..16 {
        let e = conn.recv_event().expect("recv");
        let done = matches!(e, StreamEvent::Stanza(_));
        events.push(e);
        if done {
            break;
        }
    }
    assert_eq!(first_stanza_body(events.into_iter()), "fiyat: €");
}

#[test]
fn sync_connection_reassembles_two_byte_split_euro() {
    // E2 82 | AC — dizi son bayttan önce bölünür.
    let port = serve_split(STREAM, vec![euro_start() + 2]);

    let mut conn = Connection::connect("127.0.0.1", port, "example.com", None).expect("connect");
    let mut events = Vec::new();
    for _ in 0..16 {
        let e = conn.recv_event().expect("recv");
        let done = matches!(e, StreamEvent::Stanza(_));
        events.push(e);
        if done {
            break;
        }
    }
    assert_eq!(first_stanza_body(events.into_iter()), "fiyat: €");
}

#[test]
fn async_connection_reassembles_split_euro() {
    let port = serve_split(STREAM, vec![euro_start() + 1]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut conn = AsyncConnection::connect("127.0.0.1", port, "example.com", None)
            .await
            .expect("connect");
        let mut events = Vec::new();
        for _ in 0..16 {
            let e = conn.recv_event().await.expect("recv");
            let done = matches!(e, StreamEvent::Stanza(_));
            events.push(e);
            if done {
                break;
            }
        }
        assert_eq!(first_stanza_body(events.into_iter()), "fiyat: €");
    });
}

#[test]
fn split_receiver_reassembles_split_euro() {
    let port = serve_split(STREAM, vec![euro_start() + 2]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let conn = AsyncConnection::connect("127.0.0.1", port, "example.com", None)
            .await
            .expect("connect");
        let (_sender, mut receiver) = conn.split().expect("split");
        let mut events = Vec::new();
        for _ in 0..16 {
            let e = receiver.recv_event().await.expect("recv");
            let done = matches!(e, StreamEvent::Stanza(_));
            events.push(e);
            if done {
                break;
            }
        }
        assert_eq!(first_stanza_body(events.into_iter()), "fiyat: €");
    });
}
```

- [ ] **Adım 2: Testlerin başarısız olduğunu doğrula**

Run: `cargo test --test utf8_chunk_boundaries 2>&1 | tail -30`
Expected: FAIL — `recv` hatası (`NetDropped` / `BadXml`): bölünmüş `0xE2` / `0xE2 0x82` geçersiz UTF-8 sayılıp reddedilir, bu yüzden stanza hiç gelmez.

- [ ] **Adım 3: Üç yolu `Utf8Carry`'ye geçir**

`src/net.rs` — struct'a alan ekle (satır 56-64):

```rust
pub struct Connection {
    stream: Option<ConnectionStream>,
    parser: StreamParser,
    utf8: crate::utf8::Utf8Carry,
    pending_events: VecDeque<StreamEvent>,
    domain: String,
    timeout: Option<Duration>,
    allow_insecure_tls: bool,
    log_traffic: bool,
}
```

İki kurucuda (`connect` ~74, `from_tcp_stream` ~111) `parser: StreamParser::new(),` satırının hemen altına:

```rust
            utf8: crate::utf8::Utf8Carry::new(),
```

`start_stream` (~181) ve `start_tls` (~250) içinde `self.parser.reset();` satırının hemen altına:

```rust
        self.utf8.reset();
```

`recv_event`'te (~200) dönüşümü değiştir:

```rust
            let text = self.utf8.feed(&buf[..n])?;
            if self.log_traffic {
                print!("RECV: {}", text);
            }
            let events = self.parser.parse_chunk(text)?;
            self.pending_events.extend(events);
```

Bu satırlar derlenir: `text` yalnızca `self.utf8`'i ödünç alır, `self.parser` ayrı bir alandır — Rust alan-ayrık ödünçlemeye izin verir — ve `parse_chunk` sahiplenilmiş bir `Vec<StreamEvent>` döndürdüğü için `text`'in ödüncü `extend` satırına taşınmaz.

`src/async_net.rs` — `AsyncReceiver`'a alan ekle (satır 45-50):

```rust
pub struct AsyncReceiver {
    reader: tokio::io::ReadHalf<AsyncConnectionStream>,
    parser: StreamParser,
    utf8: crate::utf8::Utf8Carry,
    pending_events: VecDeque<StreamEvent>,
    timeout: Option<Duration>,
}
```

`AsyncReceiver::recv_event` (~54) içindeki dönüşümü değiştir:

```rust
            let chunk_str = self.utf8.feed(&buf[..n])?;
            let events = self.parser.parse_chunk(chunk_str)?;
            self.pending_events.extend(events);
```

`AsyncConnection` struct'ına (147-155) `utf8: crate::utf8::Utf8Carry,` alanı ekle; `connect` kurucusuna (176) `utf8: crate::utf8::Utf8Carry::new(),`; `start_stream` (227) ve `start_tls` (368) içinde `self.parser.reset();` altına `self.utf8.reset();`; `recv_event` (266) dönüşümünü değiştir:

```rust
            let chunk_str = self.utf8.feed(&buf[..n])?;
            if self.log_traffic {
                eprintln!("[XMPP ASYNC IN] {}", chunk_str);
            }
            let events = self.parser.parse_chunk(chunk_str)?;
            self.pending_events.extend(events);
```

`split` (288) içinde `utf8` alanını receiver'a taşı:

```rust
        let receiver = AsyncReceiver {
            reader,
            parser: self.parser,
            utf8: self.utf8,
            pending_events: self.pending_events,
            timeout: self.timeout,
        };
```

- [ ] **Adım 4: Testlerin geçtiğini doğrula**

Run: `cargo test --test utf8_chunk_boundaries 2>&1 | tail -20`
Expected: PASS — 4 test.

- [ ] **Adım 5: Tam paket yeşil kalsın**

Run: `cargo test --all-features 2>&1 | grep -E "test result|FAILED"`
Expected: hiç `FAILED` yok.

- [ ] **Adım 6: Commit**

```bash
git add src/net.rs src/async_net.rs tests/utf8_chunk_boundaries.rs
git commit -m "fix(transport): üç okuma yolunu artımlı UTF-8'e geçir

Senkron Connection, AsyncConnection ve split sonrası AsyncReceiver artık
parça sınırında bölünen çok baytlı karakteri doğru birleştiriyor.
start_stream ve start_tls, carry'yi sıfırlar (TLS el sıkışması akışın
ortasında başlar; eski kuyruk yeni akışa sızmamalıdır).

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 3: Kaçış çekirdeği (A5a)

**Files:**
- Create: `src/escape.rs`
- Modify: `src/lib.rs` — modül bildirimlerine `mod escape;`

**Interfaces:**
- Consumes: yok.
- Produces:
  - `pub(crate) enum EscapeOut { Literal, LiteralSlice(&'static str), Numeric([u8; 10], u8) }`
  - `pub(crate) fn escape_char(c: char) -> Option<EscapeOut>`
  - `pub(crate) fn is_literal_byte(b: u8) -> bool`
  - `pub(crate) fn escaped_len(s: &str) -> usize`
  - `pub(crate) fn write_escaped<W: std::io::Write>(w: &mut W, s: &str) -> std::io::Result<()>`
  - `pub(crate) fn escape_to_string(s: &str) -> String` — `String` yolları için (`String` `io::Write` uygulamaz)

- [ ] **Adım 1: Başarısız testleri yaz**

`src/escape.rs` sonuna:

```rust
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
```

- [ ] **Adım 2: Testin başarısız olduğunu doğrula**

Run: `cargo test --lib escape 2>&1 | tail -20`
Expected: FAIL — `error[E0433]: failed to resolve: use of undeclared crate or module 'escape'`.

- [ ] **Adım 3: Implementasyonu yaz**

`src/escape.rs`:

```rust
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
```

`src/lib.rs` modül bildirimlerine ekle:

```rust
mod escape;
```

- [ ] **Adım 4: Testlerin geçtiğini doğrula**

Run: `cargo test --lib escape 2>&1 | tail -20`
Expected: PASS — 10 test.

- [ ] **Adım 5: Commit**

```bash
git add src/escape.rs src/lib.rs
git commit -m "feat(escape): tek kaçış çekirdeği ve sayısal referans desteği

escape_char/write_escaped/escaped_len, C escape() sözleşmesini kopyalar:
beş varlık her iki bağlamda, kontrol karakterleri &#x%02x;, ASCII dışı
kod noktaları en az iki haneli hex, U+0000 düşürülür.

escaped_len ile write_escaped'in çıktı uzunluğu yapısal olarak eşit
tutulur; C'deki bütçe taşması bu eşitliğin bozulmasından doğar.

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 4: Tüm kaçış çağrı yerlerini çekirdeğe taşı (A5b)

**Files:**
- Modify: `src/utility.rs:104-137` (`escape_cow`)
- Modify: `src/lib.rs:887-925` (üç `Display` çağrısı), `src/lib.rs:950-964` (`escape_attr`/`escape_text` silinir)
- Modify: `src/writer.rs:61`, `src/writer.rs:76` (çağrılar), `src/writer.rs:132-184` (iki fonksiyon silinir)
- Modify: `src/helper.rs:61-72` (`escape_size` delege)
- Modify: `src/parser.rs:17-61` (yerel `escape_size`/`escape` silinir), `src/parser.rs:807-840` (`serialize`), `src/parser.rs:849-882` (`serialized_size`)
- Test: `tests/escape_parity.rs` (yeni)

**Interfaces:**
- Consumes: Görev 3'ten `crate::escape::{escape_to_string, escaped_len, is_literal_byte, write_escaped}`
- Produces: `utility::escape_cow` davranışı değişir (sayısal referans); `helper::escape_size` doğru uzunluğu verir. Genel API imzaları aynı kalır.

- [ ] **Adım 1: Mevcut kilidin geçtiğini doğrula**

`tests/dom_writer_traversal.rs::test_writer_and_to_string_equivalence` iki bağımsız serileştirme yolunu (`Display` ve `XmlWriter`) karşılaştırır; A5b'nin ana kilidi budur.

Run: `cargo test --test dom_writer_traversal test_writer_and_to_string_equivalence 2>&1 | tail -10`
Expected: PASS (değişiklik öncesi).

- [ ] **Adım 2: Yeni davranışı kilitleyen testleri yaz**

`tests/escape_parity.rs`:

```rust
//! C oracle ile kaçış paritesi.

use iksemel::{escape, escape_cow, escape_size, DomParser, IksNode, XmlWriter};

/// `café ü` → `caf&#xe9; &#xfc;` (C oracle çıktısı).
#[test]
fn public_escape_matches_c_oracle() {
    assert_eq!(escape("café ü"), "caf&#xe9; &#xfc;");
    assert_eq!(escape("&<>'\""), "&amp;&lt;&gt;&apos;&quot;");
    assert_eq!(escape("\u{01}"), "&#x01;");
    assert_eq!(escape("\u{7f}"), "&#x7f;");
    assert_eq!(escape("\u{a0}"), "&#xa0;");
    assert_eq!(escape("€"), "&#x20ac;");
    assert_eq!(escape("a\0b"), "ab");
}

/// Saf ASCII ve kaçılacak karakter yoksa `Cow::Borrowed` korunur
/// (utility.rs'in sıfır-ayırma iddiası).
#[test]
fn fast_path_still_borrows() {
    assert!(matches!(
        escape_cow("plain ascii text"),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(matches!(
        escape_cow("with\ttabs and\nnewlines"),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(matches!(escape_cow("café"), std::borrow::Cow::Owned(_)));
    assert!(matches!(escape_cow("a&b"), std::borrow::Cow::Owned(_)));
    assert!(matches!(escape_cow("\u{7f}"), std::borrow::Cow::Owned(_)));
}

/// `escape_size` artık gerçek çıktı uzunluğunu verir.
#[test]
fn public_escape_size_matches_output() {
    for s in ["", "plain", "café ü", "€ 😀", "&<>'\"", "\u{01}\u{7f}", "a\0b"] {
        assert_eq!(escape_size(s), escape(s).len(), "girdi: {:?}", s);
    }
}

/// İki serileştirme yolu birebir aynı baytları üretir — ASCII dışı ve
/// tırnak içeren içerikle de.
#[test]
fn both_serializers_agree_on_tricky_content() {
    let xml = "<r a=\"café &amp; 'x'\"><b>Say \"hi\" — € 😀</b></r>";
    let node = DomParser::parse_str(xml).expect("parse");

    let via_display = node.borrow().to_string();

    let mut buf = Vec::new();
    node.borrow().write_to(&mut buf).expect("write_to");
    let via_writer = String::from_utf8(buf).expect("utf8");

    assert_eq!(via_display, via_writer);
}

/// Metin bağlamında da tırnak kaçılır; C tek fonksiyon kullandığı için böyle
/// davranır.
#[test]
fn quotes_escape_in_text_context_too() {
    let mut root = IksNode::new_tag("r");
    root.set_content("a\"b'c");
    assert_eq!(root.to_string(), "<r>a&quot;b&apos;c</r>");
}

/// `XmlWriter` hem yoğun hem pretty modda sayısal referans kullanır.
#[test]
fn xml_writer_escapes_numerically_in_both_modes() {
    let mut root = IksNode::new_tag("r");
    let mut child = IksNode::new_tag("b");
    child.set_content("café");
    root.add_child(child);

    let mut buf = Vec::new();
    let mut w = XmlWriter::new(&mut buf);
    w.write_node(&root).unwrap();
    assert_eq!(String::from_utf8(buf).unwrap(), "<r><b>caf&#xe9;</b></r>");

    let mut root2 = IksNode::new_tag("r");
    let mut child2 = IksNode::new_tag("b");
    child2.set_content("café");
    root2.add_child(child2);
    let mut buf2 = Vec::new();
    let mut w2 = XmlWriter::new(&mut buf2);
    w2.set_pretty(true, 2);
    w2.write_node(&root2).unwrap();
    let pretty = String::from_utf8(buf2).unwrap();
    assert!(pretty.contains("caf&#xe9;"), "pretty çıktı: {:?}", pretty);
}
```

- [ ] **Adım 3: Testlerin başarısız olduğunu doğrula**

Run: `cargo test --test escape_parity 2>&1 | tail -30`
Expected: FAIL — `public_escape_matches_c_oracle`, `quotes_escape_in_text_context_too`, `xml_writer_escapes_numerically_in_both_modes`.

- [ ] **Adım 4: Çağrı yerlerini çekirdeğe taşı**

`src/utility.rs` — `escape_cow` gövdesini değiştir (104-137):

```rust
/// Escapes special XML characters in a string, returning a borrowed `Cow` if no escaping is needed.
pub fn escape_cow(s: &str) -> Cow<'_, str> {
    if s.bytes().all(crate::escape::is_literal_byte) {
        return Cow::Borrowed(s);
    }
    Cow::Owned(crate::escape::escape_to_string(s))
}
```

`src/lib.rs` — `Display` gövdesindeki üç çağrıyı değiştir (887-925):

```rust
                for (name, value) in &self.attributes {
                    write!(f, " {}=\"{}\"", name, crate::utility::escape_cow(value))?;
                }
```

```rust
                    if let Some(content) = &self.content {
                        write!(f, "{}", crate::utility::escape_cow(content))?;
                    }
```

```rust
            IksType::CData => {
                if let Some(content) = &self.content {
                    write!(f, "{}", crate::utility::escape_cow(content))?;
                }
            }
```

Sonra `src/lib.rs:950-964`'teki `escape_attr` ve `escape_text` fonksiyonlarını **tamamen sil**.

`src/writer.rs` — iki çağrıyı değiştir (61 ve 76):

```rust
                for (attr_name, attr_val) in node.attributes() {
                    self.writer.write_all(b" ")?;
                    self.writer.write_all(attr_name.as_bytes())?;
                    self.writer.write_all(b"=\"")?;
                    crate::escape::write_escaped(&mut self.writer, attr_val)?;
                    self.writer.write_all(b"\"")?;
                }
```

```rust
                if let Some(text) = content {
                    crate::escape::write_escaped(&mut self.writer, text)?;
                }
```

Sonra `write_escaped_attr` (132-158) ve `write_escaped_text` (160-184) fonksiyonlarını **tamamen sil**.

`src/helper.rs` — `escape_size` gövdesini değiştir (61-72):

```rust
pub fn escape_size(s: &str) -> usize {
    crate::escape::escaped_len(s)
}
```

**Dokunma:** `helper.rs::unescape_size`. Bu görev yalnızca kaçış yönünü birleştirir; ters yön (unescape) B/C alt projelerinin konusudur.

`src/parser.rs` — dosya başındaki `use crate::{...};` satırına ekle:

```rust
use crate::escape::{escape_to_string, escaped_len};
```

Sonra yerel `escape_size` ve `escape` fonksiyonlarını (17-61 aralığı) **tamamen sil**.

`serialize` (807-840) içindeki dört `&escape(...)` çağrısını değiştir:

```rust
        if !self.buffer.is_empty() {
            result.push_str(&escape_to_string(&self.buffer));
        }
        if !self.tag_name.is_empty() {
            result.push('<');
            if self.tag_type == TagType::Close {
                result.push('/');
            }
            result.push_str(&escape_to_string(&self.tag_name));
            for (name, value) in &self.attributes {
                result.push(' ');
                result.push_str(&escape_to_string(name));
                result.push('=');
                result.push('"');
                result.push_str(&escape_to_string(value));
                result.push('"');
            }
            if self.tag_type == TagType::Single {
                result.push('/');
            }
            result.push('>');
        }
```

`serialized_size` (849-882) içindeki dört `escape_size(...)` çağrısını `escaped_len(...)` yap:

```rust
        if !self.buffer.is_empty() {
            size += escaped_len(&self.buffer);
        }
        if !self.tag_name.is_empty() {
            size += 1; // <
            if self.tag_type == TagType::Close {
                size += 1; // /
            }
            size += escaped_len(&self.tag_name);
            for (name, value) in &self.attributes {
                size += 1; // boşluk
                size += escaped_len(name);
                size += 1; // =
                size += 1; // "
                size += escaped_len(value);
                size += 1; // "
            }
            if self.tag_type == TagType::Single {
                size += 1; // /
            }
            size += 1; // >
        }
```

**Not:** `serialized_size`, yerel `escape_size`'ın **ikinci** tüketicisidir; birincisi `serialize`. İkisini de taşımadan yerel fonksiyon silinemez.

- [ ] **Adım 5: Testlerin geçtiğini doğrula**

Run: `cargo test --test escape_parity 2>&1 | tail -20`
Expected: PASS — 6 test.

- [ ] **Adım 6: Eski fonksiyonların kalmadığını doğrula**

Run:
```bash
grep -rn "escape_attr\|escape_text" src/ || echo "temiz: escape_attr/escape_text kalmadı"
grep -rn "fn escape(" src/ || echo "temiz: yerel escape kalmadı"
grep -rn "fn escape_size" src/ || echo "yok"
```
Expected: ilk iki komut `temiz` çıktısı verir. Üçüncüsü `src/helper.rs`'teki `pub fn escape_size`'ı bulur — bu tek meşru eşleşmedir (artık `escape::escaped_len`'e delege eder).

- [ ] **Adım 7: Tam paket**

Run: `cargo test --all-features 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: hiç `FAILED` yok. Bir iddia düşerse: metinde tırnak/ASCII-dışı kaçışı bekleyen **sabit bir beklenen dize** ise, değeri C oracle çıktısıyla doğrulayıp güncelle.

- [ ] **Adım 8: Commit**

```bash
git add src/lib.rs src/utility.rs src/writer.rs src/helper.rs src/parser.rs tests/escape_parity.rs
git commit -m "refactor(escape): altı kaçış fonksiyonunu tek çekirdeğe indir

escape_attr, escape_text, write_escaped_attr, write_escaped_text ve
parser.rs'in yerel escape/escape_size kopyaları silindi; hepsi
escape::write_escaped, escape::escape_to_string ve escape::escaped_len'e
delege ediyor.

Davranış C'ye yaklaştı: metin bağlamında da ' ve \" kaçılır, kontrol
karakterleri ile ASCII dışı kod noktaları sayısal referansa dönüşür.
Genel API imzaları değişmedi.

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 5: `0x00` baytını reddet (A4a)

**Files:**
- Modify: `src/parser.rs:409-411` (`parse` girişi)
- Test: `src/parser.rs` test modülü

**Interfaces:**
- Consumes: yok.
- Produces: değişen imza yok; `parse` artık `\0` içeren girdide `Err(IksError::BadXml)` döner.

- [ ] **Adım 1: Başarısız testleri yaz**

`src/parser.rs` içindeki mevcut `#[cfg(test)] mod tests` bloğuna ekle:

```rust
    /// Testler için hiçbir şey yapmayan handler.
    struct NullHandler;

    impl SaxHandler for NullHandler {
        fn on_tag(
            &mut self,
            _name: &str,
            _attributes: &[(String, String)],
            _tag_type: TagType,
        ) -> Result<()> {
            Ok(())
        }
        fn on_cdata(&mut self, _data: &str) -> Result<()> {
            Ok(())
        }
    }

    /// C `sax_core` bayt döngüsünün en başında 0x00'ı reddeder (sax.c:209),
    /// bağlamdan bağımsız olarak.
    #[test]
    fn nul_byte_is_rejected_in_every_context() {
        let cases = [
            "<r>a\0b</r>",      // metin (CData hızlı yolu)
            "<r x=\"a\0b\"/>",  // attribute değeri
            "<r x\0y=\"1\"/>",  // attribute adı
            "<r\0>text</r>",    // tag adı (TagStart dalı)
            "<![CDATA[a\0b]]>", // kesit CDATA (SectCDataC hızlı yolu)
            "<!-- a\0b -->",    // yorum (CommentBody hızlı yolu)
            "<?pi a\0b?>",      // işlem talimatı
        ];
        for xml in cases {
            let mut parser = Parser::new(NullHandler);
            assert!(
                matches!(parser.parse(xml), Err(IksError::BadXml)),
                "reddedilmeliydi: {:?}",
                xml
            );
        }
    }

    /// Kontrol: NUL içermeyen benzer belgeler hata vermez.
    #[test]
    fn documents_without_nul_still_parse() {
        for xml in ["<r>ab</r>", "<r x=\"ab\"/>", "<![CDATA[ab]]>", "<!-- ab -->"] {
            let mut parser = Parser::new(NullHandler);
            assert!(parser.parse(xml).is_ok(), "kabul edilmeliydi: {:?}", xml);
        }
    }
```

- [ ] **Adım 2: Testin başarısız olduğunu doğrula**

Run: `cargo test --lib parser::tests::nul_byte_is_rejected_in_every_context 2>&1 | tail -20`
Expected: FAIL — en az `<r>a\0b</r>`, `<![CDATA[a\0b]]>` ve `<!-- a\0b -->` için `parse` `Ok(())` döner.

- [ ] **Adım 3: Girişte tek tarama ekle**

`src/parser.rs`, `parse` gövdesinin en başı:

```rust
    pub fn parse(&mut self, data: &str) -> Result<()> {
        // C `sax_core` bayt döngüsünün en başında 0x00'ı reddeder ve bu
        // kontrol bağlamdan bağımsızdır (sax.c:209). Hızlı bayt tarama
        // yolları (CData, CommentBody, SectCDataC) ayırıcılarına kadar ham
        // baytları geçtiği için döngü tepesindeki bir kontrol NUL'u göremez;
        // bu yüzden tarama burada, durum makinesinden önce yapılır. Üç
        // döngüyü tek tek yamamak, dördüncü bir döngünün deliği sessizce
        // yeniden açmasına yol açardı.
        if data.as_bytes().contains(&0) {
            return Err(IksError::BadXml);
        }

        let bytes = data.as_bytes();
        let mut i = 0;
        // ... mevcut gövde değişmeden devam eder
```

- [ ] **Adım 4: Testlerin geçtiğini doğrula**

Run: `cargo test --lib parser 2>&1 | tail -20`
Expected: PASS — yeni 2 test dahil.

- [ ] **Adım 5: Commit**

```bash
git add src/parser.rs
git commit -m "fix(parser): 0x00 baytını tüm bağlamlarda reddet

C sax_core bu kontrolü bayt döngüsünün en başında, bağlamdan bağımsız
yapar. Rust'ın CData/CommentBody/SectCDataC hızlı taramaları ayırıcılarına
kadar ham baytları geçtiği için kontrol girişte tek yerde yapılır.

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 6: `max_token_size`'ı tüm biriktirme yollarına uygula (A4b)

**Files:**
- Modify: `src/parser.rs` — yedi biriktirme noktası: 427-429 (bulunan metin), 478-480 (kesit CDATA), 512-516 (`TagStart` tag adı), 588-608 (`SectCDataE`/`SectCDataE2`), 634 (`Tag` tag adı), 645-648 (`Attribute` attr adı), 659 (`AttributeName` attr adı)
- Test: `src/parser.rs` test modülü

**Interfaces:**
- Consumes: `ParserLimits::max_token_size`, `IksError::MaxTokenSizeExceeded` (ikisi de mevcut)
- Produces: yeni hata tipi yok.

**Sınır semantiği:** Mevcut iki kontrol (683, 705) `push`'tan **önce** `>=` test eder → "en fazla `max_token_size` bayt". Yeni kontroller `push`'tan **sonra** `>` test eder ve aynı sınıra denk gelir: limit 4 iken 4 bayt kabul, 5 bayt ret. Tutarlılık için bu biçim kullanılır.

- [ ] **Adım 1: Başarısız testleri yaz**

`src/parser.rs` test modülüne ekle:

```rust
    fn limits_with_token_size(n: usize) -> ParserLimits {
        ParserLimits {
            max_token_size: n,
            ..ParserLimits::default()
        }
    }

    /// `max_token_size`, belgelenmiş sözleşmesi gereği *her* biriktirme
    /// yolunu kapsar.
    #[test]
    fn max_token_size_covers_every_accumulation_path() {
        let cases = [
            "<r>aaaaaaaaaaaa</r>",             // bulunan metin
            "<r><![CDATA[aaaaaaaaaaaa]]></r>", // kesit CDATA
            "<aaaaaaaaaaaa>",                  // tag adı (TagStart)
            "<r aaaaaaaaaaaa=\"1\"/>",         // attribute adı (Attribute)
            "<r aaaaaaaaaaaa>",                // attribute adı (AttributeName)
            "<r x=\"aaaaaaaaaaaa\"/>",         // attribute değeri (zaten vardı)
            "<r><![CDATA[aaaaaaaa]]]]></r>",   // SectCDataE2 geri alma yolu
        ];
        for xml in cases {
            let mut parser = Parser::with_limits(NullHandler, limits_with_token_size(4));
            assert!(
                matches!(parser.parse(xml), Err(IksError::MaxTokenSizeExceeded)),
                "sınırı aşmalıydı: {:?}",
                xml
            );
        }
    }

    /// Sınır davranışı: 4 bayt kabul, 5 bayt ret.
    #[test]
    fn max_token_size_boundary_is_exact() {
        let mut ok = Parser::with_limits(NullHandler, limits_with_token_size(4));
        assert!(ok.parse("<r>abcd</r>").is_ok());

        let mut too_long = Parser::with_limits(NullHandler, limits_with_token_size(4));
        assert!(matches!(
            too_long.parse("<r>abcde</r>"),
            Err(IksError::MaxTokenSizeExceeded)
        ));
    }

    /// Varsayılan limitlerle büyük ama meşru bir belge hata vermemeli.
    #[test]
    fn default_limits_do_not_reject_large_legitimate_tokens() {
        let big = "a".repeat(1024 * 1024);

        let mut text = Parser::new(NullHandler);
        assert!(text.parse(&format!("<r>{}</r>", big)).is_ok());

        let mut cdata = Parser::new(NullHandler);
        assert!(cdata.parse(&format!("<r><![CDATA[{}]]></r>", big)).is_ok());
    }
```

- [ ] **Adım 2: Testin başarısız olduğunu doğrula**

Run: `cargo test --lib parser::tests::max_token_size_covers_every_accumulation_path 2>&1 | tail -20`
Expected: FAIL — metin, kesit CDATA, tag adı ve attribute adı vakaları `Ok` döner.

- [ ] **Adım 3: Yedi noktaya kontrol ekle**

**(a) `State::CData` hızlı yolu (427-429):**

```rust
                if i > start {
                    self.buffer.push_str(&data[start..i]);
                    if self.buffer.len() > self.limits.max_token_size {
                        return Err(IksError::MaxTokenSizeExceeded);
                    }
                }
```

**(b) `State::SectCDataC` hızlı yolu (478-480):**

```rust
                if i > start {
                    self.buffer.push_str(&data[start..i]);
                    if self.buffer.len() > self.limits.max_token_size {
                        return Err(IksError::MaxTokenSizeExceeded);
                    }
                }
```

**(c) `State::TagStart` etiket adı (512-516):**

```rust
                    _ => {
                        self.tag_type = TagType::Open;
                        self.tag_name.push(c);
                        if self.tag_name.len() > self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                        self.state = State::Tag;
                    }
```

**(d) `State::Tag` etiket adı (634):**

```rust
                    _ => {
                        self.tag_name.push(c);
                        if self.tag_name.len() > self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                    }
```

**(e) `State::Attribute` dalı (645-648):**

```rust
                    _ => {
                        self.attr_name.push(c);
                        if self.attr_name.len() > self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                        self.state = State::AttributeName;
                    }
```

**(f) `State::AttributeName` dalı (659):**

```rust
                    _ => {
                        self.attr_name.push(c);
                        if self.attr_name.len() > self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                    }
```

**(g) `State::SectCDataE` ve `SectCDataE2` (588-608)** — bu dallar `]` geri alırken tamponu büyütür. Her iki dalın sonuna tek kontrol ekle:

```rust
               State::SectCDataE => {
                   if c == ']' {
                       self.state = State::SectCDataE2;
                   } else {
                       self.buffer.push(']');
                       self.buffer.push(c);
                       self.state = State::SectCDataC;
                   }
                   if self.buffer.len() > self.limits.max_token_size {
                       return Err(IksError::MaxTokenSizeExceeded);
                   }
               }
               State::SectCDataE2 => {
                   if c == '>' {
                       self.state = State::CData;
                   } else if c == ']' {
                       self.buffer.push(']');
                   } else {
                       self.buffer.push(']');
                       self.buffer.push(']');
                       self.buffer.push(c);
                       self.state = State::SectCDataC;
                   }
                   if self.buffer.len() > self.limits.max_token_size {
                       return Err(IksError::MaxTokenSizeExceeded);
                   }
               }
```

**Yerel `escape_size`/`escape` ile etkileşim:** Görev 4, `src/parser.rs:17-61`'deki bu iki fonksiyonu siler. Görev 6 onlara dokunmaz. İki görev farklı satır aralıklarında çalışır; sıra önemli değildir.

- [ ] **Adım 4: Testlerin geçtiğini doğrula**

Run: `cargo test --lib parser 2>&1 | tail -20`
Expected: PASS.

- [ ] **Adım 5: Mevcut DoS testlerinin yeşil kaldığını doğrula**

Run: `cargo test --test security_dos_limits --test parser_stress 2>&1 | tail -20`
Expected: PASS.

- [ ] **Adım 6: Commit**

```bash
git add src/parser.rs
git commit -m "fix(parser): max_token_size'ı yedi denetimsiz biriktirme yoluna uygula

Bulunan metin, kesit CDATA (üç yol), tag adı (iki yol) ve attribute adı
(iki yol) tamponları sınırsız büyüyebiliyordu; ParserLimits dokümantasyonu
bu sınırın hepsini kapsadığını vaat ediyordu.

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 7: `append_text` yardımcısı ve metin doğruluğu (A3a + A3b)

**Files:**
- Modify: `src/lib.rs` — `find_cdata`'nın yanına crate-içi `append_text` eklenir
- Modify: `src/dom.rs:222-231` (`on_cdata`), `src/dom.rs:238-303` (iki birim testi güncellenir)
- Modify: `src/stream.rs:161-182` (`on_cdata`)
- Test: `src/dom.rs` test modülü, `tests/text_fidelity.rs` (yeni)

**Interfaces:**
- Consumes: `IksNode::new_cdata`, `IksNode::add_child`, `IksType::CData`
- Produces: `pub(crate) fn append_text(parent: &Rc<RefCell<IksNode>>, data: &str)`

- [ ] **Adım 1: Başarısız testleri yaz**

`tests/text_fidelity.rs` (yeni). **Dikkat:** entegrasyon testleri crate kökündeki private alanlara erişemez; erişimciler kullanılır.

```rust
//! Boşluk korunması ve ekleme anında birleştirme (C `dom.c` / `iks_insert_cdata`).

use iksemel::{DomParser, StreamEvent, StreamParser};

/// C `cdataHook` koşulsuz `iks_insert_cdata` çağırır; boşluk kontrolü yoktur.
#[test]
fn whitespace_only_text_is_kept_as_a_node() {
    let dom = DomParser::parse_str("<r> </r>").expect("parse");
    let root = dom.borrow();
    assert_eq!(root.children().len(), 1);
    assert_eq!(root.children()[0].borrow().content(), Some(" "));
}

/// Biçimlendirilmiş XML'de C boşluk metnini düğüm olarak saklar.
/// Oracle: `<r>\n  <a/>\n  <b/>\n</r>` → 5 çocuk
/// CDATA("\n  "), TAG a, CDATA("\n  "), TAG b, CDATA("\n").
#[test]
fn pretty_printed_input_keeps_five_children() {
    let dom = DomParser::parse_str("<r>\n  <a/>\n  <b/>\n</r>").expect("parse");
    let root = dom.borrow();
    let kids = root.children();
    assert_eq!(kids.len(), 5);

    assert_eq!(kids[0].borrow().content(), Some("\n  "));
    assert_eq!(kids[1].borrow().name(), Some("a"));
    assert_eq!(kids[2].borrow().content(), Some("\n  "));
    assert_eq!(kids[3].borrow().name(), Some("b"));
    assert_eq!(kids[4].borrow().content(), Some("\n"));
}

/// Araya tag girdiği için birleşmez: C'de üç ayrı düğüm.
#[test]
fn text_separated_by_a_tag_does_not_merge() {
    let dom = DomParser::parse_str("<r>ab<q/>cd</r>").expect("parse");
    let root = dom.borrow();
    let kids = root.children();
    assert_eq!(kids.len(), 3);
    assert_eq!(kids[0].borrow().content(), Some("ab"));
    assert_eq!(kids[1].borrow().name(), Some("q"));
    assert_eq!(kids[2].borrow().content(), Some("cd"));
}

/// Aynı metin iki ayrı `parse_chunk` çağrısıyla gelirse C tek düğüm tutar:
/// `iks_insert_cdata` son çocuk CDATA ise ona ekler.
#[test]
fn text_across_chunks_merges_into_one_node() {
    let mut parser = StreamParser::new();
    parser
        .parse_chunk("<stream:stream xmlns='jabber:client'>")
        .unwrap();
    parser.parse_chunk("<message><body>ab").unwrap();
    let events = parser.parse_chunk("cd</body></message>").unwrap();

    let stanza = events
        .into_iter()
        .find_map(|e| match e {
            StreamEvent::Stanza(s) => Some(s),
            _ => None,
        })
        .expect("stanza");

    let body = stanza.find("body").expect("body");
    let body = body.borrow();
    assert_eq!(body.children().len(), 1, "tek CDATA düğümü olmalı");
    assert_eq!(body.children()[0].borrow().content(), Some("abcd"));
}
```

`src/dom.rs` test modülündeki iki testi **değiştir** — bunlar eski davranışı kodluyor:

```rust
    #[test]
    fn test_dom_child() {
        let xml = r#"
            <root>
                <child id="3"/>
            </root>"#;

        let dom = DomParser::parse_str(xml).unwrap();
        let root = dom.borrow();

        assert_eq!(root.name.as_ref().unwrap(), "root");
        // C semantiği: kökün içindeki boşluk metni düğüm olarak korunur.
        // Oracle: 3 çocuk — CDATA("\n                "), TAG child,
        // CDATA("\n            ").
        assert_eq!(root.children.len(), 3);
        assert_eq!(
            root.children[0].borrow().content.as_deref(),
            Some("\n                ")
        );

        let child = root.children[1].borrow();
        assert_eq!(child.name.as_ref().unwrap(), "child");
        assert_eq!(child.attributes[0], ("id".to_string(), "3".to_string()));
        assert!(child.children.is_empty());

        assert_eq!(
            root.children[2].borrow().content.as_deref(),
            Some("\n            ")
        );
    }

    #[test]
    fn test_dom_parsing() {
        let xml = r#"
            <root version="1.0">
                <child id="1">Text1</child>
                <child id="2">Text2</child>
                <child id="3"/>
            </root>"#;

        let dom = DomParser::parse_str(xml).unwrap();
        let root = dom.borrow();

        assert_eq!(root.name.as_ref().unwrap(), "root");
        assert_eq!(
            root.attributes[0],
            ("version".to_string(), "1.0".to_string())
        );
        // C semantiği: 3 element + 4 boşluk CDATA düğümü = 7 çocuk.
        assert_eq!(root.children.len(), 7);

        let child1 = root.children[1].borrow();
        assert_eq!(child1.name.as_ref().unwrap(), "child");
        assert_eq!(child1.attributes[0], ("id".to_string(), "1".to_string()));
        assert_eq!(
            child1.children.first().unwrap().borrow().content.as_deref(),
            Some("Text1")
        );

        let child2 = root.children[3].borrow();
        assert_eq!(child2.name.as_ref().unwrap(), "child");
        assert_eq!(child2.attributes[0], ("id".to_string(), "2".to_string()));
        assert_eq!(
            child2.children.first().unwrap().borrow().content.as_deref(),
            Some("Text2")
        );

        let child3 = root.children[5].borrow();
        assert_eq!(child3.name.as_ref().unwrap(), "child");
        assert_eq!(child3.attributes[0], ("id".to_string(), "3".to_string()));
        assert!(child3.children.is_empty());
    }
```

- [ ] **Adım 2: Testlerin başarısız olduğunu doğrula**

Run:
```bash
cargo test --test text_fidelity 2>&1 | tail -30
cargo test --lib dom::tests 2>&1 | tail -20
```
Expected: FAIL — `whitespace_only_text_is_kept_as_a_node` (şu an 0 çocuk), `pretty_printed_input_keeps_five_children` (şu an 2 çocuk), ve `dom::tests`'in iki testi.

- [ ] **Adım 3: `append_text` yardımcısını ekle**

`src/lib.rs`, `find_cdata` fonksiyonunun hemen ardına:

```rust
/// `parent`'a metin ekler; son çocuk CDATA ise **ona ekler**.
///
/// C `iks_insert_cdata`'nın kuralı: son çocuk CDATA ise yeni düğüm açılmaz.
/// Hem DOM hem akış handler'ı bu tek kuralı paylaşır.
pub(crate) fn append_text(parent: &Rc<RefCell<IksNode>>, data: &str) {
    let merge_target = {
        let p = parent.borrow();
        match p.children.last() {
            Some(last) if last.borrow().node_type == IksType::CData => Some(Rc::clone(last)),
            _ => None,
        }
    };

    match merge_target {
        Some(last) => {
            let mut last_ref = last.borrow_mut();
            match last_ref.content.as_mut() {
                Some(content) => content.push_str(data),
                None => last_ref.content = Some(data.to_string()),
            }
        }
        None => {
            let cdata = IksNode::new_cdata(data);
            parent.borrow_mut().add_child(cdata);
        }
    }
}
```

Ödünçleme notu: `parent` ve `last` ayrı `RefCell`'lerdir, dolayısıyla `last.borrow_mut()` ile `parent.borrow()` çakışmaz. `Rc::clone(last)` iç ödünçlerin ikisini de bloğun sonunda bırakır.

- [ ] **Adım 4: İki handler'ı bu yardımcıya geçir**

`src/dom.rs`, `on_cdata` (222-231) gövdesini değiştir:

```rust
    fn on_cdata(&mut self, data: &str) -> Result<()> {
        // C `cdataHook` koşulsuz `iks_insert_cdata` çağırır: boşluk kontrolü
        // yoktur ve son çocuk CDATA ise ona eklenir.
        if let Some(parent) = self.node_stack.last() {
            crate::append_text(parent, data);
        }
        Ok(())
    }
```

`src/stream.rs`, `on_cdata` (161-182) gövdesini değiştir:

```rust
    fn on_cdata(&mut self, data: &str) -> Result<()> {
        if self.depth > 1 {
            if let Some(parent) = self.stanza_stack.last() {
                // `depth > 1` kapısı akış ayrıştırıcısına özgüdür (kök akış
                // elementi ve üst düzey metin stanza'ya girmez). Geri kalan
                // kural DOM ile aynıdır: son çocuk CDATA ise ona ekle.
                crate::append_text(parent, data);
            }
        }
        Ok(())
    }
```

- [ ] **Adım 5: Testlerin geçtiğini doğrula**

Run:
```bash
cargo test --test text_fidelity 2>&1 | tail -20
cargo test --lib dom::tests 2>&1 | tail -20
```
Expected: PASS — `text_fidelity` 4 test, `dom::tests` mevcut 5 test.

- [ ] **Adım 6: Tüm DOM/stream testlerinin yeşil kaldığını doğrula**

Bu adım **kritiktir**: boşluk korunması çocuk sayılarını değiştirir. `find`, `find_all`, `child_tags`, `select` ve `match_parsed_segment` hepsi `IksType::Tag` süzgeci uygular, dolayısıyla etkilenmemeleri beklenir.

Run: `cargo test --all-features 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: hiç `FAILED` yok. Bir iddia düşerse:
- İddia **konumsal çocuk indekslemesi** yapıyorsa (`children[n]`) → C oracle çıktısına göre güncelle.
- İddia `child_tags()`/`find()`/`select()` kullanıyorsa → düşmemeli; düştüyse `append_text` uygulamasında hata vardır, iddiayı değil kodu düzelt.

- [ ] **Adım 7: Commit**

```bash
git add src/lib.rs src/dom.rs src/stream.rs tests/text_fidelity.rs
git commit -m "fix(dom): boşluk metnini koru ve ekleme anında birleştir

C cdataHook boşluk kontrolü yapmaz ve iks_insert_cdata son çocuk CDATA
ise ona ekler. Rust boşluk-only metni düşürüyor ve her parça için yeni
düğüm açıyordu.

Ortak kural crate-içi append_text'e taşındı; DOM ve akış handler'ı aynı
davranışı paylaşıyor. dom.rs'in iki birim testi C semantiğine göre
güncellendi (3 ve 7 çocuk).

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 8: `find_cdata` ilk-çocuk kuralı (A3c)

**Files:**
- Modify: `src/lib.rs:385-393` (`find_cdata`)
- Test: `tests/text_fidelity.rs` (mevcut dosyaya eklenir)

**Interfaces:**
- Consumes: `IksNode::find`
- Produces: `find_cdata` imzası değişmez; davranışı C'ye uyar.

- [ ] **Adım 1: Başarısız testleri yaz**

`tests/text_fidelity.rs` sonuna ekle:

```rust
/// C `iks_find_cdata` (iks.c:450-459): bulunan düğümün **ilk çocuğu** CDATA
/// değilse NULL döner.
#[test]
fn find_cdata_requires_the_first_child_to_be_cdata() {
    // İlk çocuk doğrudan CDATA → döner.
    let dom = DomParser::parse_str("<r><a>ab</a></r>").expect("parse");
    assert_eq!(dom.borrow().find_cdata("a"), Some("ab".to_string()));

    // İlk çocuk bir TAG → ileride CDATA olsa bile None.
    let dom = DomParser::parse_str("<r><a><b/>sonra</a></r>").expect("parse");
    assert_eq!(dom.borrow().find_cdata("a"), None);

    // Çocuksuz element → None.
    let dom = DomParser::parse_str("<r><a/></r>").expect("parse");
    assert_eq!(dom.borrow().find_cdata("a"), None);
}

/// Aynı addan iki kardeş: ilki CDATA taşır, ikincisi taşımaz.
#[test]
fn find_cdata_picks_the_first_matching_element_not_the_first_cdata() {
    let dom = DomParser::parse_str("<r><a>ab</a><a/>x</r>").expect("parse");
    assert_eq!(dom.borrow().find_cdata("a"), Some("ab".to_string()));
}

/// Pretty-printed girdide ilk çocuk boşluk CDATA'sıdır ve C onu **döndürür**
/// — boşluklarıyla birlikte. Çağıranların bunu kırpılmış varsaymaması gerekir.
#[test]
fn find_cdata_returns_whitespace_cdata_verbatim() {
    let dom = DomParser::parse_str("<r><a>\n  Hello\n</a></r>").expect("parse");
    assert_eq!(
        dom.borrow().find_cdata("a"),
        Some("\n  Hello\n".to_string())
    );
}
```

- [ ] **Adım 2: Testin başarısız olduğunu doğrula**

Run: `cargo test --test text_fidelity find_cdata 2>&1 | tail -20`
Expected: FAIL — `find_cdata_requires_the_first_child_to_be_cdata` ikinci vakada `Some("sonra")` döner.

- [ ] **Adım 3: Uygulamayı değiştir**

`src/lib.rs:385-393`:

```rust
    /// Finds the first child's CDATA content with the specified tag name.
    ///
    /// C `iks_find_cdata` (`iks.c:450-459`) bulunan düğümün **ilk çocuğunu**
    /// inceler; ilk çocuk CDATA değilse `None` döner.
    pub fn find_cdata(&self, name: &str) -> Option<String> {
        let node = self.find(name)?;
        let node = node.borrow();
        let first = node.children.first()?;
        let first = first.borrow();
        if first.node_type != IksType::CData {
            return None;
        }
        first.content.clone()
    }
```

- [ ] **Adım 4: Testlerin geçtiğini doğrula**

Run: `cargo test --test text_fidelity 2>&1 | tail -20`
Expected: PASS — 7 test.

- [ ] **Adım 5: Tam paket**

Run: `cargo test --all-features 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: hiç `FAILED` yok. XEP/SASL testleri `find_cdata` kullanıyorsa düşebilirler: ilk çocuk gerçekten TAG ise bu **istenen** değişikliktir; iddia C oracle çıktısına göre güncellenir.

- [ ] **Adım 6: Commit**

```bash
git add src/lib.rs tests/text_fidelity.rs
git commit -m "fix(dom): find_cdata'yı C'nin ilk-çocuk kuralına uydur

C iks_find_cdata bulunan düğümün ilk çocuğuna bakar; ilk çocuk CDATA
değilse NULL döner. Rust çocuklar arasında ilk CDATA'yı arıyordu, bu da
karışık içerikte C'den farklı sonuç veriyordu.

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 9: `writer.rs` pretty modda CDATA kırpmasını kaldır (A3d)

**Files:**
- Modify: `src/writer.rs:101-111` (`IksType::CData` kolu)
- Test: `src/writer.rs` test modülü

**Interfaces:**
- Consumes: `crate::escape::write_escaped` (Görev 4'ten)
- Produces: değişen imza yok.

- [ ] **Adım 1: Başarısız testleri yaz**

`src/writer.rs` test modülüne ekle (mevcut `use` satırlarına `use crate::dom::DomParser;` eklemek gerekebilir):

```rust
    /// Pretty mod düğümler *arasına* boşluk ekleyebilir ama bir düğümün kendi
    /// içeriğini asla değiştiremez. `" ab "` kırpılmamalıdır.
    #[test]
    fn pretty_mode_does_not_trim_cdata_content() {
        let mut root = IksNode::new_tag("r");
        let mut cdata = IksNode::new(IksType::CData);
        cdata.set_content(" ab ");
        root.add_child(cdata);

        let mut buf = Vec::new();
        let mut writer = XmlWriter::new(&mut buf);
        writer.set_pretty(true, 2);
        writer.write_node(&root).unwrap();

        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains(" ab "), "içerik kırpılmamalı: {:?}", out);
    }

    /// Sırf boşluk olan CDATA düğümleri pretty modda atlanır — böylece
    /// `parse → pretty → parse` döngüsü her turda boşluk biriktirmez. Yoğun
    /// modda hiçbir şey atlanmaz.
    #[test]
    fn pretty_mode_skips_whitespace_only_cdata_but_compact_does_not() {
        let mut root = IksNode::new_tag("r");
        let mut ws = IksNode::new(IksType::CData);
        ws.set_content("\n  ");
        root.add_child(ws);

        let mut compact = Vec::new();
        let mut w1 = XmlWriter::new(&mut compact);
        w1.write_node(&root).unwrap();
        assert_eq!(String::from_utf8(compact).unwrap(), "<r>\n  </r>");

        let mut pretty = Vec::new();
        let mut w2 = XmlWriter::new(&mut pretty);
        w2.set_pretty(true, 2);
        w2.write_node(&root).unwrap();
        let pretty_out = String::from_utf8(pretty).unwrap();
        assert!(
            !pretty_out.contains("\n  \n"),
            "boşluk-only düğüm atlanmalı: {:?}",
            pretty_out
        );
    }

    /// Pretty çıktı yeniden ayrıştırıldığında boşluk her turda büyümemeli.
    #[test]
    fn pretty_output_is_stable_across_round_trips() {
        let xml = "<r><a>bir</a><b>iki</b></r>";
        let node = DomParser::parse_str(xml).unwrap();
        let once = node.borrow().to_pretty_string(2);
        let re = DomParser::parse_str(&once).unwrap();
        let twice = re.borrow().to_pretty_string(2);
        assert_eq!(once, twice, "pretty çıktı idempotent olmalı");
    }
```

- [ ] **Adım 2: Testin başarısız olduğunu doğrula**

Run: `cargo test --lib writer 2>&1 | tail -20`
Expected: FAIL — `pretty_mode_does_not_trim_cdata_content` (`"ab"` yazılır, `" ab "` değil).

- [ ] **Adım 3: Kırpmayı kaldır**

`src/writer.rs:101-111` — `IksType::CData` kolunu değiştir:

```rust
            IksType::CData => {
                if let Some(text) = node.content() {
                    if self.pretty && self.depth > 0 {
                        // Pretty modda sırf biçimlendirme boşluğu olan düğümler
                        // atlanır; bu, çıktının idempotent (ve dolayısıyla
                        // round-trip'te kararlı) kalmasını sağlar. `trim()`
                        // burada **yalnızca bir yüklem** olarak kullanılır:
                        // yazılan baytlar kırpılmaz.
                        if !text.trim().is_empty() {
                            self.write_indent()?;
                            crate::escape::write_escaped(&mut self.writer, text)?;
                            self.writer.write_all(b"\n")?;
                        }
                    } else {
                        // Yoğun mod hiçbir şeyi atlamaz ve hiçbir şeyi kırpmaz.
                        crate::escape::write_escaped(&mut self.writer, text)?;
                    }
                }
            }
```

- [ ] **Adım 4: Testlerin geçtiğini doğrula**

Run: `cargo test --lib writer 2>&1 | tail -20`
Expected: PASS — 5 test.

- [ ] **Adım 5: Round-trip testlerinin yeşil kaldığını doğrula**

Run: `cargo test --test dom_writer_traversal 2>&1 | tail -20`
Expected: PASS.

- [ ] **Adım 6: Commit**

```bash
git add src/writer.rs
git commit -m "fix(writer): pretty modda CDATA içeriğini kırpma

Pretty mod \" ab \" gibi karma içeriği \"ab\"ye indiriyordu — pretty yazıcı
düğümler arasına boşluk ekleyebilir ama bir düğümün kendi içeriğinin
baytlarını değiştiremez. Sırf boşluk olan düğümlerin atlanması korunur,
böylece çıktı idempotent kalır.

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Görev 10: DOM tamamlanma — `current`/`root` ayrımı ve `finish()` (A2)

**Files:**
- Modify: `src/parser.rs:67-96` (`SaxHandler::on_finish`), `src/parser.rs:381-394` civarı (`Parser::finish`)
- Modify: `src/dom.rs:40-58` (struct + `new`), `src/dom.rs:94-112` (`parse_str`, `parse_str_with_limits`), `src/dom.rs:165-231` (`on_tag`, `on_cdata`, `on_finish`)
- Test: `src/dom.rs` test modülü

**Interfaces:**
- Consumes: Görev 7'den `crate::append_text`
- Produces:
  - `SaxHandler::on_finish(&mut self) -> Result<()>` (varsayılan gövde `Ok(())`)
  - `Parser::finish(&mut self) -> Result<()>`
  - `DomParser` artık `root` + `current` alanlarını tutar; `node_stack` kaldırılır.

- [ ] **Adım 1: Başarısız testleri yaz**

`src/dom.rs` test modülüne ekle:

```rust
    /// C yalnızca kök element kapandığında `*iksptr`'yi yazar; kapanmamış
    /// belge için kök vermez. Rust `finish()` ile bunu hataya çevirir
    /// (spec D1: C'nin ölü `finish` parametresine anlam kazandırma kararı).
    #[test]
    fn unfinished_document_is_an_error() {
        assert!(matches!(
            DomParser::parse_str("<r><c/>"),
            Err(IksError::BadXml)
        ));
        assert!(matches!(DomParser::parse_str("<r>"), Err(IksError::BadXml)));
    }

    /// C: kapanış etiketi `current` NULL iken gelirse IKS_BADXML
    /// (`iks_strcmp(NULL, name)` = -1). Oracle: `</x>` → err=2,
    /// `<a/></b>` → err=2, `<a></a></b>` → err=2.
    #[test]
    fn stray_closing_tag_is_an_error() {
        assert!(matches!(
            DomParser::parse_str("</x>"),
            Err(IksError::BadXml)
        ));
        assert!(matches!(
            DomParser::parse_str("<a/></b>"),
            Err(IksError::BadXml)
        ));
        assert!(matches!(
            DomParser::parse_str("<a></a></b>"),
            Err(IksError::BadXml)
        ));
    }

    /// Hata verilse bile, kök kapandıysa ağaç handler'da kalır
    /// (C: `<a/></b>` → err=2 ama `*iksptr` dolu).
    #[test]
    fn completed_root_survives_a_later_error() {
        let parser = DomParser::new().unwrap();
        let mut sax = crate::Parser::new(parser);
        assert!(sax.parse("<a/></b>").is_err());
        let doc = sax.handler().document().expect("kök teslim edilmiş olmalı");
        assert_eq!(doc.borrow().name(), Some("a"));
    }

    /// C: kök kapanmadan sonra gelen yeni bir üst düzey element kökü **ezer**
    /// (son kök kazanır). Oracle: `<a/><b/>` → `iks_string` = `<b/>`.
    #[test]
    fn last_top_level_element_wins() {
        let dom = DomParser::parse_str("<a/><b/>").unwrap();
        assert_eq!(dom.borrow().name(), Some("b"));
    }

    /// C: kök kapanmadan önceki üst düzey metin atılır.
    #[test]
    fn text_before_root_is_discarded() {
        let dom = DomParser::parse_str("text<r/>").unwrap();
        assert_eq!(dom.borrow().name(), Some("r"));
        assert!(dom.borrow().children.is_empty());
    }

    /// Kapanış etiketi adı `current`'ın adıyla uyuşmazsa hata.
    #[test]
    fn mismatched_closing_tag_is_an_error() {
        assert!(matches!(
            DomParser::parse_str("<a><b></c></a>"),
            Err(IksError::BadXml)
        ));
    }
```

- [ ] **Adım 2: Testlerin başarısız olduğunu doğrula**

Run: `cargo test --lib dom::tests 2>&1 | tail -40`
Expected: FAIL — derleme hatası (`Parser::finish` ve `SaxHandler::on_finish` yok) ve davranış hataları (`<r><c/>` şu an `Ok`, `<a/></b>` şu an `Ok`).

- [ ] **Adım 3: `SaxHandler::on_finish` ve `Parser::finish` ekle**

`src/parser.rs`, `SaxHandler` trait'inde `on_cdata`'dan sonra:

```rust
    /// Belge bittiğinde çağrılır.
    ///
    /// Varsayılan gövde hiçbir şey yapmaz; bu, trait'i uygulayan mevcut
    /// tiplerin (DOM dışındaki tüm handler'lar) değişmeden derlenmesini
    /// sağlar.
    fn on_finish(&mut self) -> Result<()> {
        Ok(())
    }
```

`src/parser.rs`, `Parser` impl bloğunda `reset`'ten sonra:

```rust
    /// Belgenin bittiğini handler'a bildirir.
    ///
    /// C `iks_parse`'ın `finish` parametresi ölü koddur (`sax.c:634-644`
    /// onu hiçbir yere geçirmez). Bu metot o parametrenin amaçlanan anlamını
    /// gerçekler: handler'ın "belge tamamlandı" bilgisine göre karar
    /// vermesini sağlar.
    pub fn finish(&mut self) -> Result<()> {
        self.handler.on_finish()
    }
```

- [ ] **Adım 4: `DomParser`'ı yeniden yaz**

`src/dom.rs:40-58` — struct ve kurucu:

```rust
pub struct DomParser {
    /// C: `*iksptr` — yalnızca kök element **kapandığında** yazılır.
    root: Option<Rc<RefCell<IksNode>>>,
    /// C: `data->current` — hâlen açık olan en içteki düğüm.
    current: Option<Rc<RefCell<IksNode>>>,
    chunk_size: usize,
}

impl DomParser {
    pub fn new() -> Result<Self> {
        Ok(DomParser {
            root: None,
            current: None,
            chunk_size: memory::DEFAULT_IKS_CHUNK_SIZE,
        })
    }
```

`src/dom.rs:94-112` — `parse_str` ve `parse_str_with_limits`:

```rust
    pub fn parse_str(xml: &str) -> Result<Rc<RefCell<IksNode>>> {
        let parser = DomParser::new()?;
        let mut sax_parser = crate::Parser::new(parser);
        sax_parser.parse(xml)?;
        sax_parser.finish()?;
        sax_parser.handler().document().ok_or(IksError::BadXml)
    }

    pub fn parse_str_with_limits(
        xml: &str,
        limits: crate::ParserLimits,
    ) -> Result<Rc<RefCell<IksNode>>> {
        let parser = DomParser::new()?;
        let mut sax_parser = crate::Parser::with_limits(parser, limits);
        sax_parser.parse(xml)?;
        sax_parser.finish()?;
        sax_parser.handler().document().ok_or(IksError::BadXml)
    }
```

`src/dom.rs:165-231` — `on_tag`, `on_cdata`, `on_finish` üçlüsünü değiştir:

```rust
    fn on_tag(
        &mut self,
        name: &str,
        attributes: &[(String, String)],
        tag_type: TagType,
    ) -> Result<()> {
        match tag_type {
            TagType::Open | TagType::Single => {
                let mut node = IksNode::new_tag(name);
                node.attributes.extend(attributes.iter().cloned());

                match self.current.clone() {
                    Some(parent) => {
                        // `add_child` parent/prev/next bağlarını kurar ve
                        // yerleştirilmiş `Rc`'yi döndürür.
                        let node_rc = parent.borrow_mut().add_child(node);
                        if tag_type == TagType::Open {
                            self.current = Some(node_rc);
                        }
                    }
                    None => {
                        // Üst düzey element: C `*iksptr`'yi burada yazar.
                        // Kök zaten varsa **ezilir** — son kök kazanır.
                        let node_rc = node.into_rc();
                        if tag_type == TagType::Open {
                            self.current = Some(node_rc);
                        } else {
                            self.root = Some(node_rc);
                        }
                    }
                }
            }
            TagType::Close => {
                // C `iks_strcmp(NULL, name)` = -1 → IKS_BADXML.
                let Some(current) = self.current.clone() else {
                    return Err(IksError::BadXml);
                };
                if current.borrow().name.as_deref() != Some(name) {
                    return Err(IksError::BadXml);
                }
                match current.borrow().parent() {
                    Some(parent) => self.current = Some(parent),
                    None => {
                        self.root = Some(current);
                        self.current = None;
                    }
                }
            }
        }
        Ok(())
    }

    fn on_cdata(&mut self, data: &str) -> Result<()> {
        // C `cdataHook` koşulsuz `iks_insert_cdata` çağırır. `current` NULL
        // iken gelen metin (kökten önceki üst düzey metin) atılır.
        if let Some(parent) = self.current.as_ref() {
            crate::append_text(parent, data);
        }
        Ok(())
    }

    fn on_finish(&mut self) -> Result<()> {
        // Kapanmamış bir element varsa belge eksiktir.
        if self.current.is_some() {
            return Err(IksError::BadXml);
        }
        Ok(())
    }
```

**Kritik:** `on_cdata` gövdesi Görev 7'de `self.node_stack.last()` kullanıyordu; burada `self.current.as_ref()` olur. `node_stack` alanı tamamen kalkar. `document()` (78) değişmez — artık tam olarak `*iksptr` anlamına gelir.

- [ ] **Adım 5: Testlerin geçtiğini doğrula**

Run: `cargo test --lib dom 2>&1 | tail -30`
Expected: PASS — mevcut 5 test + 6 yeni test.

- [ ] **Adım 6: Tam paket**

Run: `cargo test --all-features 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: hiç `FAILED` yok. `tests/security_dos_limits.rs` `parse_str_with_limits` kullanır; sınır aşıldığında `parse` zaten hata verdiği için `finish` çağrılmaz ve davranış değişmez.

- [ ] **Adım 7: Clippy ve doctest**

Run:
```bash
cargo clippy --all-targets --all-features 2>&1 | tail -20
cargo test --doc 2>&1 | tail -10
```
Expected: clippy temiz; 3 doctest geçer.

- [ ] **Adım 8: Commit**

```bash
git add src/parser.rs src/dom.rs
git commit -m "fix(dom): kök sahipliğini C'nin current/root ayrımına çevir

DomParser kökü açılış anında yazıyordu; C yalnızca kök element
kapandığında yazar. Ayrıca sarkan kapanış etiketi (</x>, <a/></b>)
sessizce yutuluyordu, C ise IKS_BADXML verir.

SaxHandler::on_finish (varsayılan gövdeli) ve Parser::finish eklendi;
parse_str ve parse_str_with_limits belge bittiğinde bunu çağırır. Bu,
C'nin ölü finish parametresine amaçlanan anlamı verir (D1).

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

## Son doğrulama

- [ ] **Adım 1: Tam paket + clippy + doctest**

Run:
```bash
cargo test --all-features 2>&1 | grep -E "test result|FAILED"
cargo clippy --all-targets --all-features 2>&1 | tail -5
cargo test --doc 2>&1 | tail -5
```
Expected: hiç `FAILED` yok; clippy uyarısız; doctest 3/3.

- [ ] **Adım 2: C oracle ile differential karşılaştırma**

`/tmp/iksprobe/oracle.c` yaz (kaçış ve DOM tablolarını üreten program) ve şu komutla derle:

```bash
mkdir -p /tmp/iksprobe && cc -w \
  -I/Users/zaryob/Development/iksemel/include \
  -o /tmp/iksprobe/oracle /tmp/iksprobe/oracle.c \
  /Users/zaryob/Development/iksemel/src/{iks,sax,dom,ikstack,utility}.c
```

Çıktısını, Rust tarafının aynı belgeler için ürettiği çıktıyla karşılaştır. İstisnalar yalnızca spec §8'de listelenen üç tanedir:

1. **D9 aralıkları** — `U+0200`–`U+03FF`, `U+0600`–`U+07FF`: C `0xE8` maske hatası yüzünden çöp üretir, Rust doğru kod noktasını yazar.
2. **`U+0000`** — iki taraf da reddeder (bağlam farkı yok).
3. **Kapanmamış belge** — C `IKS_OK` + boş kök, Rust `BadXml` (D1).

- [ ] **Adım 3: Özet**

```bash
git log --oneline -10
```
Beklenen: 10 görev commit'i, her biri kendi testiyle.

## Kapsam dışı

Spec §10 ile aynı: NodeRef/düğüm modeli (B), filtre ve IQ yönlendirme (C), yeni entegrasyon API'leri (D), kripto (E), C ABI (F), modern katman (G), C reposunda değişiklik, `ParserLimits`'in C ile hizalanması (C'de limit yoktur).
