# iksemel-rs — C iksemel Davranış Paritesi: Yol Haritası ve Mimari Kararlar

- **Tarih:** 2026-10-09
- **Durum:** Onay bekliyor (yol haritası). Alt proje A'nın detay spec'i bundan sonra yazılacak.
- **Referans commit'ler:** iksemel-rs `6658e41`, iksemel (C) `5abdbce`
- **Dayanak:** Kullanıcının iksemel-rs ↔ iksemel karşılaştırma raporu. Bu spec'teki her madde, ilgili kaynak okunarak bağımsız olarak doğrulanmıştır (kanıtlar §2'de).

---

## 1. Amaç ve başarı ölçütü

**Niyet (yorumlamam — yanlışsa düzeltin):** C `iksemel`'i referans/oracle kabul edip `iksemel-rs`'in ondan saptığı davranışları düzeltmek; sonuçta C kütüphanesini kullanan uygulamaların Rust sürümüne geçebilmesi.

**Başarı ölçütü:** Aşağıdaki üç şey aynı anda doğru olduğunda iş bitmiş sayılır.

1. Raporun "öncelikli doğruluk sorunları" listesindeki 8 maddenin tamamı kapanmış, her biri C davranışına karşı **differential test** ile kilitlenmiş; ayrıca bu inceleme sırasında bulunan 9. ve 10. madde (§2.1) de aynı şekilde kapatılmış.
2. Raporun "C API'sine geçiş açısından kalan eksikler" ve "modern özellikler" tablolarındaki kalemler ya uygulanmış ya da bilinçli sapma defterinde (§8) gerekçesiyle listelenmiş.
3. `cargo test --all-features` yeşil; differential suite yeşil; sapma defterindeki her kalemin bir testi var.

**Kapsam:** Kullanıcı kararı gereği **her şey** — doğruluk düzeltmeleri, DOM/API uyumu, entegrasyon API'leri, C ABI, Python binding, DIGEST-MD5, Stream Management entegrasyonu, `Send` dönüşümü, iksroster SCRAM, JID IDNA. Tek spec'e sığmadığı için §5'te yedi alt projeye bölünmüştür.

**Kapsam dışı:** C kütüphanesinin kendisinde değişiklik yapılması. C reposu yalnızca oracle'dır; orada kod değişmez.

---

## 2. Doğrulanmış kusur envanteri

Her satır kaynak okunarak doğrulandı. "Kanıt" kolonundaki konumlar rapor değil, **okunan kod**dur.

### 2.1 Öncelikli doğruluk sorunları

| # | Konu | Kanıt (Rust) | Doğrulanan davranış |
|---|---|---|---|
| 1 | Bölünmüş UTF-8 hatası | `net.rs:222`, `async_net.rs:75`, `async_net.rs:266` | Her soket okuması bağımsız `std::str::from_utf8(&buf[..n])` ile çözülüyor. Çok baytlı bir karakter iki okumaya bölünürse ikinci okuma `BadXml` verir. Repo genelinde bayt→`str` dönüşümü yapan **tam olarak üç** üretim yolu var (senkron `Connection::recv_event`, `AsyncConnection::recv_event`, `AsyncReceiver::recv_event`); `from_tcp_stream` ve `split`/`split_channels` bu üçüne bağlanır, ayrı dönüşüm noktası değildir. |
| 2 | DOM bitirme adımı yok | `dom.rs:94-101` | `parse_str` yalnızca `parser.parse(xml)?` çağırıp `document()` okuyor. Kök element kapanmamış olsa bile `Ok` dönüyor: `<r><c/>` → `Ok(<r><c/></r>)`. |
| 3a | Boşluk kaybı | `dom.rs:224`, `stream.rs:174` | `if !data.trim().is_empty()` — yalnızca boşluk içeren CDATA düğümü hiç oluşturulmuyor. `<r> </r>` → `<r/>`. |
| 3b | Bitişik metin birleştirilmiyor | `dom.rs:222-228` | DOM handler her `on_cdata` çağrısında **yeni** düğüm ekliyor. `stream.rs:165-172` birleştiriyor, `dom.rs` birleştirmiyor — iki handler tutarsız. |
| 3c | `find_cdata` semantiği | `lib.rs:385-393` | Rust çocuklar arasında **ilk CDATA düğümünü arıyor**; C `iks.c:451` yalnızca **ilk çocuğun** CDATA olmasını kabul ediyor (`IKS_CDATA != y->type` → NULL). Ayrıca C'de bitişik parçalar eklenti anında tek düğüme birleştiği için `ab`+` `+`cd` zinciri tek düğüm olur. |
| 3d | Pretty-print modu CDATA'yı kırpıyor | `writer.rs:103-105` | `XmlWriter` `pretty` modunda `text.trim()` yazıyor → CDATA'nın başındaki/sonundaki boşluk **serileştirmede kaybolur**. C'nin `iks_string`'i biçimlendirme yapmaz; boşluk her zaman korunur. |
| 4a | Değer düğümünde parent bağlanmıyor | `lib.rs:539-555`, `lib.rs:828-845` | `add_child` parent'ı `self.as_rc()` üzerinden kuruyor; `as_rc()` bir değer düğümünde (`self_ref` yok, `parent` yok) `None` döner. Yani değer olarak oluşturulmuş hiçbir kökte parent bağı kurulamaz. |
| 4b | `deep_clone` bağları kaybediyor | `lib.rs:847-863` | `deep_clone` değer döndürüyor ve çocukları yine değer biçimli `add_child` ile ekliyor → torunların parent'ı ve `self_ref`'i yok; klonun kendisi de Rc'siz. |
| 4c | Stream teslimi klonluyor | `stream.rs:151` | `root_rc.borrow().clone()` = `deep_clone()` → `StreamEvent::Stanza(IksNode)` değer olarak taşınıyor → alınan stanza çocuklarında parent yok, dolayısıyla namespace mirası ve sibling işlemleri çalışmıyor. |
| 5 | Attribute güncelleme/silme yok | `lib.rs:706`, `lib.rs:756` | `add_attribute` koşulsuz `push`; aynı ad iki kez eklenirse çift kayıt oluşur, `find_attrib` ilkini (eski değeri) döndürür. Silme API'si yok. |
| 6 | Filtre semantiği uyumsuz | `filter.rs:75-123` | `dispatch` eklenme sırasıyla **tüm** eşleşmeleri çalıştırıyor; handler'ın `bool` dönüşü yalnızca sayacı artırıyor, akışı kesmiyor. Kural/hook kaldırma API'si yok. |
| 7 | IQ yanıtı isteğe bağlanmıyor | `roster.rs:243-260`, `roster.rs:263-281`, `async_net.rs:419-450` | `fetch_roster`/`sync_roster`/`bind_resource_async` sıradaki stanzayı yanıt kabul ediyor; `id` karşılaştırması yok, tip kontrolü yalnızca `type == "result"`. Araya giren bir presence/message yanıt sanılabilir. |
| 8 | Token limiti eksik yollar | `parser.rs:683`, `parser.rs:705` | `max_token_size` yalnızca attribute değeri iki noktasında kontrol ediliyor. Metin, CDATA ve tag adı yollarında kontrol yok. |
| 9 | **Serileştirme kaçışları uyumsuz** *(raporda yok)* | `lib.rs:951-964`, `writer.rs:133/161`; ayrıca `utility.rs:107/140`, `parser.rs:48` | C tek bir `escape()` fonksiyonu kullanır (`iks.c:640-724`): hem metin hem attribute için `& < > ' "` kaçırılır **ve ASCII dışı her karakter `&#xNN;` sayısal referansına çevrilir**. Rust'ta **altı ayrı** kaçış fonksiyonu var (`lib.rs` ×2, `writer.rs` ×2, `utility.rs` ×1 çekirdek, `parser.rs` ×1) ve hiçbiri sayısal referans üretmiyor; üstelik metin yolları (`escape_text`, `write_escaped_text`) `'` ve `"` kaçırmıyor, yalnızca attribute yolları kaçırıyor. Oracle çıktısı: `<r>café ü</r>` → C `caf&#xe9; &#xfc;`, Rust `café ü`; `<r>"</r>` → C `&quot;`, Rust `"`. C tarafında ayrıca 2 baytlık maske hatası var — bkz. §3.2 ve D9. Bunlara ek olarak **iki ayrı bütçe fonksiyonu** daha var (`helper.rs:61` genel API, `parser.rs:26` özel kopya); ikisi de "her karakter 1 bayt" varsayar, yani sayısal referans semantiğine geçildiğinde **ikisi de yanlış olur** ve üretilen metinle bütçenin ayrışması C'deki taşma hatasının (D9) birebir Rust karşılığını doğurur. A5'te çekirdekle birlikte ele alınır. |
| 10 | **NUL baytı reddedilmiyor** *(raporda yok)* | `parser.rs:413-448` (CData), `parser.rs:661-710` (attribute) | C `sax_core` döngünün **en başında** `0x00`, `0xFE`, `0xFF` baytlarını `IKS_BADXML` ile reddeder (`sax.c:209`) — bağlamdan bağımsız, her yerde. Rust'ın bayt tarafları yalnızca `<`, `&`, `'`, `"` arar ve `\0`'ı olduğu gibi kabul eder: `DomParser::parse_str("<r>a\0b</r>")` `Ok` döner ve içerikte NUL taşır. (`0xFE`/`0xFF` zaten geçerli UTF-8 olmadığı için transport sınırında reddedilir; fark yalnızca `0x00` içindir.) |

