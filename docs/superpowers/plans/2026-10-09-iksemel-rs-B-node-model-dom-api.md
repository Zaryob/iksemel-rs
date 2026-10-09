# iksemel-rs — Alt proje B Uygulama Planı: Düğüm Modeli, NodeRef ve DOM Düzenleme API'leri

- **Tarih:** 2026-10-09
- **Durum:** Hazır / Yürütülüyor
- **Üst belge:** [`docs/superpowers/specs/2026-10-09-iksemel-rs-B-node-model-dom-api.md`](../specs/2026-10-09-iksemel-rs-B-node-model-dom-api.md)
- **Branch:** `feat/c-parity-b` (merge base `f943ae3`)
- **TDD İlkesi:** Her görevde önce başarısız test (RED), ardından minimal uygulama (GREEN), clippy kontrolü ve commit.

---

## Görev Listesi

| Görev | Başlık | Dokunulan Dosyalar | Commit Türü |
|---|---|---|---|
| **Görev 1** | `IksNode` Attribute Upsert, Silme, `set_content` ve `hide` | `src/lib.rs` | `fix(dom): attribute upsert, silme, set_content ve hide uygula` |
| **Görev 2** | `NodeRef` Veri Tipi ve Temel Yönlendiriciler | `src/lib.rs` | `feat(dom): NodeRef sarmalayıcısını ve temel API'lerini ekle` |
| **Görev 3** | `NodeRef` Ağaç Operasyonları (`add_child`, `clone_subtree`, navigasyon) | `src/lib.rs` | `feat(dom): NodeRef ağaç operasyonları ve clone_subtree ekle` |
| **Görev 4** | `DomParser`'ın `NodeRef` Döndürmesi | `src/dom.rs` | `refactor(dom): DomParser çıkışını NodeRef'e çevir` |
| **Görev 5** | `StreamEvent`'in `NodeRef` Taşıması ve Bağlı Ağaç Teslimi | `src/stream.rs` | `fix(stream): StreamEvent::Stanza'yı bağlı NodeRef olarak ilet` |
| **Görev 6** | Ağ Katmanının (`net.rs`, `async_net.rs`) `NodeRef` Uyumu | `src/net.rs`, `src/async_net.rs` | `refactor(net): recv_stanza dönüşünü NodeRef yap` |
| **Görev 7** | XEP, Roster, SASL ve Araçların `NodeRef` Göçü | `src/xep.rs`, `src/roster.rs`, `src/sasl.rs`, `tools/*` | `refactor: protokol ve araçları NodeRef API'sine geçir` |
| **Görev 8** | Alt-proje B Parite Entegrasyon Testleri ve Kapanış | `tests/dom_parity_b.rs` | `test(dom): alt-proje B parite ve entegrasyon testlerini ekle` |

---

## Görev 1: `IksNode` Attribute Upsert, Silme, `set_content` ve `hide`

**Hedef:**
1. `add_attribute`: Mevcut bir attribute varsa değerini günceller (upsert), yoksa sona ekler.
2. `remove_attribute`: Adı eşleşen attribute'u siler, silindiyse `true` döner.
3. `set_content`: Önce tüm çocukları temizler (`children.clear()`), ardından içeriği ayarlar.
4. `hide`: Kendisini parent ve komşu (prev/next) bağlarından söker.

- [ ] **Adım 1:** Testleri yaz: `test_attribute_upsert_and_removal`, `test_set_content_clears_children`, `test_iks_node_hide`.
- [ ] **Adım 2:** Testlerin başarısız olduğunu gör (RED).
- [ ] **Adım 3:** `src/lib.rs` içinde metotları uygula.
- [ ] **Adım 4:** Testlerin geçtiğini doğrula (GREEN).
- [ ] **Adım 5:** Commit: `fix(dom): attribute upsert, silme, set_content ve hide uygula`.

---

## Görev 2: `NodeRef` Veri Tipi ve Temel Yönlendiriciler

**Hedef:**
`NodeRef(pub Rc<RefCell<IksNode>>)` tipini ve `new`, `new_tag`, `new_cdata`, `borrow`, `borrow_mut`, `add_attribute`, `remove_attribute`, `find_attrib`, `has_attribute`, `name`, `text` yönlendiricilerini eklemek.

- [ ] **Adım 1:** `src/lib.rs` test modülüne `NodeRef` kurucu ve attribute testlerini ekle.
- [ ] **Adım 2:** Derleme hatası / başarısız olduğunu gör (RED).
- [ ] **Adım 3:** `NodeRef` struct ve temel impl bloğunu yaz.
- [ ] **Adım 4:** Testleri geçir (GREEN).
- [ ] **Adım 5:** Commit: `feat(dom): NodeRef sarmalayıcısını ve temel API'lerini ekle`.

---

