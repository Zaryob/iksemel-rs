# Alt-Proje C Uygulama Planı: Paket Sınıflandırması (`ikspak`), Filtre Semantiği (`PacketFilter`) ve IQ Yönlendirme

**Tarih:** 2026-10-09  
**Hedef Dal:** `feat/c-parity-c`  
**Referans Spec:** `docs/superpowers/specs/2026-10-09-iksemel-rs-C-classification-filter-iq.md`

---

## Görev Listesi

| Görev | Başlık | Dosyalar | Commit Mesajı |
|---|---|---|---|
| **Görev 1** | `ikspak` Paket Sınıflandırması ve Tipleri | `src/filter.rs`, `src/lib.rs` | `feat(packet): ikspak paket sınıflandırması ve tiplerini ekle` |
| **Görev 2** | Ağırlıklı Skorlamalı `PacketFilter` ve `Eat` Akış Kesme | `src/filter.rs` | `feat(filter): ağırlıklı puanlama ve FilterStatus::Eat akış kontrolünü ekle` |
| **Görev 3** | Kural Kaldırma (`remove_rule`) | `src/filter.rs` | `feat(filter): remove_rule kural kaldırma API'sini ekle` |
| **Görev 4** | `Connection` ve `AsyncConnection`'da `recv_iq_response` | `src/net.rs`, `src/async_net.rs` | `feat(net): id ile eşleşen recv_iq_response ekle` |
| **Görev 5** | Roster, SASL ve Bind Akışlarında IQ ID Eşleştirmesi | `src/roster.rs`, `src/sasl.rs`, `src/async_net.rs` | `refactor: roster, sasl ve bind akışlarında id tabanlı iq eşleştirmesini kullan` |
| **Görev 6** | Alt-Proje C Entegrasyon Testleri ve Kapanış | `tests/filter_parity_c.rs` | `test(filter): alt-proje C paket sınıflandırması ve filtre parite testlerini ekle` |

---

## Görev 1: `ikspak` Paket Sınıflandırması ve Tipleri

**Hedef:**
C `jabber.c:68-159` kural tablosuna birebir uygun `IksPacketType`, `IksSubtype`, `IksShowType`, `FilterStatus` ve `IksPacket::from_node` inşası.
Özellikle `ns` alanının yalnızca `iq`'nun ilk tag çocuğundan alınması kuralı (C `jabber.c:146-156`).

- [ ] **Adım 1:** `src/filter.rs` içinde paket sınıflandırmasını doğrulayan testleri yaz (RED).
- [ ] **Adım 2:** Tipleri ve `IksPacket::from_node`/`from_node_ref` fonksiyonunu uygula.
- [ ] **Adım 3:** Testlerin geçtiğini gör (GREEN).
- [ ] **Adım 4:** Commit: `feat(packet): ikspak paket sınıflandırması ve tiplerini ekle`.

---

## Görev 2: Ağırlıklı Skorlamalı `PacketFilter` ve `Eat` Akış Kesme

**Hedef:**
`filter.c:115-167` mantığına birebir uygun puanlama:
- ID = 16, FROM = 8, FROM_PARTIAL = 8, NS = 4, SUBTYPE = 2, TYPE = 1.
- Herhangi bir kriter tutmazsa skor = 0 (kısmi puan yok).
- Hiç kriteri olmayan kural çalışmaz (skor = 0).
- En yüksek skorlu kural önce çalışır.
- `FilterStatus::Eat` dönerse döngü derhal durur.

- [ ] **Adım 1:** Puanlama sırasını ve `Eat` durumunu test eden testleri yaz (RED).
- [ ] **Adım 2:** `PacketFilter::filter_packet` ve `RuleBuilder` skorlama mantığını uygula.
- [ ] **Adım 3:** Testlerin geçtiğini gör (GREEN).
- [ ] **Adım 4:** Commit: `feat(filter): ağırlıklı puanlama ve FilterStatus::Eat akış kontrolünü ekle`.

---

## Görev 3: Kural Kaldırma (`remove_rule`)

**Hedef:**
`PacketFilter::remove_rule(&mut self, id: RuleId) -> bool` eklemek.

- [ ] **Adım 1:** Kural ekleme, `RuleId` alma ve kuralı silip artık tetiklenmediğini doğrulayan test yaz (RED).
- [ ] **Adım 2:** `remove_rule` metodunu uygula.
- [ ] **Adım 3:** Testlerin geçtiğini gör (GREEN).
- [ ] **Adım 4:** Commit: `feat(filter): remove_rule kural kaldırma API'sini ekle`.

---

## Görev 4: `Connection` ve `AsyncConnection`'da `recv_iq_response`

**Hedef:**
`Connection::recv_iq_response(&mut self, expected_id: &str) -> Result<NodeRef>` ve
`AsyncConnection::recv_iq_response(&mut self, expected_id: &str) -> Result<NodeRef>` eklemek.
Araya giren stanzalar `pending_events` içine geri konur, böylece akıştan kaybolmazlar.

- [ ] **Adım 1:** Araya giren `<presence>` ve `<message>` stanzaları varken beklenen `<iq id='...'>`'nun bulunduğunu ve araya giren stanzaların bir sonraki okumada kaybolmadığını test et (RED).
- [ ] **Adım 2:** `recv_iq_response` metotlarını uygula.
- [ ] **Adım 3:** Testlerin geçtiğini gör (GREEN).
- [ ] **Adım 4:** Commit: `feat(net): id ile eşleşen recv_iq_response ekle`.

---

## Görev 5: Roster, SASL ve Bind Akışlarında IQ ID Eşleştirmesi

**Hedef:**
`fetch_roster`, `sync_roster`, `bind_resource`, `bind_resource_async`, `establish_session`, `authenticate_non_sasl` çağrılarını `recv_iq_response` ile bağlamak.

- [ ] **Adım 1:** Roster ve SASL çağrılarında araya giren stanzalara karşı dayanıklılık testleri yaz.
- [ ] **Adım 2:** Fonksiyonları güncelle.
- [ ] **Adım 3:** Tüm testlerin geçtiğini gör (GREEN).
- [ ] **Adım 4:** Commit: `refactor: roster, sasl ve bind akışlarında id tabanlı iq eşleştirmesini kullan`.

---

## Görev 6: Alt-Proje C Entegrasyon Testleri ve Kapanış

**Hedef:**
`tests/filter_parity_c.rs` dosyasında C oracle davranışlarını kilitlemek, tüm test ve linter kontrollerini geçirmek.

- [ ] **Adım 1:** `tests/filter_parity_c.rs` dosyasını oluştur.
- [ ] **Adım 2:** `cargo test --all-features` ile doğrula.
- [ ] **Adım 3:** `cargo clippy --all-targets --all-features` sıfır uyarı ile doğrula.
- [ ] **Adım 4:** Commit: `test(filter): alt-proje C paket sınıflandırması ve filtre parite testlerini ekle`.