### 2.2 Raporun "kalan eksikler" tablosunun doğrulanması

| Kalem | Rust durumu | Kanıt |
|---|---|---|
| `iks_hide`, attribute silme, CDATA sibling ekleme, `iks_set_cdata` | Yok. `set_content()` mevcut çocukları kaldırmıyor. | `lib.rs` API yüzeyi |
| `ikspak`, subtype/show sınıflandırması, message/presence/subscription builder'ları | Yok. | `lib.rs:63-77` export listesi |
| Özel transport ve TLS arayüzleri | `ConnectionStream` yalnızca `Plain`/`Tls`; `from_tcp_stream()` yalnızca senkron tarafta (`net.rs:111`) | `net.rs:23-26`, `async_net.rs:95-98` |
| Log callback, byte sayacı, ayrı stream-error olayı | Yok; yalnızca `set_log_traffic(bool)` ile stdout'a yazma. `StreamEvent` üç varyant: `StreamStart`/`Stanza`/`StreamEnd`. | `stream.rs:20-28` |
| MD5, durum tutan hash API'leri, DIGEST-MD5 | MD5 hiç yok. `crypto.rs` yalnızca base64/sha1/sha256/hmac/pbkdf2 sunuyor. Cargo'da md5 bağımlılığı yok. | `crypto.rs` (10 `pub fn`), `Cargo.toml` |
| C ABI + Python binding | Yok. `Cargo.toml`'da `crate-type` yok, `src/` genelinde `extern "C"` yok, `python/` dizini yok. | `Cargo.toml`, `grep` |
| Stream Management bağlantıya entegre değil | `StreamManagementState` (`xep.rs:960`) var ama ACK/resume/replay otomasyonu yok. | `xep.rs:958-1000` |
| Tokio `Send` sınırı | `AsyncReceiver` (`async_net.rs:45-50`) `StreamParser` + `ReadHalf` tutuyor; `StreamParser` → `StreamDispatcher` → `Vec<Rc<RefCell<IksNode>>>` → `!Send`. | `async_net.rs:45`, `stream.rs:34` |
| `iksroster` SCRAM kullanmıyor | `args.plain` false iken sabit `authenticate_plain` çağrılıyor; `--sasl` seçimi akışı değiştirmiyor; `--secure` `set_allow_insecure_tls(true)` yapıyor. | `tools/iksroster.rs:59-95` |
| JID normalizasyonu sınırlı | Yalnızca `to_ascii_lowercase()` + temel karakter kontrolü; Unicode/IDNA yok. | `jid.rs:67-96` |

---

## 3. C referans semantiği (oracle)

Bu bölüm, düzeltmelerin hedefini tanımlar. Hepsi C kaynağından okunmuştur; bir kısmı raporun çerçevelemesini **düzeltir**.

**Doğrulama yöntemi:** Aşağıdaki maddeler yalnızca okunarak değil, **çalıştırılarak** da doğrulanmıştır. C kütüphanesinin saf XML alt kümesi meson ve `config.h` olmadan derlenip bir oracle programı çalıştırılmıştır (spike, `/tmp` altında, repoya dokunulmadan):

```sh
cc -o oracle oracle.c -I<iksemel>/include \
   -DSTDC_HEADERS -DHAVE_STRING_H -DHAVE_ERRNO_H -DHAVE_UNISTD_H \
   <iksemel>/src/{iks,dom,sax,ikstack,utility,filter,base64}.c
```

`HAVE_CONFIG_H` tanımlanmadığı için `config.h` gerekmez (`common.h` koşullu içe aktarır). TLS, ağ ve `io-posix` harness'a dahil değildir. **Sonuç:** §7'deki differential harness fizibildir ve meson gerektirmez; kullanıcının "9 C testini elle derledim" ifadesiyle de tutarlıdır.

