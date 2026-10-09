# c-parity-c inceleme bulgularının kapatılması

Kaynak: `target/reviews/c-parity-c/REVIEW.md` (2026-10-09). Sürüm 0.3.6 kalır; manifest sürümü, tag, release, push yok.

## Mevcut durum (2026-10-10, b906617 + çalışma ağacı)

| Bulgu | Kod | Test | Kalan |
|---|---|---|---|
| P1 async IQ iptali stanza kaybı | `recv_iq_response` artık `pending_events` içinde yerinde arıyor, eşleşmeyenleri hiç çıkarmıyor | Yok | Regresyon testleri |
| P2 FROM/FROM_PARTIAL C paritesi | `PacketJid` (ham, `jabber:` öneki soyuluyor), kurallar `String` | Yok | Parite testleri, API notu |
| P2 `remove_hook` | `FilterHook` + `add_rule_with_hook` + `remove_hook` (Arc::ptr_eq) | Yok | Testler |
| P2 fmt | — | — | `cargo fmt --all` |
| ns iddiası | Kod doğru | Test adı yanıltıcı | Test adı + 3 doküman + `filter.rs` yorumu |
| (yeni) clippy | `src/actor.rs:59` RefCell await üstünde; `src/stream.rs:318` test modülünden sonra öğe | — | Düzelt |

## Görev A — async iptal güvenliği (Sonnet ajanı)

Dosyalar: `src/async_net.rs`, `src/actor.rs`, yeni `tests/async_iq_cancellation.rs`.

1. `tests/async_iq_cancellation.rs` — `target/reviews/c-parity-c/cancellation.rs` mock TCP sunucusunu temel al (`#[tokio::test]`):
   - `external_timeout_preserves_buffered_stanza`: `message id=before` → `timeout(100ms, recv_iq_response("missing"))` → Err; serbest bırak → sonraki `recv_stanza` `before`, ardından `after`.
   - `select_cancel_preserves_fifo_order`: üç stanza (`m1`, `iq id=other`, `m2`) + `select!` ile iptal → sıra `m1, iq other, m2`.
   - `matching_iq_is_extracted_others_kept`: `m1`, `iq id=x type=result`, `m2` → `recv_iq_response("x")` iq döner; sonra `m1`, `m2` sırayla.
   - `stream_end_while_waiting_returns_net_dropped`: eşleşme yok + `</stream:stream>` → `Err(NetDropped)`; önceki mesaj kuyrukta kalmalı.
   - Önce testleri değişikliği geri alarak (`git stash` KULLANMA; geçici olarak eski gövdeyi yazıp) başarısız olduğunu doğrula, sonra güncel koda geri dön. Kabul edilmezse en az birinci testin eski kodda kırıldığını gerekçelendir.
2. `read_events` iptal güvenliğini gözden geçir: `read` tamamlanmadan durum değişmemeli; `read` sonrası kod senkron. Sorun varsa düzelt ve testle.
3. `recv_iq_response` doc yorumunu iptal güvenliğini belirtecek şekilde güncelle.
4. `src/actor.rs:59` clippy `await_holding_refcell_ref`: düğümü `borrow()` ile klonla/sahiplen ve borç await'ten önce düşsün.
5. Doğrulama: `cargo test --all-features --test async_iq_cancellation`, `cargo clippy --all-targets --all-features -- -D warnings` (yalnız kendi dosyaların için temiz).

## Görev B — filtre/JID paritesi ve dokümanlar (Sonnet ajanı)

Dosyalar: `tests/filter_parity_c.rs`, `src/filter.rs` (yalnız test modülü/yorumlar + gerekirse küçük düzeltme), `src/jid.rs` testleri, `src/stream.rs`, `docs/superpowers/{plans,specs}/*C-classification*`, `docs/superpowers/specs/2026-10-09-iksemel-rs-c-parity-roadmap.md`, `CHANGELOG.md` (varsa).

1. `tests/filter_parity_c.rs` içine C oracle'ı (`target/reviews/c-parity-c/comparison.json`) baz alan testler:
   - `Bob@Example.COM/phone`: küçük harfli `with_from("bob@example.com/phone")` ve `with_from_partial("bob@example.com")` TETİKLENMEZ; birebir `Bob@Example.COM/phone` / `Bob@Example.COM` TETİKLENİR.
   - `jabber:bob@example.com/phone`: `pak.from.full == "bob@example.com/phone"`, FROM ve FROM_PARTIAL eşleşir.
   - `example.com/resource@device`: `user=None`, `server="example.com"`, `resource="resource@device"`; FROM eşleşir.
   - comparison.json'daki diğer farklı JID girdilerini de C çıktısıyla kilitle (boş/geçersiz adres: C ne yapıyorsa `PacketJid` aynısını yapmalı, çünkü artık ham).
   - `Jid` ile kural (`with_from(Jid::new(..)?)`) geriye dönük uyumluluk testi.
2. `remove_hook` testleri: aynı `FilterHook` ile 3 kural + farklı hook ile 1 kural → `remove_hook` 3 döner, diğer hook hâlâ çağrılır; ikinci çağrı 0 döner; `add_rule` kurallarını etkilemez; `remove_rule` ile tek tek silme hâlâ çalışır.
3. `src/jid.rs` testi: `Jid::new("example.com/resource@device")` başarılı, resource `resource@device`; `user@host/res@x` node=`user`.
4. ns: `test_c_parity_ns_only_iq_first_tag_child` → `test_c_parity_ns_from_first_iq_child_with_xmlns` olarak yeniden adlandır; `<iq><first/><query xmlns='custom:ns'/></iq>` → `custom:ns` testi yoksa ekle. `src/filter.rs:780` yorumu ve 3 dokümandaki "yalnız ilk tag çocuğu" ifadelerini "xmlns niteliği taşıyan ilk tag çocuğu (jabber.c:146-155)" olarak düzelt. Kodu DEĞİŞTİRME.
5. `src/stream.rs:318` clippy `items_after_test_module`: `stanza_event` vb. öğeleri test modülünün üstüne taşı.
6. `IksPacket.from: Option<Jid>` → `Option<PacketJid>` ve `RuleBuilder.from*` → `String` kırıcı API değişikliğidir; CHANGELOG varsa "Unreleased / Breaking" altına kaydet. Sürüm numarasına dokunma.
7. Doğrulama: `cargo test --all-features --test filter_parity_c`, `cargo test --lib`.

## Görev C — entegrasyon (ana oturum)

1. `cargo fmt --all`.
2. `cargo test --all-features`, `cargo test --all-targets --all-features`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`.
3. `target/reviews/c-parity-c/cancellation.rs` senaryosunun artık `before` döndürdüğünü test ile teyit.
4. Commit/push yok; kullanıcıya özet.

## Kapsam dışı

Çalışma ağacındaki actor/builders/digest_md5/transport/ffi/python/SM-resume/stringprep işleri bu raporun parçası değil; yalnız clippy/fmt'i geçecek kadar dokunulur.
