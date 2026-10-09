# Alt-Proje C Detaylı Spesifikasyonu: Paket Sınıflandırması (`ikspak`), Filtre Semantiği (`PacketFilter`) ve IQ Yönlendirme

**Tarih:** 2026-10-09  
**Hedef Dal:** `feat/c-parity-c`  
**Referans Yol Haritası:** `docs/superpowers/specs/2026-10-09-iksemel-rs-c-parity-roadmap.md` (§3.3, §3.4, Rapor 6, 7)  
**C Referans Dosyaları:** `src/filter.c`, `src/jabber.c:68-159`, `include/iksemel.h:308-395`

---

## 1. Giriş ve Hedefler

Alt-proje C, yol haritasında tespit edilen **Rapor 6 (Filtre semantiği uyumsuz)** ve **Rapor 7 (IQ yanıtı isteğe bağlanmıyor)** kusurlarını C iksemel paritesinde çözmeyi amaçlar:

1. **`ikspak` Paket Sınıflandırması (`jabber.c:68-159`):**
   - Stanza adından `IksPacketType` (`Message`, `Presence`, `Iq`, `Subscription`),
   - Stanza niteliklerinden `IksSubtype` (`Chat`, `Get`, `Set`, `Result`, `Probe`, `Subscribe` vb.),
   - `<presence>` içeriğinden `IksShowType` (`Chat`, `Away`, `Xa`, `Dnd`, `Available`, `Unavailable`),
   - **`ns` Kuralı (Kusur 6):** `ns` **yalnızca ve yalnızca `Iq` stanzalarında** ve **yalnızca ilk tag çocuğunun `xmlns` niteliğinden** elde edilir. `message` veya `presence` için `ns` her zaman `None`'dır.
   - **`from` Alanı:** Tam JID (`from`) ve kısmi bare JID (`from_partial`) desteği.

2. **Ağırlıklı Skorlamalı Filtre (`filter.c:115-167`):**
   - Skor ağırlıkları: `ID = 16`, `FROM = 8`, `FROM_PARTIAL = 8`, `NS = 4`, `SUBTYPE = 2`, `TYPE = 1`.
   - **Tümü-ya-da-hiç kuralı:** Tanımlanan kriterlerden herhangi biri eşleşmezse kuralın tüm skoru `0` olur (kısmi puan yoktur).
   - **Ölçütsüz kural çalışmaz:** Hiçbir kriter tanımlanmamış bir kuralın skoru `0`'dır; `max_score = 0` kapısı nedeniyle asla tetiklenmez.
   - **En yüksek skor önceliği:** En yüksek skora sahip kural ilk çalıştırılır.
   - **`Eat` Akış Kesme:** Bir handler `FilterStatus::Eat` dönerse filtreleme işlemi derhal sonlanır, kalan hiçbir kural çalıştırılmaz.
   - **Kural ve Hook Silme:** `remove_rule(id: RuleId)` ve `remove_hook(...)` API'si.

3. **IQ Yanıtı ID Eşleştirme (Kusur 7):**
   - `fetch_roster`, `sync_roster`, `bind_resource`, `bind_resource_async`, `establish_session`, `authenticate_non_sasl` fonksiyonları sıradaki herhangi bir stanzayı yanıt kabul etmek yerine, gönderilen IQ'nun `id` niteliğiyle eşleşen `Iq` yanıtını bekler. Araya giren `presence` veya `message` stanzaları kuyrukta saklanır veya elenir.

---

## 2. Veri Tipleri ve API Tasarımı