### 3.1 SAX/transport

- `sax_core` **bayt yönelimlidir** ve UTF-8 durumunu çağrılar arasında taşır: `prs->uni_max`, `uni_len`, `uni_char` (`sax.c:210-253`). Bölünmüş çok baytlı karakter bu yüzden sorun olmaz. **Sonuç:** 1. maddenin düzeltmesi transport sınırındadır (bayt → `str`), parser'ın içinde değil.
- C UTF-8'i doğrular: overlong diziler reddedilir (`sax.c:217-226`), `0x00`/`0xFE`/`0xFF` reddedilir (`sax.c:209`). **Ama** 5 ve 6 baytlık eski (RFC 2279) dizileri kabul eder (`sax.c:243-248`) — bunlar Rust `str` ile temsil edilemez.
- `iks_parse(prs, data, len, finish)` — **`finish` parametresini hiç kullanmaz** (`sax.c:638-643`). Yalnızca `sax_core`'a delege eder.

### 3.2 DOM ve ağaç

- `iks_insert_cdata(x, data, len)` (`iks.c:110-130`): son çocuk CDATA ise **ona ekler** (strcat), değilse yeni CDATA düğümü yaratır. Boşluk asla düşürülmez, hiçbir budama yapılmaz. **Oracle ile doğrulandı:** `"<w><r>ab"` + `" "` + `"cd</r></w>"` üç ayrı `iks_parse` çağrısıyla verildiğinde sonuç `<w><r>ab cd</r></w>` ve `<r>`'nin **tek** çocuğu var — yani birleştirme *eklenti anında*, sorgu anında değil.
- `iks_find_cdata(x, name)` (`iks.c:451-460`): `name` adlı çocuğu bulur, sonra **onun ilk çocuğu** CDATA değilse `NULL` döner — çocuklar arasında CDATA *aramaz*. **Oracle ile doğrulandı:** `<w><r><b/>cd</r></w>` için `find_cdata(w,"r")` → `NULL`. Rust `lib.rs:385` çocuklar arasında aradığı için burada `"cd"` dönerdi — gerçek bir sapma.
  - **Raporun çerçevelemesi burada düzeltilir:** `find_cdata` "tüm CDATA parçalarını birleştirmeli" değil; birleştirme eklenti anında olmalı, `find_cdata` ise ilk-çocuk kuralına uymalı.
- `iks_set_cdata(x, data, len)` (`iks.c:211-226`): **tüm çocukları `iks_hide` ile ayırır**, sonra CDATA ekler. **Oracle ile doğrulandı:** 2 çocuklu `<z>` → `set_cdata` sonrası 1 çocuk, `<z>txt</z>`.
- `iks_insert_attrib(x, name, value)` (`iks.c:133-173`): ada göre arar. Bulursa ve `value != NULL` ise **değeri günceller**; bulursa ve `value == NULL` ise **attribute'u listeden çıkarır**; bulamazsa ve `value != NULL` ise sona ekler. Ayrı bir `iks_delete_attrib` **yoktur**. **Oracle ile doğrulandı:** aynı ada iki kez ekleme → attribute sayısı 1, `find_attrib` yeni değeri döner, serileştirme `<r id="new"/>`; `NULL` değerle çağrı → attribute listeden çıkar, `<r/>`.
- `iks_insert(x, name)` (`iks.c:91-107`): yeni düğümü `y->parent = x` ile bağlar; `parent`/`next`/`prev` **gerçek işaretçilerdir**, `IKS_TAG_LAST_CHILD` kuyruk önbelleği tutulur.
- `iks_hide(x)` (`iks.c:329-343`): düğümü kardeş listesinden ve ebeveynin `children`/`last_child` işaretçilerinden çıkarır; belleği **serbest bırakmaz** (arena sahipli).
- **Serileştirme (`iks_string` → `escape`, `iks.c:640-724`).** Tek fonksiyon, hem metin hem attribute değeri için:
  - `&` `<` `>` `'` `"` → sırasıyla `&amp;` `&lt;` `&gt;` `&apos;` `&quot;` (yani metinde `'` ve `"` **de** kaçırılır);
  - ASCII dışı (2/3/4 baytlık UTF-8) → `&#xNN;` sayısal referansı, lowercase hex, `%02x` biçimi;
  - `0x80`–`0x9F` aralığı ve `0x00` **sessizce atılır** (satır 667-669, 702); diğer yazdırılamaz ASCII (`< 0x20`) sayısal referansa çevrilir.
  - **C'de doğrulanmış bir maske hatası var.** 2 baytlık dalın koşulu `(*ptr & 0xE8) == 0xC0` (satır 671); doğrusu `0xE0` olmalıydı. Maske bit 4'ü istediği için yalnız `0xC0`–`0xC7` ve `0xD0`–`0xD7` eşleşir; `0xC8`–`0xCF` ve `0xD8`–`0xDF` ile başlayan karakterler `else` dalına (satır 694-697) düşer ve orada `char c` **işaret genişletmesiyle** `%02x`'e verilir. Sonuç, geri çözülemeyen çöp: `ε` (U+03B5, `CE B5`) → `&#xffffffce;&#xffffffb5;`. **Oracle ile doğrulandı:**

    | Girdi | Kod noktası | C çıktısı |
    |---|---|---|
    | `é` (`C3 A9`) | U+00E9 | `&#xe9;` ✓ |
    | `А` (`D0 90`) | U+0410 | `&#x410;` ✓ |
    | `ε` (`CE B5`) | U+03B5 | `&#xffffffce;&#xffffffb5;` ✗ |
    | `Ȁ` (`C8 80`) | U+0200 | `&#xffffffc8;` (0x80 ayrıca düşer) ✗ |
    | `€` (`E2 82 AC`) | U+20AC | `&#x20ac;` ✓ |
    | `😀` (`F0 9F 98 80`) | U+1F600 | `&#x1f600;` ✓ |

    Yani bozulma **U+0200–U+03FF** ve **U+0600–U+07FF** aralığındadır (Yunanca, Arapça, NKo…). Rust bu çöpü **kopyalamaz**; doğru kod noktasını üretir (bkz. D9).
  - **Bunun sonucu bir bellek güvenliği hatasıdır ve ASAN ile doğrulandı.** `escape_size` (`iks.c:573-630`) 2 baytlık dal için `2*1+4 = 6` bayt bütçeler, ama `escape` (`iks.c:673`) 3 haneli kod noktaları için 7 bayt yazar (`&#x410;` = 7). Bu yüzden **U+0100–U+01FF ve U+0400–U+05FF** girdilerinde bütçe 1 bayt eksik kalır (metnin kendisi doğru olduğu için bu aralıklarda Rust C ile birebir aynıdır; yalnızca taşma kopyalanmaz). Maske hatasına düşen girdilerde ise `else` dalı 6 bayt bütçelerken `escape` (`iks.c:695`) 12 bayt yazar (`&#xffffffce;`). `iks_string` tamponu bu bütçeyle ayırdığı için (`iks.c:789`, `iks.c:821`), `iks_string(NULL, x)` çağrısı ASAN altında **heap-buffer-overflow** veriyor:

    ```
    WRITE of size 12 at 0x...aa
      #1 my_strcat iks.c:636
      #2 escape    iks.c:695
      #3 iks_string iks.c:821
    0 bytes after 26-byte region
    ```
    Taşma, `ikstack` ile çağrıldığında arena bloğunun içinde kaldığı için ASAN'a görünmez ama yine de gerçek bir taşmadır (bu yüzden ilk spike temiz geçmişti). **Harness kuralı:** oracle, `iks_string(NULL, ...)` yerine daima bir `ikstack` kullanır; ayrıca non-ASCII girdilerde C çıktısı güvenilmez kabul edilir.
  - Oracle doğrulaması: `café ü` → `caf&#xe9; &#xfc;`; attribute `a="café"` → `a="caf&#xe9;"`; metinde `"` → `&quot;`; CDATA'da `'` → `&apos;`; `0x01` (attribute) → `&#x01;`; `0x7f` → `&#x7f;`; `\t`/`\n`/`\r` (`is_print` bunları yazdırılabilir sayar, `iks.c:568-571`) **birebir** geçer, sayısal referansa çevrilmez. **C1 kontrolleri korunur:** `U+0080` (`C2 80`) → `&#x80;`, `U+009F` (`C2 9F`) → `&#x9f;` — C'nin `0x80`–`0x9F` düşürmesi yalnızca ham baytlar içindir ve bu baytlar geçerli UTF-8'de tek başına bulunamaz (D8). `U+0000` düşürülür (`a\0b` → `ab`).