## Görev 3: `NodeRef` Ağaç Operasyonları (`add_child`, `clone_subtree`, navigasyon)

**Hedef:**
1. `add_child(&self, child: impl Into<NodeRef>) -> NodeRef`: Çocuğun parent'ını koşulsuz `Rc::downgrade` ile bağlar.
2. `clone_subtree(&self) -> NodeRef`: Alt ağacı eksiksiz bağlı kopyalar.
3. `parent(&self) -> Option<NodeRef>`, `first_child`, `last_child`, `next`, `prev`, `children`, `child_tags`, `hide`, `append_cdata`, `prepend_cdata`.

- [ ] **Adım 1:** `tests/text_fidelity.rs` veya `src/lib.rs` içine 4a ve 4b doğrulama testlerini yaz (değer kök parent bağı, clone_subtree bağı).
- [ ] **Adım 2:** Testlerin başarısız olduğunu gör (RED).
- [ ] **Adım 3:** `NodeRef` ağaç metotlarını uygula.
- [ ] **Adım 4:** Testleri geçir (GREEN).
- [ ] **Adım 5:** Commit: `feat(dom): NodeRef ağaç operasyonları ve clone_subtree ekle`.

---

## Görev 4: `DomParser`'ın `NodeRef` Döndürmesi

**Hedef:**
`DomParser::parse_str`, `parse_str_with_limits`, `load_file`, `document` metotlarını `NodeRef` döndürecek şekilde güncellemek.

- [ ] **Adım 1:** `src/dom.rs` testlerinde dönüş tipini `NodeRef` olarak bekle.
- [ ] **Adım 2:** Derleme hatası / başarısızlık (RED).
- [ ] **Adım 3:** `src/dom.rs` içindeki metotları güncelle.
- [ ] **Adım 4:** `cargo test --lib dom` geçir (GREEN).
- [ ] **Adım 5:** Commit: `refactor(dom): DomParser çıkışını NodeRef'e çevir`.

---

## Görev 5: `StreamEvent`'in `NodeRef` Taşıması ve Bağlı Ağaç Teslimi

**Hedef:**
`StreamEvent::Stanza(NodeRef)` ve `StreamEvent::StreamStart(NodeRef)` yaparak stanzanın çocuklarında parent ve namespace bağlarını korumak (4c).

- [ ] **Adım 1:** Stanza çocuklarının `parent()` bağını doğrulayan test yaz.
- [ ] **Adım 2:** Testin başarısız olduğunu gör (RED).
- [ ] **Adım 3:** `src/stream.rs` içinde `StreamEvent` ve `StreamDispatcher`'ı güncelle.
- [ ] **Adım 4:** `cargo test --lib stream` geçir (GREEN).
- [ ] **Adım 5:** Commit: `fix(stream): StreamEvent::Stanza'yı bağlı NodeRef olarak ilet`.

---

## Görev 6: Ağ Katmanının (`net.rs`, `async_net.rs`) `NodeRef` Uyumu

**Hedef:**
`recv_stanza` ve ilgili okuyucuları `NodeRef` döndürecek şekilde bağlamak.

- [ ] **Adım 1:** Ağ testlerinde `recv_stanza` dönüş tiplerini doğrula.
- [ ] **Adım 2:** `src/net.rs` ve `src/async_net.rs` dosyalarını güncelle.
- [ ] **Adım 3:** Ağ testlerini geçir (GREEN).
- [ ] **Adım 4:** Commit: `refactor(net): recv_stanza dönüşünü NodeRef yap`.

---

## Görev 7: XEP, Roster, SASL ve Araçların `NodeRef` Göçü

**Hedef:**
Kalan `src/xep.rs`, `src/roster.rs`, `src/sasl.rs`, `tools/` ve `tests/` çağrı yerlerini derleyici kılavuzluğunda `NodeRef`'e geçirmek.

- [ ] **Adım 1:** Derleyici hatalarını sırayla çöz.
- [ ] **Adım 2:** Tüm birim ve entegrasyon testlerini çalıştır.
- [ ] **Adım 3:** Commit: `refactor: protokol ve araçları NodeRef API'sine geçir`.

---

## Görev 8: Alt-proje B Parite Entegrasyon Testleri ve Kapanış

**Hedef:**
`tests/dom_parity_b.rs` dosyasında 4a, 4b, 4c, 5 ve tüm yeni DOM API'lerini C oracle'ına karşı kilitlemek.

- [ ] **Adım 1:** `tests/dom_parity_b.rs` test paketini oluştur.
- [ ] **Adım 2:** `cargo test --all-features` ile tüm testlerin yeşil olduğunu doğrula.
- [ ] **Adım 3:** `cargo clippy --all-targets --all-features` sıfır uyarı ile doğrula.
- [ ] **Adım 4:** Commit: `test(dom): alt-proje B parite ve entegrasyon testlerini ekle`.