### 2.1 Paket Tipleri (`src/filter.rs` veya `src/packet.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IksPacketType {
    None = 0,
    Message = 1,
    Presence = 2,
    Iq = 3,
    Subscription = 4, // C IKS_PAK_S10N
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IksSubtype {
    None = 0,
    Error = 1,
    Chat = 2,
    Groupchat = 3,
    Headline = 4,
    Get = 5,
    Set = 6,
    Result = 7,
    Subscribe = 8,
    Subscribed = 9,
    Unsubscribe = 10,
    Unsubscribed = 11,
    Probe = 12,
    Available = 13,
    Unavailable = 14,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IksShowType {
    Unavailable = 0,
    Available = 1,
    Chat = 2,
    Away = 3,
    Xa = 4,
    Dnd = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterStatus {
    Pass = 0,
    Eat = 1,
}
```

### 2.2 `IksPacket` Struct'ı

```rust
#[derive(Debug, Clone)]
pub struct IksPacket {
    pub packet_type: IksPacketType,
    pub subtype: IksSubtype,
    pub show: IksShowType,
    pub id: Option<String>,
    pub from: Option<Jid>,
    pub ns: Option<String>,
    pub query: Option<NodeRef>,
    pub node: NodeRef,
}

impl IksPacket {
    pub fn from_node(node: &IksNode) -> Self;
    pub fn from_node_ref(node: &NodeRef) -> Self;
}
```

### 2.3 `PacketFilter` ve `RuleBuilder`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleId(pub usize);

pub struct RuleBuilder {
    packet_type: Option<IksPacketType>,
    subtype: Option<IksSubtype>,
    id: Option<String>,
    from: Option<Jid>,
    from_partial: Option<Jid>,
    ns: Option<String>,
}

impl RuleBuilder {
    pub fn new() -> Self;
    pub fn with_type(mut self, packet_type: IksPacketType) -> Self;
    pub fn with_subtype(mut self, subtype: IksSubtype) -> Self;
    pub fn with_id<S: Into<String>>(mut self, id: S) -> Self;
    pub fn with_from(mut self, from: Jid) -> Self;
    pub fn with_from_partial(mut self, from_partial: Jid) -> Self;
    pub fn with_ns<S: Into<String>>(mut self, ns: S) -> Self;
}

pub struct PacketFilter {
    rules: Vec<InternalRule>,
    next_id: usize,
}

impl PacketFilter {
    pub fn new() -> Self;
    pub fn add_rule<F>(&mut self, builder: RuleBuilder, handler: F) -> RuleId
    where
        F: Fn(&IksPacket) -> FilterStatus + Send + Sync + 'static;

    pub fn remove_rule(&mut self, id: RuleId) -> bool;

    pub fn filter_packet(&self, pak: &IksPacket) -> FilterStatus;

    // Geriye dönük uyumluluk forwarder'ı
    pub fn dispatch(&self, stanza: &IksNode) -> usize;
    pub fn dispatch_ref(&self, stanza: &NodeRef) -> usize;
}
```

---

## 3. IQ Yanıtı Yönlendirme (`Connection` & `AsyncConnection`)

`recv_iq_response(expected_id: &str)` API'si:
1. `recv_event` döngüsünde stanzaları okur.
2. Stanza bir `iq` ise ve `id == expected_id` ise `Ok(stanza)` döner.
3. Eşleşmeyen stanzaları (ör. sunucudan gelen anlık `<presence>` veya `<message>`) `pending_events` kuyruğuna geri ekler veya saklar; böylece kaybolmazlar.
4. `fetch_roster`, `sync_roster`, `bind_resource`, `bind_resource_async`, `establish_session`, `authenticate_non_sasl` bu API'yi kullanır.

---

## 4. Kabul Kriterleri

1. **Ağırlıklı Puanlama Doğrulaması:**
   - ID (16) > FROM (8) > NS (4) > SUBTYPE (2) > TYPE (1) öncelik sırasıyla tetiklenir.
   - Eşleşmeyen bir kritere sahip kural skoru 0 alır ve çalışmaz.
   - Hiç kriteri olmayan kural çalışmaz.
2. **`Eat` Doğrulaması:**
   - `FilterStatus::Eat` dönen kuraldan sonra daha düşük puanlı eşleşen kurallar çalışmaz.
3. **`ns` Sınır Doğrulaması:**
   - `<message>` veya `<presence>` içine `xmlns` konsa dahi `ns` kuralı eşleşmez; yalnızca `iq`'nun ilk tag çocuğunun `xmlns`'i eşleşir.
4. **Subscription (`IKS_PAK_S10N`) Doğrulaması:**
   - `<presence type='subscribe'/>` `Subscription` tipi ve `Subscribe` alt tipi üretir.
   - `<presence type='probe'/>` `Presence` tipi ve `Probe` alt tipi üretir.
5. **IQ Eşleştirme Doğrulaması:**
   - `fetch_roster` veya `bind_resource` sırasında araya giren `<presence/>` stanzası yanıt olarak kabul edilmez; doğru `iq id` beklenir.