- DOM tamamlanma garantisi `dom.c`'deki `tagHook`'tan gelir (`dom.c:16-55`): kök düğüm `*iksptr`'ye **yalnızca kök element kapandığında** yazılır. Dolayısıyla `iks_tree("<r><c/>", 0, &err)` → `NULL` döner ve `err` **`IKS_OK`** olur (hata bildirilmez). Bu, C'nin kendi tutarsızlığıdır; sapma defterine girer. **Oracle ile doğrulandı:** `tree=NULL err=0`; tam belge `<r><c/></r>` için `err=0`, serileştirme `<r><c/></r>`.

### 3.3 Filtre (`filter.c`)

Puanlama (`filter.c:115-167`):

| Ölçüt | Ağırlık | Eşleşmezse |
|---|---|---|
| `IKS_RULE_TYPE` | 1 | tüm skor 0 |
| `IKS_RULE_SUBTYPE` | 2 | tüm skor 0 |
| `IKS_RULE_NS` | 4 | tüm skor 0 |
| `IKS_RULE_FROM` (tam JID) | 8 | tüm skor 0 |
| `IKS_RULE_FROM_PARTIAL` (bare JID) | 8 | tüm skor 0 |
| `IKS_RULE_ID` | 16 | tüm skor 0 |

- Herhangi bir ölçüt eşleşmezse `fail = 1` → skor 0 (kısmi puan yok).
- **En yüksek skorlu kural önce** çalışır; sonuç `IKS_FILTER_EAT` ise `iks_filter_packet` **hemen döner** ve kalan kurallar çalışmaz.
- EAT değilse o kuralın skoru 0'lanır ve kalanlar arasından yeniden en yüksek seçilir; skoru > 0 kural kalmayınca döngü biter.
- Başlangıç `max_score = 0` ve karşılaştırma `score > max_score` olduğundan, **hiç ölçütü olmayan bir kural asla çalışmaz** (skoru 0).
- `iks_filter_remove_rule` ve `iks_filter_remove_hook` (`filter.c:92-113`) kural/hook kaldırmayı destekler.
- `iks_strcmp` NULL-güvenlidir (`utility.c`), yani id'siz pakete karşı id kuralı çökme değil **eşleşmeme** üretir.

### 3.4 Paket sınıflandırması (`jabber.c:68-159`)

- `pak->type`: `message` / `presence` / `iq`; diğer adlar için 0 (hiçbiri).
- `message` alt tipi: `chat`/`groupchat`/`headline`/`error`; yok ise 0.
- `presence`: `type` yoksa `IKS_PAK_PRESENCE` + `IKS_TYPE_AVAILABLE` + `<show>` içeriğinden `show` (`chat`/`away`/`xa`/`dnd`, varsayılan `available`). `type` varsa `unavailable`/`probe` → `IKS_PAK_PRESENCE`, diğerleri (`subscribe`/`subscribed`/`unsubscribe`/`unsubscribed`/`error`) → `IKS_PAK_S10N`. Dikkat: `pak->type` başlangıçta `IKS_PAK_S10N`, `probe` bunu `PRESENCE`'a çevirir.
- `iq` alt tipi: `get`/`set`/`result`/`error`.
- **`pak->ns` yalnızca `iq` için ve xmlns niteliği taşıyan ilk tag çocuğunun `xmlns`'i** olarak doldurulur (`jabber.c:146-155`). Diğer stanza tiplerinde `ns` boş kalır → `NS` kuralı eşleşmez.
- `pak->from`: `iks_id_new` ile `full` ve `partial` (bare) alanlarına ayrılır (`jabber.c:80-81`).

### 3.5 C ABI yüzeyi

- `include/iksemel.h` ~123 public sembol; `struct iks_struct` **yalnızca ileri bildirim** (`iksemel.h:54-55`), yani yapı düzeni gizli → opak tutamaç yaklaşımı ABI için mümkün.
- Yüzey: arena (`ikstack_*`), bellek kancaları (`iks_set_mem_funcs`), yardımcılar (`iks_strdup` vb.), DOM (`iks_*`), SAX (`iks_sax_new`/`iks_sax_extend`), filtre, paket, JID, **durum tutan** `iksha`/`iksmd5`, stream/IO (`iks_stream_new`, `iks_connect_*`, async olay yapısı), TLS, Python binding (`python/pyiks.c`).

---

## 4. Mimari kararlar (iki çatal)

### Karar 1 — Düğüm modeli: `NodeRef` sarmalayıcı, Rc zorunlu *(onaylandı)*

**Kök neden.** Ağacın sahibi ya `Rc`'dir ya değerdir; bu belirsizlik 4. maddenin kök nedenidir. Bir `IksNode` değeri için `parent` alanı *hiçbir zaman* geçerli bir Rc'ye işaret edemez — ortada Rc yoktur. Bu yüzden:
- `add_child(&mut self, child: IksNode)` değer köklerde parent bağlayamaz (düzeltilemez, tasarım gereği),
- `deep_clone(&self) -> Self` aynı çukura düşer,
- `StreamEvent::Stanza(IksNode)` teslim anında bağları koparır.

**Karar.** Ağaç sahipliği `NodeRef` üzerinden zorunlu kılınır:

```rust
pub struct NodeRef(Rc<RefCell<IksNode>>);

impl NodeRef {
    pub fn new_tag(name: impl Into<String>) -> Self;
    pub fn new(node_type: IksType) -> Self;
    pub fn add_child(&self, child: impl Into<NodeRef>) -> NodeRef;   // parent'ı her zaman bağlar
    pub fn clone_subtree(&self) -> NodeRef;                            // klon da bağlıdır
    // Ergonomi için yönlendiriciler (çağrı yerleri şekil değiştirmez):
    pub fn add_attribute(&self, k: impl Into<String>, v: impl Into<String>);
    pub fn set_content(&self, c: impl Into<String>);
    pub fn insert_cdata(&self, c: impl Into<String>) -> NodeRef;
    // ...
    pub fn borrow(&self) -> Ref<'_, IksNode>;
    pub fn borrow_mut(&self) -> RefMut<'_, IksNode>;
}
```

- `IksNode` **iç veri tipi** olarak kalır (alanları ve salt-okunur sorgularıyla); `IksNode::add_child` kaldırılır veya `#[deprecated]` işaretlenir.
- `IksNode::into_rc()` yerini `NodeRef::from(node)` / `From<IksNode> for NodeRef` alır.
- `StreamEvent::Stanza` artık `NodeRef` taşır (kırıcı ama doğru).
- **Neden bu:** `add_child` çağrı biçimi aynı kalır (`x.add_child(c)`), çoğu kurulum yeri yalnızca `IksNode::new_tag` → `NodeRef::new_tag` değişimi ister; `add_attribute`/`set_content`/`insert_cdata` yönlendiricileri sayesinde attribute ekleme satırları da değişmez. Derleyici kalan tüm yerleri gösterir.
- **Ölçülen maliyet (sayıldı):** 102 değer-biçimli `add_child` çağrısı, 15 dosyada — `src/` 9 dosya 84 çağrı (dom 3, stream 3, xep 42, roster 5, filter 1, sasl 11, writer 2, lib 14, async_net 3), `tests/` 4 dosya 13 çağrı, `examples/` 2 dosya 5 çağrı. Mekanik ve derleyici güdümlü.
- **Neden arena değil:** Arena + indeks modeli (F için opak tutamaç, G için `Arc` paylaşımı) daha güçlü olurdu, ancak A'dan G'ye her şeyi yeniden yazar ve F/G gereksinimleri henüz somutlaşmadan spekülatiftir. `NodeRef`, tüm kurulum/ekleme işlemlerini tek bir dikişten geçirdiği için ileride arena'ya geçişi de kolaylaştırır. (YAGNI.)
- **F ve G'yi bağlamıyor.** F opak tutamaç kullanabilir (C başlığı yapıyı açmıyor) ve G'nin `Send` ihtiyacı `Rc<RefCell>` zorunlu kılmadan `LocalSet`/adanmış iş parçacığı + kanal ile çözülebilir — C kütüphanesi de iş parçacığı güvenli değildir, yani parite bozulmaz.

### Karar 2 — C uyum sıkılığı: Rust idiomatik + ABI'de birebir *(onaylandı)*

- **Rust API'si:** ağaç, metin, serileştirme, filtre yönlendirme sırası ve protokol çıktısı C ile **birebir**; hata sinyali Rust idiomuna çevrilir (`Err(IksError::BadXml)`, `Option<&str>`).
- **C ABI katmanı (F):** sınırda C davranışı **aynen** geri konur (ör. `NULL` + `IKS_OK`), böylece C tüketicileri için uyumluluk kaybedilmez.
- Her bilinçli sapma §8'deki deftere yazılır ve **her biri için bir test** bulunur.

---

## 5. Alt projeler

Her alt proje ayrı bir spec + plan + uygulama döngüsü alır. Aşağıdaki kabul kriterleri o alt projenin spec'ine girdi olur.

### A — Parser, transport ve serileştirme doğruluğu *(rapor 1, 2, 3, 8 + 9, 10)*

- **İş:**
  - Artımlı UTF-8 çözücü: bayt→`str` sınırında tamamlanmamış kuyruk baytlarını (en fazla 3) tamponla; yalnızca tam karakterleri `parse_chunk`'a ver. Üç dönüşüm noktası: `net.rs::Connection::recv_event` (222), `async_net.rs::AsyncConnection::recv_event` (266), `async_net.rs::AsyncReceiver::recv_event` (75). Ortak bir `Utf8Carry` yardımcısı olarak tek yerde. **Belirsizlik giderildi:** kuyruk yalnızca `std::str::from_utf8`'in `error_len() == None` (yani "girdi erken bitti") dediği durumda tamponlanır; `error_len() == Some(_)` ise girdi hiçbir uzatmada geçerli olmayacaktır ve derhal `BadXml` üretilir. Böylece bayt tamponlaması C'nin reddettiği girdiyi asla kabul etmez.
  - DOM bitirme: `Parser::finish()` (kök kapandı mı) ekle; `parse_str`/`parse_str_with_limits`/`load_file` parse + finish yapmalı. Bitmemiş belge `Err(IksError::BadXml)` verir (bkz. D1).
  - Metin doğruluğu: `dom.rs::on_cdata` `stream.rs::on_cdata` gibi son çocuk CDATA ise **eklemeli**; boşluk düşürme kaldırılmalı. `find_cdata` C semantiğine çekilmeli (ilk çocuk CDATA değilse `None`). `writer.rs:103-105`'teki pretty-print `trim()` kaldırılmalı — C'de boşluk serileştirmede hiç kırpılmaz.
  - Token limiti: `max_token_size` kontrolü metin, CDATA ve tag adı yollarına da uygulanmalı.
  - Yasak bayt (madde 10): `0x00` baytı parser'ın **bayt tarama düzeyinde** reddedilmeli — metin, CDATA, tag adı ve attribute değeri dahil her bağlamda. C bunu `sax_core`'un döngü başında yapar (`sax.c:209`, `0x00`/`0xFE`/`0xFF`). Rust yalnızca `<`, `&`, `'`, `"` aradığı için `\0` sessizce içerikte taşınıyor. **Oracle ile doğrulandı:** `iks_parse(p, "<r>a\0b</r>", 9, 1)` → `IKS_BADXML`; attribute de aynı. Bu düzeltme D8'in "`0x00` zaten parser'a hiç ulaşmaz" varsayımını gerçek kılar.
  - Serileştirme (satır 9): altı ayrı kaçış fonksiyonu (`lib.rs:951` `escape_attr`, `lib.rs:960` `escape_text`, `writer.rs:133` `write_escaped_attr`, `writer.rs:161` `write_escaped_text`, `utility.rs:107` `escape_cow`/`:140` `escape`, `parser.rs:48` `escape`) tek bir ortak çekirdeğe indirilmeli ve §3.2'deki C `escape()` semantiğine çekilmeli — `'`/`"` her iki bağlamda, ASCII dışı **sayısal referans** (`&#xNN;`, lowercase hex, `%02x`). `writer.rs`'in ayırıcı avantajı (ara `String` ayırmadan doğrudan `Write`'a yazma) korunur; ortak çekirdek bu iki kullanım biçimini de (döndüren ve akıtan) besleyecek şekilde tasarlanır. Bu, §7'deki differential suite'in bayt-birebir karşılaştırma yapabilmesinin **ön koşuludur**; A'da yapılmazsa sonraki tüm karşılaştırmalar gürültülü olur. Sessiz veri kaybı yalnızca `U+0000` için geçerlidir ve orada Rust C ile **aynıdır** (D8); C'nin `0x80`–`0x9F` düşürmesi geçersiz UTF-8 gerektirdiği için `&str` ile erişilemez. Sayısal referans üretimi birebir uygulanır: lowercase hex, `%02x` yani **en az iki hane** (`&#x80;`, `&#xe9;`, `&#x3b5;`, `&#x1f600;`), ondalık değil.
- **Dokunulan:** `src/parser.rs`, `src/dom.rs`, `src/stream.rs`, `src/net.rs`, `src/async_net.rs`, `src/writer.rs`, `src/lib.rs` (`find_cdata`, `escape_text`, `escape_attr`), `src/utility.rs`.
- **Kabul:** §7'deki differential suite'te 1/2/3/8/9 numaralı senaryolar C oracle ile aynı sonucu vermeli; bölünmüş UTF-8 için üç dönüşüm noktasının her birinde bir regresyon testi; `<r> </r>` ve `ab`+` `+`cd` senaryoları; `max_token_size: 4` ile 10 baytlık metin/CDATA/uzun tag adının **reddedilmesi**; `<r>café ü</r>` serileştirmesinin oracle'ın `caf&#xe9; &#xfc;` çıktısıyla eşleşmesi; metinde `"`/`'` ve attribute'ta aynı karakterlerin kaçırılması; `XmlWriter::set_pretty(true, _)` ile yazılan `<r> ab </r>`'ın boşluklarını koruması; `error_len() == Some(_)` veren baytın tamponlanmadan hata vermesi; `<r>a\0b</r>` ile `<r x="a\0b"/>`'nin **reddedilmesi** (C oracle `IKS_BADXML` ile aynı); D9 aralığı için Rust'ın doğru kod noktasını ürettiğini ve C'nin çöp ürettiğini kilitleyen ayrı bir test.

### B — Düğüm modeli & DOM düzenleme API'leri *(rapor 4, 5 + eksikler)*

- **İş:** `NodeRef` girişi (§4 Karar 1); `deep_clone` yerine `clone_subtree`; stream teslimi bağlı ağaç versin. Attribute: `add_attribute` **upsert** olsun, `remove_attribute` eklensin (C'nin `NULL` değeri yerine ayrı metod; ABI'de C şekli geri konur). Eksikler: `NodeRef::hide()`, `set_cdata`/`set_content`'ün **çocukları kaldırması**, `append_cdata`/`prepend_cdata` (CDATA sibling), `insert_node`.
- **Kabul:** değer kökler dahil her yolda `parent()` dolu; `deep_clone` sonrası torunların parent'ı dolu; `add_attribute("id","a")` sonra `("id","b")` → tek kayıt, `"b"`; `remove_attribute` sonrası `None`; `set_content` sonrası eski çocuk yok.

### C — Sınıflandırma, filtre, IQ yönlendirme *(rapor 6, 7 + ikspak)*

- **İş:** `ikspak` eşdeğeri: `StanzaType` + alt tip + `show` + `ns` + full/bare JID ayrımı, §3.4'teki C tablosuna birebir. `PacketFilter`: §3.3'teki ağırlıklı puanlama, en yüksek skor önce, `Eat` sonucu akışı keser, `remove_rule`/`remove_hook`, ölçütsüz kural hiç çalışmaz. IQ yardımcıları: `fetch_roster`, `sync_roster`, `bind_resource(_async)` ve SASL akışları yanıtı **`id` ile** eşleştirsin; ilgisiz stanza atılsın.
- **Kabul:** sıralı/puanlı dispatch testleri (ID=16 > FROM=8 > NS=4 > SUBTYPE=2 > TYPE=1); EAT sonrası kalan handler'ın çalışmadığı; araya giren presence'ın yanıt sanılmadığı; `iq` dışı stanza'da NS kuralının eşleşmediği.

### D — Entegrasyon API'leri

- **İş:** message/presence/subscription builder'ları; log callback (`set_log_traffic` yerine/yanına kullanıcı geri çağrısı); byte sayacı; ayrı stream-error olayı (`StreamEvent::Error`); özel transport/TLS backend arayüzleri (trait) ve `from_tcp_stream`'ün async karşılığı; sertifika doğrulama kontrolünün API'ye taşınması. **Sınır:** D kütüphane API'sini sağlar; bunu `iksroster`'ın `--secure` bayrağına bağlamak G'nin işidir.
- **Kabul:** transport trait'i ile sahte (in-memory) transport üzerinden uçtan uca stream testi; log callback'in yön ve bayt sayısı doğruluğu; stream-error'ın `NetDropped`'dan ayrı raporlanması.

### E — Kripto & eski SASL

- **İş:** MD5 (`md-5` crate), durum tutan `iksha`/`iksmd5` eşdeğerleri, DIGEST-MD5 mekanizması.
- **Kabul:** RFC 1321 ve RFC 2831 test vektörleri; C `tst-md5.c` çıktılarıyla eşleşme; DIGEST-MD5 el sıkışması bir mock sunucuya karşı.

### F — C ABI + Python binding

- **İş:** `crate-type = ["rlib", "cdylib", "staticlib"]`; `extern "C"` yüzeyi (§3.5) opak tutamaçla; `iks_set_mem_funcs` gibi Rust'ta dürüstçe karşılanamayan semboller için belgelenmiş davranış (sapma defteri). `forbid(unsafe_code)` bu katman için gevşetilmeli (ayrı crate veya `deny` + modül istisnası). `python/pyiks.c` karşılığı.
- **Ön koşul:** A–E kararlı Rust API. Bu yüzden en sonda.
- **Kabul:** C başlığıyla derlenen bir C tüketici programı; `tst-*.c` testlerinin Rust kütüphanesine karşı derlenip geçmesi; **ABI sınırında** sapma defterindeki C davranışlarının birebir üretilmesi.

### G — Modern katman

- **İş:** Stream Management entegrasyonu (otomatik ACK, `resume`, yeniden bağlanma, tekrar gönderim); async katmanın `Send` kullanılabilirliği (`LocalSet`/adanmış iş parçacığı + kanal); `iksroster`'ın `args.plain`/`--sasl` ile SCRAM seçmesi ve `--secure` bayrağının D'de eklenen sertifika doğrulama API'sine bağlanması; JID IDNA/Unicode normalizasyonu.
- **Kabul:** ACK/resume senaryoları bir mock SM sunucusuna karşı; async API'nin gerçek bir çok-iş parçacıklı tokio runtime'ında kullanılabildiğini gösteren test; IDNA test vektörleri.

---

## 6. Sıra ve bağımlılık gerekçesi

```
A → B → C → D → E → F → G
```

- **A önce:** kullanıcının 1. önceliği ve node modeline bağımsız. A'nın `on_cdata` birleştirmesi, hâlihazırda **hiç çağrılmayan** ama doğru çalışan `IksNode::add_child_node(&Rc<RefCell<IksNode>>, IksNode)` yardımcısını (`lib.rs:645-657`; parent/`prev`/`next` bağlarını kurar, repo genelinde tek geçişi kendi tanımıdır) kullanır; böylece B'nin imza değişikliğini beklemez. A'nın B'ye devrettiği tek yüzey `deep_clone`/`add_child` düzeltmeleridir.
- **B, C'den önce:** 4. maddenin çözümü `add_child`'ın imzasını değiştirir; C/D/E/F/G'deki tüm kurulum yerleri düzeltilmiş modelin üstüne yazılsın diye erken gelir.
- **`ikspak` C'de (D'de değil):** C'nin filtre puanlaması `pak->ns`/`pak->subtype`/`pak->from->full|partial` üzerinden çalışır — yani sınıflandırma, filtre uyumunun **ön koşuludur**.
- **E bağımsız:** MD5/DIGEST-MD5 başka hiçbir alt projeye bağlı değil; istenirse paralel yürüyebilir.
- **F en sonda:** ABI, kararlı bir Rust API üzerine yazılan bir eşleme katmanıdır; erken yazılırsa iki kez yazılır.
- **G en sonda:** `Send` çalışması B'nin node modeline, transport arayüzleri D'ye, SM entegrasyonu A'nın okuma yollarına dayanır.

---

## 7. Doğrulama stratejisi

**Differential test (birincil).** C kütüphanesi oracle olarak kullanılır:

- **Derleme (doğrulandı):** meson bu makinede **kurulu değil** ve gerekmiyor. Saf XML alt kümesi (`iks.c dom.c sax.c filter.c ikstack.c utility.c jabber.c base64.c sha.c md5.c`) düz `cc` ile derlenir; §3'teki reçete ile fiilen derlenip çalıştırılmıştır. `HAVE_CONFIG_H` tanımlanmadığı için el yazımı `config.h` **gerekmez** (`common.h` onu koşullu içe aktarır). TLS, ağ ve `io-posix` kaynakları harness'a **dahil edilmez**.
- **Oracle programı:** stdin'den senaryo okuyup kanonik çıktı üreten tek bir C CLI (ağaç dökümü, `find_cdata` sonucu, attribute durumu, `iks_filter_packet` çağrı izi, `ikspak` alanları).
- **Rust tarafı:** aynı senaryoları işleyen bir Rust ikizi.
- **Sürücü:** paylaşılan bir senaryo korpusu üzerinde ikisini çalıştırıp diff alan bir Rust integration testi. Sapma defterindeki kalemler ayrı bir kategoride, "beklenen fark" olarak kilitlenir. **Kaçış karşılaştırmasında istisna:** Yalnızca U+0200–U+03FF ve U+0600–U+07FF kod noktalarını içeren senaryolar bayt-birebir diff'e girmez; bunun yerine "Rust doğru kod noktasını üretir, C çöp üretir" beklentisi ayrı bir testle kilitlenir (D9). `U+0000` senaryosu ise A'dan sonra parser'da reddedildiği için serileştirmeye hiç ulaşmaz; "her iki taraf da hata verir" olarak kilitlenir. Bunların dışında **hiçbir istisna yoktur** — özellikle U+0080–U+009F artık istisna değildir: oracle, C'nin C1 kontrollerini `&#x80;`–`&#x9f;` olarak koruduğunu gösterdi (D8), dolayısıyla diff zorunludur.
- **C'nin mevcut testleri:** `tst-dom.c`, `tst-filter.c`, `tst-iks-utf8.c`, `tst-iks.c`, `tst-sax.c`, `tst-jid.c` doğrudan oracle olarak kullanılır.

**Diğer katmanlar:** her alt projenin kendi unit/integration testleri; `#![forbid(unsafe_code)]`'un F'ye kadar korunması; `cargo clippy` ve mevcut CI matrisi (Windows/Schannel dahil) yeşil kalmalı; bellek davranışı için `cargo miri` (A ve B'de özellikle değerli).

---

## 8. Bilinçli sapma defteri

C'den **kasıtlı** olarak ayrıldığımız noktalar. Her kalem bir testle kilitlenir; F katmanı bunları sınırda geri koyar.

| # | Konu | C davranışı | Rust davranışı | Gerekçe |
|---|---|---|---|---|
| D1 | Tamamlanmamış belge | `iks_tree("<r><c/>")` → `NULL`, `err=IKS_OK` | `Err(IksError::BadXml)` | C hatayı bildirmiyor; Rust `Result` idiomu. F'de C şekli geri konur. |
| D2 | Attribute silme | `iks_insert_attrib(x, name, NULL)` | `remove_attribute(name) -> Option<String>` | Aynı fonksiyona aşırı yüklemek yerine ayrı, açık API. F'de `NULL` yolu geri konur. |
| D3 | 5/6 baytlık UTF-8 | Kabul edilir (`sax.c:243-248`) | Reddedilir | `str` bu dizileri temsil edemez; RFC 2279 geçersizdir. |
| D4 | `finish` parametresi | Yok sayılır (`sax.c:638`) | `Parser::finish()` gerçek doğrulama yapar | Amacı yerine getirilmiş olur; C'de ölü parametredir. |
| D5 | Filtre `ns` kapsamı | Yalnız `iq` + `xmlns` taşıyan ilk tag çocuğun `xmlns`'i | **C ile aynı** (sapma değil, düzeltme) | Rapordaki "her çocuğa bak" davranışı C'den sapıyordu; C'ye çekilir. |
| D6 | `iks_set_mem_funcs` | Global ayırıcı kancası | Karşılığı yok; belgelenir | Rust'ta global ayırıcıyı güvenle değiştirmek mümkün değil. |
| D7 | `find_cdata` çok-çocuk | İlk çocuk CDATA değilse `NULL` | **C ile aynı** (sapma değil, düzeltme) | Rapor "hepsini birleştir" diye çerçeveliyordu; doğrusu eklenti anında birleştirme + ilk-çocuk kuralı. |
| D8 | `escape()` veri kaybı | **Ham bayt** `0x00` ve `0x80`–`0x9F` düşürülür (`iks.c:667-669`, `iks.c:702`); düzgün UTF-8 kodlanmış U+0080–U+009F ise **korunur** (`C2 80` → `&#x80;`, oracle ile doğrulandı, `iks.c:673`) | Aynı: `U+0000` düşürülür, `U+0080`–`U+009F` → `&#x80;`–`&#x9f;` | **Sapma yok.** C'nin `0x80`–`0x9F` düşürmesi **geçersiz UTF-8** girdi gerektirir (bu baytlar ancak çok baytlı bir dizinin devamı ya da geçersiz bir lead olarak geçerli metinde bulunur); Rust `&str` böyle bir girdiyi zaten temsil edemez. Geriye kalan tek kayıp `U+0000`'dır ve orada iki taraf da aynıdır. (Bu satır ilk yazıldığında C'nin C1 kontrollerini de düşürdüğü sanılmıştı; oracle bunu yanlışladı. Ayrıca `escape_size` `0x00` için 6 bayt bütçe ayırır ama `escape` 0 bayt yazar — bu fazla bütçedir, taşma değil.) |
| D9 | `escape()` 2 baytlık maske hatası ve bütçe taşması | `(*ptr & 0xE8) == 0xC0` (`iks.c:671`) yüzünden `0xC8`–`0xCF` ve `0xD8`–`0xDF` lead'leri işaret genişletmeli çöp üretir (`ε` → `&#xffffffce;`); ayrıca `escape_size` bu girdilerde bütçeyi eksik hesaplayıp **heap-buffer-overflow**'a yol açar (ASAN ile doğrulandı) | Her zaman doğru kod noktası üretilir: `ε` → `&#x3b5;` | C'nin çıktısı geçerli bir karakter referansı değil, geri çözülemez çöptür ve üretimi bellek güvenliği ihlalidir; kopyalanamaz. Kapsam: bozulma U+0200–U+03FF ve U+0600–U+07FF'tir; **ayrıca** bütçe taşması U+0100–U+01FF (Latin Extended-A) ve U+0400–U+05FF (Kiril, İbrani) için de tetiklenir, ama bu aralıklarda üretilen *metin* doğru olduğu için Rust orada C ile birebir aynıdır — yalnızca C'nin taşması kopyalanmaz. F, C'nin çöpünü ABI sınırında geri koymaz; bu bilinçli kabul edilmiş bir uyumsuzluktur. |

---

## 9. Riskler

1. **B'nin churn'ü.** 102 çağrı yeri + public imzalar. Azaltım: `NodeRef` yönlendirici metodları sayesinde çağrı biçimleri korunur; migration mekanik ve derleyici güdümlüdür; A ve B ayrı commit'lerde kalır.
2. **Differential harness'ın kırılganlığı.** ~~`config.h` el yazımı ve C kaynak sürümüne bağlı.~~ **Düşürüldü:** harness bu makinede fiilen derlenip çalıştırıldı — düz `cc`, meson yok, el yazımı `config.h` **gerekmiyor** (`HAVE_CONFIG_H` tanımlanmadığı için `common.h` koşullu içe aktarıyor); §3'teki reçete doğrulanmıştır. Kalan risk yalnızca "C kaynak sürümüne bağlılık" ve o da harness'ın kaynakları sabit bir commit'ten okumasıyla sınırlanır.
3. **A'nın kapsam kayması.** `on_cdata`/`find_cdata` semantiği ve kaçış davranışı düzeltilirken `writer.rs` (pretty-print `trim()`, satır 3d), `xep.rs` ve `roster.rs` içindeki cdata varsayımları etkilenebilir; ayrıca `utility::escape`/`unescape` public API'sinin çağıranları (`lib.rs:60` export'u, `tools/iksperf.rs` karşılaştırması) sayısal referans üretmeye başlayınca davranış değiştirir. Azaltım: A'nın spec'inde bu dosyalar ve `iksperf` için regresyon testleri tanımlanır; `utility::escape`'in mi yoksa yalnızca serileştirme yollarının mı değişeceği A'da açıkça karara bağlanır.
4. **F'nin `unsafe` gereksinimi.** `forbid(unsafe_code)` ile çelişir. Azaltım: F'yi ayrı crate/modül olarak izole etmek; karar F'nin spec'inde verilir.
5. **G'nin `Send` çözümünün API'yi şekillendirmesi.** `LocalSet`/iş parçacığı + kanal yaklaşımı async API'nin imzasını etkiler. Azaltım: G'nin spec'inde en az iki yaklaşım karşılaştırılır.

---

## 10. Kapsam dışı

- C kütüphanesinde değişiklik.
- C'nin `ikstack` arena'sının birebir bellek istatistiklerinin (`iks_stack_stat`) Rust'ta aynı sayılarla üretilmesi — anlamlı bir eşleme yok; D/F'de "desteklenmiyor" olarak raporlanır.
- TLS backend'lerinin (GnuTLS/OpenSSL) birebir eşlenmesi; Rust `native-tls` üzerinden eşdeğer işlevsellik hedeflenir.
