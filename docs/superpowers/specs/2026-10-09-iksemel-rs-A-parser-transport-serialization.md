# iksemel-rs — Alt proje A: Parser, transport ve serileştirme doğruluğu

- **Tarih:** 2026-10-09
- **Durum:** Onay bekliyor. Uygulama planı (`writing-plans`) bu spec onaylandıktan sonra yazılır.
- **Üst belge:** [`2026-10-09-iksemel-rs-c-parity-roadmap.md`](2026-10-09-iksemel-rs-c-parity-roadmap.md) §5.A
- **Referans commit'ler:** iksemel-rs `6658e41`, iksemel (C) `5abdbce`
- **Kapsanan yol haritası kalemleri:** §2.1 satır **1, 2, 3a, 3b, 3c, 3d, 8, 9, 10**
- **Kapsam dışı bırakılan yol haritası kalemleri:** 4a/4b/4c (B), 5 (B), 6/7 (C)

Bu spec'teki **her** C davranışı, C kaynağı okunarak ve/veya derlenmiş oracle programı çalıştırılarak doğrulanmıştır. Doğrulanmamış hiçbir iddia yoktur; doğrulama komutları §1'de verilmiştir.

---

## 1. Doğrulanmış C semantiği (bu alt projenin oracle'ı)

A'nın tamamı bu bölümdeki davranışlara göre yazılır. Aşağıdaki her satır ya kaynak okumasıyla ya da fiilen çalıştırılmış bir oracle programının çıktısıyla sabitlenmiştir.

### 1.1 DOM tamamlanma ve kök sahipliği (`dom.c:16-55`)

C'nin DOM handler'ı iki ayrı işaretçi tutar: `data->current` (hâlen açık olan en içteki düğüm) ve `*data->iksptr` (kullanıcıya verilecek kök). Kök, **yalnızca kök element kapandığında** `iksptr`'ye yazılır.

Oracle çıktısı (`iks_tree(xml, 0, &err)`):

| Girdi | `err` | `*iksptr` | `iks_string` |
|---|---|---|---|
| `<r/>` | 0 | dolu | `<r/>` |
| `<a/><b/>` | 0 | dolu | `<b/>` — **son kök kazanır** |
| `text<r/>` | 0 | dolu | `<r/>` — kök öncesi metin atılır |
| `<r><c/>` | **0** | **NULL** | — |
| `</x>` | **2** | NULL | — |
| `<a></b>` | **2** | NULL | — |
| `<a/></b>` | **2** | dolu | `<a/>` — hata olsa da ağaç döner |
| `<a></a></b>` | **2** | dolu | `<a/>` |

Buradan çıkan kurallar:

1. Kapanış etiketi, `current` NULL iken gelirse (`</x>`, `<a/></b>`) **`IKS_BADXML`**'dir. Sebep: `iks_name(NULL)` NULL döner, `iks_strcmp(NULL, name)` = -1 ≠ 0 (`iks.c:63-67`) → `tagHook` `IKS_BADXML` verir (`dom.c:45-46`).
2. Kapanış etiketinin adı `current`'ın adıyla uyuşmazsa **`IKS_BADXML`**.
3. Eşleşme tamamsa `current = iks_parent(current)`; parent NULL ise kök teslim edilir ve `current = NULL` yapılır.
4. Kök kapanmadan girdi biterse `iksptr` **yazılmaz** ve `err` **`IKS_OK`** kalır — C'nin kendi tutarsızlığı (yol haritası D1).
5. Kök kapandıktan sonra gelen üst düzey metin atılır; gelen yeni bir element **yeni kök** olur ve `iksptr`'yi **ezer** (madde 2 tablosu, ikinci satır).
6. `TagType::Single` üst düzeyde ise C onu açar ve aynı çağrıda kapatır, yani hemen kök olur.

### 1.2 CDATA eklenti kuralı (`iks_insert_cdata`, `dom.c:57-62`)

`dom.c`'nin `cdataHook`'u **koşulsuz** `iks_insert_cdata(current, cdata, len)` çağırır; boşluk kontrolü **yoktur**. `iks_insert_cdata` ise:

- Son çocuk CDATA ise → **ona ekler** (yeni düğüm açmaz).
- Aksi hâlde → sona **yeni** bir CDATA düğümü ekler.

Oracle çıktısı:

| Girdi | Çocuklar |
|---|---|
| `<r> </r>` | 1 × CDATA `" "` — **boşluk korunur** |
| `<r>ab</r>` | 1 × CDATA `"ab"` |
| `<r>ab<q/>cd</r>` | CDATA `"ab"`, TAG `q`, CDATA `"cd"` — arada tag olduğu için **birleşmez** |
| `<r>\n  <a/>\n  <b/>\n</r>` | **5** çocuk: CDATA `"\n  "`, TAG `a`, CDATA `"\n  "`, TAG `b`, CDATA `"\n"` |

Son satır kritik: C biçimlendirilmiş (pretty-printed) XML'de boşluk metnini **düğüm olarak saklar**, kırpmaz.

### 1.3 `find_cdata` (`iks.c:450-459`)

```c
y = iks_find (x, name);
if (!y) return NULL;
y = IKS_TAG_CHILDREN (y);          /* ILK cocuk */
if (!y || IKS_CDATA != y->type) return NULL;
```

`IKS_TAG_CHILDREN` **ilk** çocuktur; `IKS_TAG_LAST_CHILD` sondur (`iks.c:30-31`), `iks_insert` sona ekler. Yani: **ilk çocuk CDATA değilse `NULL`**. Rust'ın şu anki "çocuklar arasında ilk CDATA'yı bul" davranışı C'den sapar.

### 1.4 Serileştirme kaçışı (`escape`, `iks.c:640-724`)

Tek fonksiyon, hem metin hem attribute için (`iks_string`, `iks.c:757` ve `iks.c:768`). Karakter başına kural:

| Girdi | Çıktı | Not |
|---|---|---|
| `&` `<` `>` `'` `"` | `&amp;` `&lt;` `&gt;` `&apos;` `&quot;` | **her iki bağlamda** (`iks.c:710`) |
| `\t` `\n` `\r` | birebir | `is_print` bunları yazdırılabilir sayar (`iks.c:568-571`) |
| diğer yazdırılabilir ASCII | birebir | |
| `0x01`–`0x08`, `0x0B`, `0x0C`, `0x0E`–`0x1F`, `0x7F` | `&#xNN;` | `snprintf("&#x%02x;")`, `iks.c:703` — **en az 2 hane, lowercase** |
| `U+0000` | **hiçbir şey** (düşürülür) | `c != 0x00` kontrolü, `iks.c:702`; `a\0b` → `ab` |
| ASCII dışı kod noktaları | `&#x...;` | `snprintf("&#x%02x;", code_point)`, `iks.c:673/681/689` — en az 2 hane, lowercase |
| ham bayt `0x80`–`0x9F` | **hiçbir şey** (düşürülür) | `iks.c:667-669` |

Doğrulanmış çıktılar: `café ü` → `caf&#xe9; &#xfc;`; `a="café"` → `a="caf&#xe9;"`; metinde `"` → `&quot;`; CDATA'da `'` → `&apos;`; `0x01` → `&#x01;`; `0x7f` → `&#x7f;`; `U+0080` (`C2 80`) → `&#x80;`; `U+009F` (`C2 9F`) → `&#x9f;`; `U+00A0` → `&#xa0;`; `U+03B5` → `&#x3b5;`; `U+20AC` → `&#x20ac;`; `U+1F600` → `&#x1f600;`.

**Önemli:** C1 aralığı (`U+0080`–`U+009F`) **korunur**. C'nin `0x80`–`0x9F` düşürmesi *ham bayt* üzerindedir ve geçerli UTF-8'de öyle bir bayt tek başına bulunamaz; dolayısıyla Rust `&str` girdisiyle o yol erişilemez (yol haritası D8). `U+0000` dışında A'da **kopyalanmayan veri kaybı yoktur**.

C'nin bu fonksiyondaki iki hatası (2 baytlık maske `0xE8` ve buna bağlı bütçe taşması) **kopyalanmaz** — gerekçe yol haritası D9.

### 1.5 Bayt düzeyi yasaklar (`sax_core`, `sax.c:209`)

Döngünün en başında, bağlamdan bağımsız: `0x00`, `0xFE`, `0xFF` baytları → `IKS_BADXML`.

Oracle: `iks_parse(p, "<r>a\0b</r>", 9, 1)` → `2`; `iks_parse(p, "<r x=\"a\0b\"/>", 12, 1)` → `2`.

Rust'ın bayt tarafları yalnızca `<`, `&`, `'`, `"` aradığı için `\0`'ı kabul eder. `0xFE`/`0xFF` zaten geçerli UTF-8 olmadığından transport sınırında reddedilir; A'da yalnızca `0x00` için iş yapılır.

### 1.6 Token limitleri

C'de **hiçbir** boyut limiti yoktur (`sax.c` içinde `MAX_`/limit sabiti yoktur; tarandı). `ParserLimits` **Rust'a özgü** bir DoS korumasıdır. Dolayısıyla A4 bir C paritesi işi değil, **kendi sözleşmesini tutarlı kılmak** işidir: `max_token_size` "tek bir token, attribute değeri veya CDATA tamponu için azami bayt" diye belgelenmiştir (`parser.rs:250`), ama şu an yalnızca iki attribute-değeri noktasında uygulanıyor.

---

## 2. A1 — Artımlı UTF-8 çözme

### Sorun

Her soket okuması bağımsız olarak `std::str::from_utf8(&buf[..n])` ile çözülüyor. Çok baytlı bir karakter iki `read` arasına düşerse ikinci parça geçersiz UTF-8 olur ve `BadXml` döner. Bu, gerçek ağda **olağan** bir durumdur (TCP parça sınırları keyfîdir), dolayısıyla mevcut kod yavaş/parçalı bağlantılarda çalışmaz.

Repo genelinde bayt→`str` dönüşümü yapan **tam olarak üç** üretim yolu vardır (`grep` ile doğrulandı):

1. `net.rs` — `Connection::recv_event` (`src/net.rs:222`)
2. `async_net.rs` — `AsyncConnection::recv_event` (`src/async_net.rs:266`)
3. `async_net.rs` — `AsyncReceiver::recv_event` (`src/async_net.rs:75`)

`ConnectionStream::from_tcp_stream`, `AsyncConnection::split` ve `split_channels` bu üçüne bağlanır; ayrı dönüşüm noktası değildir.

### Tasarım

Yeni bir crate-içi modül: **`src/utf8.rs`**, içinde `Utf8Carry`.

```rust
pub(crate) struct Utf8Carry {
    pending: [u8; 3],   // yarım kalmış dizinin baştaki baytları
    pending_len: usize,
    staging: Vec<u8>,   // yalnızca pending_len > 0 iken kullanılan birleştirme tamponu
}

impl Utf8Carry {
    pub(crate) fn new() -> Self;
    pub(crate) fn reset(&mut self);
    /// `chunk`'ı besler ve geçerli UTF-8 olan en uzun öneki döndürür.
    /// Kesinlikle geçersiz bayt varsa `Err(IksError::BadXml)`.
    pub(crate) fn feed<'a>(&'a mut self, chunk: &'a [u8]) -> Result<&'a str>;
}
```

`staging: Vec<u8>` alanı kasıtlıdır: yarım kalan bir karakterin baytları ile `chunk`'ın başı bitişik olmadığı için birleştirilmiş çıktı hiçbir tek dilimde bulunmaz. Dönen `&'a str` bu yüzden ya `self.staging`'e ya `chunk`'a ödünç verir; imzadaki ortak `'a` ikisini de kapsar. `staging` yalnızca `pending_len > 0` iken (yani bir karakter okuma sınırına denk geldiğinde) kullanılır ve tam da bu durumda `chunk`'ı kopyalar — sınır durumu seyrektir.

Algoritma (`pending_len == 0` yolu):

1. `std::str::from_utf8(chunk)`:
   - `Ok(s)` → `Ok(s)`.
   - `Err(e)` ve `e.error_len() == Some(_)` → **kesinlikle geçersiz** → `Err(BadXml)`.
   - `Err(e)` ve `e.error_len() == None` → **girdi erken bitti**: `n = e.valid_up_to()`, kuyruk `chunk[n..]`.
     - `chunk[n..].len() > 3` ise `Err(BadXml)` (savunma; `error_len() == None` için bu asla olmaz, çünkü tamamlanmamış bir dizi en fazla 3 bayttır).
     - Kuyruğu `self.pending`'e kopyala, `pending_len`'i ayarla, `Ok(&chunk[..n])` döndür. `n == 0` olabilir (dönen değer boş `&str`).

Algoritma (`pending_len > 0` yolu):

1. `staging = pending[..pending_len] ++ chunk`.
2. Aynı `from_utf8` mantığı. `error_len() == Some(_)` → `Err(BadXml)`.
3. `n = valid_up_to()` (`Ok` ise `n = staging.len()`).
4. **`n < pending_len` ise `Err(BadXml)`** — savunma amaçlı; `pending` yapısı gereği geçerli bir dizinin önekidir, dolayısıyla `valid_up_to()` ya `pending_len`'e eşit ya da ondan büyüktür. Bu kontrol, `pending`'in 3 baytlık tamponu taşmasını yapısal olarak imkânsız kılar.
5. Yeni kuyruk `staging[n..]` (uzunluğu ≤ 3 garantidir).
6. `Ok(&self.staging[..n])`.

### Yerleşim

Her üç çağrı noktası da `feed`'in `Err` sonucunu olduğu gibi `BadXml` olarak yukarı verir (mevcut davranışla aynı hata tipi). `Connection`/`AsyncConnection`/`AsyncReceiver` birer `utf8: Utf8Carry` alanı kazanır ve:

- `Connection::start_stream` / `AsyncConnection::start_stream` / `AsyncConnection::start_tls` içinde `parser.reset()`'in **yanında** `utf8.reset()` çağrılır.
- `split` ve `split_channels` carry'yi **receiver**'a taşır (parser oraya taşındığı gibi).
- `log_traffic` açıkken günlüğe yazılan metin `feed`'in döndürdüğü önek olur (mevcut davranışla aynı çıktı).

### Kabul kriterleri

- Birim testleri (`src/utf8.rs` içinde, soketsiz): `€` (`E2 82 AC`) baytları 1+2, 2+1 ve 1+1+1 şeklinde bölündüğünde toplam çıktı tektir ve `€`'dür; `😀` (`F0 9F 98 80`) 4 farklı bölünmede doğru çıkar; kuyruk gerçekten tamponlanır (dönen önek kısadır) ve `reset()` kuyruğu temizler.
- `Err` testleri: `0xFF` → ilk `feed`'de `BadXml`, hiç tamponlanmadan; `[0xE2, 0x28]` (kesin geçersiz devam baytı) → `BadXml`; `[0xE2, 0x82]` tek başına → `Ok("")`, sonraki `feed` ile tamamlanır.
- Entegrasyon testi (**üç yolun her biri için ayrı**): `127.0.0.1:0` üzerinde bir `TcpListener`; sunucu bir stanza'yı çok baytlı bir karakterin **ortasından** iki ayrı `write` ile gönderir. Sırasıyla `Connection` (senkron), `AsyncConnection` (async) ve `AsyncConnection::split` sonrası `AsyncReceiver` bunu tek ve doğru stanza olarak almalıdır. Test üç kez ayrı ayrı yazılır, tek bir yardımcıyla üçünü birlikte değil — regresyonun hangi yolda olduğunu göstermelidir.

---

## 3. A2 — DOM tamamlanma

### Sorun

`DomParser::on_tag` kökü **açılış** anında `self.root`'a yazıyor ve açık düğümleri `node_stack` içinde tutuyor. İki sonuç:

- Kök kapanmasa bile `document()` dolu döner; `parse_str("<r><c/>")` → `Ok(<r><c/></r>)` (yol haritası satır 2).
- Sarkan kapanış etiketi (`</x>`, `<a/></b>`) yığın boşken **sessizce yutulur**; C ise `IKS_BADXML` verir (§1.1 madde 1). Bu, yol haritasında listelenmemiş **ek** bir sapmadır ve A2 kapsamında kapatılır.

### Tasarım

`DomParser` C'nin `dom_data` yapısını birebir taklit eder; `node_stack` **kaldırılır** (parent bağı zaten mevcut).

```rust
pub struct DomParser {
    root: Option<Rc<RefCell<IksNode>>>,     // C: *iksptr  — yalnız kök kapandığında yazılır
    current: Option<Rc<RefCell<IksNode>>>,  // C: data->current
    chunk_size: usize,
}
```

`on_tag` (C'nin `tagHook`'unun doğrudan karşılığı):

- **Open/Single:**
  - `current` doluysa: `let rc = current.borrow_mut().add_child(node_with_attrs);`
  - `current` boşsa, yani yeni kök: `let rc = node_with_attrs.into_rc();`
  - `Open` ise → `self.current = Some(rc)`. `Single` ise → C'nin aynı çağrıda kapatma davranışı: parent yoksa `self.root = Some(rc)` (yeni kök, öncekini ezer), `self.current` değişmez; parent varsa `self.current` yine değişmez.
- **Close:**
  - `let Some(cur) = self.current.clone() else { return Err(IksError::BadXml) };`  ← C'nin `iks_strcmp(NULL, name) != 0` yolu.
  - `cur.borrow().name.as_deref() != Some(name)` → `Err(IksError::BadXml)`.
  - Aksi hâlde parent'a çık: `Some(p)` → `self.current = Some(p)`; `None` → `self.root = Some(cur); self.current = None;`

`document()` artık `self.root.clone()` döner ve **tam olarak `*iksptr` anlamına gelir**.

### `Parser::finish()` ve `SaxHandler::on_finish()`

`parse_str`/`parse_str_with_limits`/`load_file` parse bittikten sonra `finish()` çağırır. Yol haritası D4'ün sözü budur: C'nin `iks_parse`'ının `finish` parametresi ölü koddur (`sax.c:634-644` onu hiçbir yere geçirmez), Rust ise onun *amaçlanan* anlamını gerçekler.

```rust
pub trait SaxHandler {
    fn on_tag(...) -> Result<()>;
    fn on_cdata(&mut self, data: &str) -> Result<()>;
    /// Belge bittiğinde çağrılır. Varsayılan gövde hiçbir şey yapmaz.
    fn on_finish(&mut self) -> Result<()> { Ok(()) }
}
```

Varsayılan gövdeli (provided) olması **zorunludur**: repo içinde `SaxHandler`'ı uygulayan 7 tip var (`tools/iksperf.rs`, `tools/ikslint.rs`, `bench/comparison.rs`, `tests/parser_stress.rs`, `src/stream.rs`, `src/dom.rs` ve `src/filter.rs` dolaylı) ve hiçbiri kırılmamalıdır.

```rust
impl<H: SaxHandler> Parser<H> {
    pub fn finish(&mut self) -> Result<()> {
        self.handler.on_finish()
    }
}
```

`DomParser::on_finish` → `match self.current { Some(_) => Err(IksError::BadXml), None => Ok(()) }`.

**Not — `finish` neden bu kadar küçük:** `Parser::parse`'ın sonundaki mevcut kuyruk boşaltma (`parser.rs:748-752`) `state == CData` iken `buffer`'ı zaten `on_cdata`'ya verir. `finish`'in ekleyeceği tek gerçek bilgi "hâlâ açık bir element var mı" sorusudur ve bunu yalnızca handler bilir. Bu yüzden mantık handler'a, kontrol parser'a konur.

### Sonuç (`parse_str` davranışı)

| Girdi | Önce | Sonra | C (§1.1) |
|---|---|---|---|
| `<r/>` | `Ok(<r/>)` | `Ok(<r/>)` | `err=0`, dolu |
| `<r><c/>` | `Ok(<r><c/></r>)` | **`Err(BadXml)`** | `err=0`, NULL (D1 sapması) |
| `</x>` | `Ok`? → `document()` yok → `Err(BadXml)` | `Err(BadXml)` | `err=2` ✓ |
| `<a/></b>` | **`Ok(<a/>)`** | **`Err(BadXml)`** | `err=2`, dolu ✓ |
| `<a/></b>` (hata sonrası ağaç) | — | `document()` = `<a/>` | ✓ |
| `<a/><b/>` | `Ok(<b/>)` | `Ok(<b/>)` | ✓ |
| `text<r/>` | `Ok(<r/>)` | `Ok(<r/>)` | ✓ |

`<a/></b>` satırı bu spec'in bulduğu, yol haritasında olmayan sapmayı kapatır.

### Kabul kriterleri

- Yukarıdaki yedi satırın her biri için bir test; `<a/></b>` için hem `Err` hem de `handler().document()` değerinin `<a/>` olduğu ayrıca kontrol edilir.
- `SaxHandler`'ın yeni metodu yüzünden hiçbir mevcut uygulamanın değişmesi gerekmediğini kanıtlayan derleme (mevcut test paketi yeşil).
- `Parser::finish()` doğrudan kullanıldığında (DomParser olmadan) bir handler'ın kendi `on_finish`'ini gördüğünü doğrulayan bir test.

---

## 4. A3 — Metin doğruluğu

Dört ayrı düzeltme; hepsi §1.2 ve §1.3'ten türer.

### 4.1 Boşluk korunur

`dom.rs::on_cdata` (`src/dom.rs:224`) ve `stream.rs::on_cdata` (`src/stream.rs:174`) içindeki `if !data.trim().is_empty()` koşulları **kaldırılır**. C'nin `cdataHook`'unda böyle bir kontrol yoktur; `<r> </r>` tek bir `" "` CDATA düğümü üretir.

Bunun görünür sonucu, biçimlendirilmiş XML'de çocuk sayılarının artmasıdır: `<r>\n  <a/>\n  <b/>\n</r>` artık **5** çocuk üretir (C ile aynı). `src/dom.rs`'in kendi test modülündeki `test_dom_child` (1 bekliyor) ve `test_dom_parsing` (3 bekliyor) bu yüzden **C semantiğine göre güncellenir** — gevşetilmez, beklenen değerler C'nin verdiği sayılara çekilir. Aynı şekilde `tests/dom_writer_traversal.rs` ve `tests/parser_stress.rs` gözden geçirilir.

### 4.2 Ekleme anında birleştirme (madde 3b)

İki handler de aynı kuralı uygulamalı: **son çocuk CDATA ise ona ekle, değilse yeni CDATA düğümü aç.** `stream.rs` bunu zaten yapıyor ama `dom.rs` her çağrıda yeni düğüm açıyor.

Kural **tek bir yerde** yazılır — `src/lib.rs` içinde, `find_cdata`'nın yanında, crate-içi bir yardımcı olarak:

```rust
/// `parent`'a metin ekler; son çocuk CDATA ise ona ekler.
/// `iks_insert_cdata`'nın kuralı (C: `iks.c`, ayrıca bkz. spec §1.2).
pub(crate) fn append_text(parent: &Rc<RefCell<IksNode>>, data: &str) {
    let mut p = parent.borrow_mut();
    if let Some(last) = p.children.last() {
        let mut last = last.borrow_mut();
        if last.node_type == IksType::CData {
            if let Some(content) = last.content.as_mut() {
                content.push_str(data);
                return;
            }
        }
    }
    let mut node = IksNode::new(IksType::CData);
    node.set_content(data);
    p.add_child(node);
}
```

B (düğüm modeli) bu yardımcıyı `NodeRef::append_cdata` olarak **genel** API'ye yükseltecek; A onu yalnızca crate-içi tutar.

**Neden parser'da birleştirmiyoruz:** `Parser::parse` bir `buffer`'ı yalnızca `<`/`&` gördüğünde veya girdi bittiğinde boşaltır (`parser.rs:439-444`, `748-752`), dolayısıyla `<r>ab` + `</r>` iki parça hâlinde gelirse iki `on_cdata` çağrısı olur ama araya tag girmez. C'de bu tek düğümdür. Birleştirmeyi handler'da yapmak hem C ile birebir olur hem de `SaxHandler` sözleşmesini (parça parça teslim) bozmaz.

### 4.3 `find_cdata` ilk-çocuk kuralı (madde 3c)

`lib.rs:385-393` §1.3'e çekilir: `find(name)` ile bulunan düğümün **ilk çocuğu** CDATA değilse `None`.

### 4.4 `writer.rs` pretty-print kırpması (madde 3d)

`src/writer.rs:103-105` içindeki `text.trim()` **kaldırılır**; CDATA içeriği birebir yazılır.

`XmlWriter` C'de karşılığı olmayan Rust'a özgü bir akış yazıcısıdır; pretty modu zaten düğümler arasına boşluk ekler ve bu bilinçli olarak kayıplıdır. Kural şudur: **pretty modu düğümler *arasına* boşluk ekleyebilir, ama bir düğümün kendi içeriğinin veya attribute değerinin baytlarını asla değiştiremez.** Kırpma bu kuralı ihlal ediyordu. Pretty modda CDATA için `write_indent()` korunur (okunabilirlik için eklenen boşluk), `\n` ayırıcısı da korunur; içerik kırpılmaz.

Parite açısından asıl yol `writer.rs` **değil**, `IksNode::to_string()`'in kullandığı `lib.rs`'teki `escape_attr`/`escape_text`'tir (`lib.rs:895-918`); `writer.rs` düzeltmesi içerik kaybını önlemek içindir.

### Kabul kriterleri

- `<r> </r>` → serileştirme `<r> </r>`, çocuk sayısı 1 (C oracle ile aynı).
- `<r>\n  <a/>\n  <b/>\n</r>` → çocuk sayısı **5**, düğüm tipleri sırayla CDATA/TAG/CDATA/TAG/CDATA, metinler `"\n  "`, `"\n  "`, `"\n"` — oracle çıktısıyla bayt-birebir.
- `<r>ab<q/>cd</r>` → 3 çocuk.
- `<r>ab` + `</r>` iki `parse_chunk` çağrısıyla → **tek** CDATA düğümü `"ab"`.
- `<r><a>ab</a><a/>x</r>` → `find_cdata("a")` ilki için `"ab"`, ikincisi için `None` (ilk çocuk TAG değil, CDATA yok çünkü `<a/>` çocuksuz).
- `<r><a><b/>text</a></r>` → `find_cdata("a")` **`None`** (ilk çocuk TAG `b`), Rust'ın eski "dolaş ve bul" davranışı kilitlenir.
- `XmlWriter::set_pretty(true, 2)` ile `<r>` içinde CDATA `" ab "` yazıldığında çıktı `" ab "` içerir (kırpılmamış).
- Pretty **kapalı** `XmlWriter` çıktısı `parse_str` ile geri okunduğunda CDATA baytları birebir aynıdır (round-trip).

---

## 5. A4 — Token limitleri ve yasak bayt

### 5.1 `max_token_size` tüm biriktirme yollarına

Şu an kontrol yalnızca `parser.rs:683` (`ValueApos`) ve `parser.rs:705` (`ValueQuot`). Denetimsiz birikme noktaları:

| Yol | Konum | Biriktirme |
|---|---|---|
| Metin | `parser.rs:427-429` | `self.buffer.push_str(&data[start..i])` |
| CDATA bölümü | `parser.rs:479` civarı | `self.buffer.push_str(...)` |
| Tag adı | `parser.rs:514`, `parser.rs:634` | `self.tag_name.push(c)` |
| Attribute adı | `parser.rs:659` | `self.attr_name.push(c)` |

Her noktaya, birikimden **sonra**, `len > limits.max_token_size` ise `Err(IksError::MaxTokenSizeExceeded)` kontrolü eklenir. Mevcut iki noktayla aynı hata tipi ve aynı "kesin büyüklük > limit" karşılaştırması kullanılır (`==` değil), böylece mevcut `tests/security_dos_limits.rs:141` davranışı korunur.

Metin ve CDATA için kontrol `self.buffer` üzerindedir ve `buffer` her `on_cdata` sonrası temizlendiği için "tek token" = "tek kesintisiz parça" olur; bu, `ParserLimits` dokümantasyonundaki "tek bir token, attribute değeri veya CDATA tamponu" ifadesiyle uyumludur.

### 5.2 `0x00` baytı reddedilir (madde 10)

`Parser::parse`'ın **girişinde**, tek bir satır:

```rust
pub fn parse(&mut self, data: &str) -> Result<()> {
    if data.as_bytes().contains(&0) {
        return Err(IksError::BadXml);
    }
    let bytes = data.as_bytes();
    ...
}
```

**Neden üst düzey bir bayt döngüsü kontrolü *yetmez*:** `parse`'ın üç hızlı bayt tarama yolu (`State::CData`, `parser.rs:416`; `State::CommentBody`, `parser.rs:450`; `State::SectCDataC`, `parser.rs:468`) ham baytları tarar ve yalnızca kendi ayırıcılarında (`<`/`&`, `-`, `]`) durur. `<r>a\0b</r>` girdisinde `CData` taraması `a\0b`'yi tek hamlede geçer ve `i` doğrudan `<`'e atlar; döngünün tepesindeki bir kontrol `\0`'ı **hiç görmez**. Bu üç yolu tek tek yamamak gerekir ve yarın eklenecek dördüncü bir hızlı yol sessizce aynı boşluğu açar.

Girişteki tek tarama bunun yerine C'nin yapısını birebir kopyalar: `sax_core`'daki kontrol de (§1.5) bağlamdan ve konumdan bağımsızdır, her baytta bir kez çalışır. Bir satır, tüm bağlamları (metin, CDATA, kesit CDATA, yorum, PI, tag adı, attribute adı ve değeri) istisnasız kapsar ve parser'ın durum makinesinden **bağımsız** olduğu için gelecekteki durumlara karşı da doğru kalır.

Maliyet: `parse` çağrısı başına bir doğrusal tarama. Parser zaten girdiyi en az bir kez bayt bayt geziyor, dolayısıyla bu sabit çarpanlı bir ek geçiştir. `parse_chunk` parça parça çağrıldığında tarama parça başına yapılır ve bu da doğrudur (parça sınırı `0x00`'ı gizleyemez). Ölçüm bu geçişin kayda değer olduğunu gösterirse, tarama mevcut üç hızlı döngünün içine, her birinin `let b = bytes[i];` satırının yanına dağıtılabilir — ama bu, doğruluğu dört ayrı yerde doğru yazmaya bağlar ve bu yüzden **tercih edilmez**.

### Kabul kriterleri

- `<r>a\0b</r>` ve `<r x="a\0b"/>` → `Err(BadXml)` (C oracle `IKS_BADXML` ile aynı). Yanı sıra `\0` tag adında, attribute adında ve `<![CDATA[\0]]>` içinde de reddedilir.
- `max_token_size: 4` ile: 10 baytlık metin, 10 baytlık CDATA, 10 baytlık tag adı ve 10 baytlık attribute adı **reddedilir**; 4 baytlık olanlar kabul edilir (sınır davranışı: 4 kabul, 5 reddet).
- Mevcut `tests/security_dos_limits.rs` ve `tests/parser_stress.rs` yeşil kalır.
- `ParserLimits::default()` ile 1 MB'lık tek bir metin/CDATA parçası hâlâ kabul edilir (yanlış pozitif yok).

---

## 6. A5 — Kaçış birleştirme

### Sorun

Altı ayrı kaçış fonksiyonu ve iki ayrı bütçe fonksiyonu var; hiçbiri sayısal referans üretmiyor ve metin yolları `'`/`"` kaçırmıyor:

| Fonksiyon | Konum | Kapsam |
|---|---|---|
| `escape_cow` | `utility.rs:107` | 5 varlık, `Cow` sıfır-ayırma hızlı yolu |
| `escape` | `utility.rs:140` | `escape_cow`'un sarmalayıcısı (genel API, `lib.rs:60`'ta dışa veriliyor) |
| `escape_attr` | `lib.rs:951` | 5 varlık |
| `escape_text` | `lib.rs:960` | 3 varlık (`& < >`) |
| `write_escaped_attr` | `writer.rs:133` | 5 varlık, doğrudan `Write`'a |
| `write_escaped_text` | `writer.rs:161` | 3 varlık, doğrudan `Write`'a |
| `escape_size` | `helper.rs:61` | genel API, "her karakter 1 bayt" varsayımı |
| `escape_size` + `escape` | `parser.rs:26`, `parser.rs:48` | yalnızca `Parser::serialize` (hata ayıklama) kullanıyor |

### Tasarım

Tek bir crate-içi çekirdek, **iki çıktı biçimi**: döndüren ve akıtan. Çekirdek, "bir karakter hangi baytlara karşılık gelir" kararını **tek bir yerde** verir; iki biçim yalnızca o baytları nereye yazacağında ayrışır.

```rust
// src/escape.rs  (yeni, crate-içi)

/// Bir karakter için §1.4 tablosuna göre kaçış çıktısı.
/// `None` → karakter düşürülür (yalnızca U+0000).
pub(crate) fn escape_char(c: char) -> Option<EscapeOut>;

pub(crate) enum EscapeOut {
    Literal,                    // olduğu gibi
    LiteralSlice(&'static str), // &amp; &lt; &gt; &apos; &quot;
    Numeric([u8; 10], u8),      // "&#xNNNN;" — lowercase hex, en az 2 hane
}

/// `write`'a kaçışlı yazar. Ayırma yapmaz.
pub(crate) fn write_escaped<W: Write>(w: &mut W, s: &str) -> io::Result<()>;

/// Kaçışlanmış uzunluğu döndürür (bütçe; her zaman `write_escaped`'in
/// yazdığı bayt sayısına eşit).
pub(crate) fn escaped_len(s: &str) -> usize;
```

Karar tablosu (§1.4'ün birebir kodu):

- `&` `<` `>` `'` `"` → `LiteralSlice`
- `\t` `\n` `\r` → `Literal`
- `c.is_ascii_graphic()` ve `c != ' '`… dikkat: C `isprint` **boşluğu da** yazdırılabilir sayar, `0x7F`'i saymaz. Doğru ifade: `c` yazdırılabilir ASCII ise (`0x20`–`0x7E`) → `Literal`; `0x7F` dahil diğer ASCII kontrolleri → `Numeric`.
- `'\0'` → **`None`** (C gibi düşür).
- ASCII olmayan (`c > '\u{7F}'`) → `Numeric`, **her zaman doğru kod noktasıyla** (C'nin iki baytlık maske hatası kopyalanmaz, D9).

`Numeric`'in biçimi C'nin `snprintf("&#x%02x;")` çıktısıyla birebir olmalıdır: `&#x` + lowercase hex + en az iki hane (`format!("{:x}")` tek haneliyse başına `0` eklenir) + `;`. Tampon boyu en büyük kod noktasından türer: `U+10FFFF` → `&#x10ffff;` = 10 bayt, dolayısıyla `[u8; 10]` yeterlidir ve taşma derleme zamanında imkânsızdır.

### Mevcut fonksiyonların yeni rolleri

| Fonksiyon | Yeni gövde |
|---|---|
| `escape_cow` | Değişmez, saf ASCII girdide `Cow::Borrowed` dönen hızlı yol **korunur**; kaçış gerektiğinde çekirdeğe delege eder. Hızlı yol koşulu artık "girdi tamamen `0x20`–`0x7E` ve beş özel karakterden hiçbiri yok" olur. |
| `escape` | `escape_cow(s).into_owned()` — değişmez. |
| `escape_attr` | **Silinir.** Tek çağrı yeri `lib.rs:895`; artık `escape_cow(...)` kullanır (attribute'ta `'` ve `"` kaçılır, çünkü çekirdek bağlamdan bağımsızdır). |
| `escape_text` | **Silinir.** `lib.rs:905` ve `lib.rs:918` artık `escape_cow(...)` kullanır. Metinde `'`/`"` da kaçılır — C böyle yapar. |
| `write_escaped_attr` | **Silinir.** `writer.rs:61` → `escape::write_escaped(...)`. |
| `write_escaped_text` | **Silinir.** `writer.rs:76` ve CDATA kolu → `escape::write_escaped(...)`. |
| `helper.rs::escape_size` | Gövdesi `escape::escaped_len(s)` olur. Genel API korunur, anlamı artık doğrudur. |
| `parser.rs::escape_size`, `parser.rs::escape` | **Silinir.** `Parser::serialize` çekirdeği kullanır. |

`utility.rs`'in sıfır-ayırma iddiası korunur ve `tools/iksperf.rs` (`escape_cow_test`, `escape_test`) bunu ölçmeye devam eder. **Dikkat:** `iksperf.rs`'in örnek dizeleri saf ASCII olduğu için hızlı yol geçerli kalır; sayısal referans yolu yalnızca ASCII dışı girdide devreye girer ve o durumda `Cow::Owned` beklenmelidir. `iksperf.rs`'in bir kıyaslaması ASCII dışı dize içeriyorsa beklenen sonuç güncellenir.

### Kabul kriterleri

- Oracle ile bayt-birebir diff (§8), D9 aralıkları hariç: `café ü`, `a="café"`, metinde `"`, metinde `'`, `0x01`, `0x7f`, `\t`/`\n`/`\r`, `U+0080`, `U+009F`, `U+00A0`, `€`, `😀`.
- `U+0000` programatik olarak (`set_content("\0")`) yazıldığında çıktıdan düşer, `parse_str` çıktısında hata vermez — C ile aynı.
- `escaped_len(s)` her test dizesi için `write_escaped(...)`'in yazdığı bayt sayısına **eşittir** (property testi; C'de bu eşitlik taşmanın kaynağıdır ve burada yapısal olarak garanti edilir).
- Altı eski fonksiyonun adı repoda kalmaz (`grep` ile doğrulanır).
- `Parser::serialize` çıktısı hâlâ derleniyor ve eski davranışıyla aynı şekli üretir, yalnızca kaçışları yeni semantiğe uyar.

---

## 7. Dokunulan dosyalar

| Dosya | Değişiklik |
|---|---|
| `src/utf8.rs` | **Yeni.** `Utf8Carry` + birim testleri |
| `src/escape.rs` | **Yeni.** `escape_char`, `write_escaped`, `escaped_len` + birim testleri |
| `src/parser.rs` | `0x00` reddi, `max_token_size` dört yol, `finish()`, `SaxHandler::on_finish` (varsayılan gövdeli), `escape`/`escape_size` silinir, `serialize` çekirdeğe geçer |
| `src/dom.rs` | `current`/`root` ayrımı, `node_stack` kaldırılır, `on_cdata` `append_text`, `on_finish`, `parse_str`/`parse_str_with_limits`/`load_file` `finish()` çağırır, testler C semantiğine güncellenir |
| `src/lib.rs` | `append_text` (crate-içi), `find_cdata` ilk-çocuk kuralı, `escape_attr`/`escape_text` silinir |
| `src/stream.rs` | `on_cdata` boşluk koşulu kaldırılır, `append_text` kullanır |
| `src/writer.rs` | CDATA kırpması kaldırılır, `write_escaped_*` çekirdeğe geçer |
| `src/helper.rs` | `escape_size` çekirdeğe delege |
| `src/net.rs` | `Utf8Carry` alanı, `from_utf8` yerine `feed`, `reset` |
| `src/async_net.rs` | `Utf8Carry` alanı (`AsyncConnection` ve `AsyncReceiver`), `reset`, `split`/`split_channels` carry'yi taşır |
| `tests/` | Yeni differential + regresyon testleri; `dom_writer_traversal.rs`, `parser_stress.rs`, `security_dos_limits.rs` gözden geçirilir |
| `tools/iksperf.rs` | `escape_cow` beklentisi gerekirse güncellenir |
| `src/lib.rs:60` | Dışa verilen adlar: `escape`, `escape_cow`, `escape_size`, `unescape`, `unescape_cow`, `unescape_size` korunur (genel API daraltılmaz) |

**Genel API kırılması:** Yalnızca davranış değişir (`escape` artık sayısal referans üretir), imzalar değişmez. Ancak `escape_text`/`escape_attr` zaten `pub` değildi (lib.rs içi özel fonksiyonlar), `parser.rs`'tekiler de öyle; dolayısıyla **hiçbir genel imza değişmez**. Yeni genel API: `Parser::finish()` ve `SaxHandler::on_finish()`.

**`#![forbid(unsafe_code)]` korunur.** A'nın hiçbir parçası `unsafe` gerektirmez; `Utf8Carry` de `escape_char`'ın `Numeric` tamponu da güvenli Rust ile yazılır.

---

## 8. Doğrulama planı

**Birincil: differential suite.** Yol haritası §7'deki harness A'da **kurulur** (A5 onun ön koşuludur: kaçış birleşmeden çıktı karşılaştırması gürültülüdür).

- **Oracle derlemesi (doğrulandı):** meson gerekmez. Saf XML alt kümesi düz `cc` ile derlenir; bu spec yazılırken fiilen kullanılan asgari reçete:
  ```
  cc -w -include string.h -include stdlib.h -include errno.h \
     -I<iksemel>/include -o oracle oracle.c \
     src/iks.c src/sax.c src/dom.c src/ikstack.c src/utility.c
  ```
  `HAVE_CONFIG_H` tanımlanmadığı için el yazımı `config.h` gerekmez. TLS/ağ/`io-posix` dahil edilmez.
- **Oracle kuralı:** oracle, `iks_string(NULL, x)` (malloc yolu) yerine **daima bir `ikstack`** kullanır. Sebep: §1.4'teki taşma `ikstack` ile arena bloğunun içinde kalır ve ASAN'a görünmez; harness'ı taşmaya maruz bırakmamak için malloc yolu kullanılmaz ve non-ASCII çıktılar zaten §8'deki istisnaya girer.
- **Karşılaştırma istisnaları:** yalnızca (a) U+0200–U+03FF ve U+0600–U+07FF içeren senaryolar (D9 — Rust doğru kod noktasını üretir, C çöp üretir; ayrı testle kilitlenir), (b) `U+0000` (A'dan sonra parser'da reddedilir; "iki taraf da hata verir" olarak kilitlenir). **Başka istisna yoktur**; C1 aralığı dahil her şey bayt-birebir karşılaştırılır.
- **Korpus:** §1'deki tüm oracle tabloları (kök/kapanış davranışı, çocuk şekilleri, `find_cdata`, kaçış tablosu) + bölünmüş UTF-8 senaryoları + limit senaryoları.

**İkincil:** `cargo test --all-features` (mevcut 95 test + 3 doctest) yeşil; `cargo clippy` temiz; `cargo miri` en az yeni `Utf8Carry` ve `escape` birim testlerinde temiz (her ikisi de `unsafe` içermediği için asıl faydası `DomParser`'ın yeni `Rc`/`Weak` trafiğindedir).

**Başlangıç durumu:** `cargo test --all-features` bu çalışmaya başlarken **yeşildi** (95 test + 3 doctest, exit 0). A'nın kabul ölçütü bu sayının altına düşmemek ve yeni testleri eklemektir.

---

## 9. Riskler ve açık sorular

| # | Risk | Değerlendirme |
|---|---|---|
| R1 | Boşluk korunması çocuk sayılarını değiştirdiği için mevcut testler kırılır | **Beklenen.** `dom.rs`'in iki testi zaten yanlış davranışı kodluyor; C'nin verdiği sayılara çekilir. Diğer testler `child_tags()`/`find()` süzgeçleri kullandığı için etkilenmez (önceden incelendi: `tests/dom_writer_traversal.rs:202`, `tests/parser_stress.rs:107-123`). |
| R2 | `on_cdata`'nın artık boşluk düğümü üretmesi XMPP stanza işlemede gürültü yaratır | Boşluk düğümleri **C'de de vardır**; C ile çalışan uygulamalar bunlarla yaşar. `find_cdata`'nın ilk-çocuk kuralı zaten `<body>` gibi alanların önündeki boşluğu (varsa) görünür kılar — bu da C ile aynıdır. |
| R3 | `Utf8Carry::feed`'in ömür imzası çağrı yerlerinde borç çakışmasına yol açar | İmza `fn feed<'a>(&'a mut self, chunk: &'a [u8]) -> Result<&'a str>` olarak tasarlandı; dönen değer kullanıldıktan sonra NLL borcu bitirir. Tasarım bu yüzden `&'a` paylaşımlı; bir alternatif olarak carry'nin `&mut self` yerine sahiplik devretmesi gerekmez. |
| R4 | `staging` tamponu chunk'ı kopyaladığı için sıcak yolda ek maliyet | Kopya **yalnızca** bir karakter okuma sınırına denk geldiğinde (yani bir önceki okuma yarım karakterle bittiğinde) olur. Sınır durumu olağandışıdır; doğru davranış için kabul edilen maliyettir. |
| R5 | A5 `escape`'in davranışını değiştirdiği için genel API tüketicileri bozulur | Davranış **C'ye yaklaşıyor**, uzaklaşmıyor: C'den gelen uygulamalar için düzeltmedir. Rust'a özgü `escape`'i kullanan kod (varsa) çıktısının değiştiğini görecektir; bu, `CHANGELOG`/belge notu gerektirir. |
| R6 | `0x00` reddi, veri içinde NUL taşıyan meşru bir akışı kırar | Böyle bir XML zaten geçersizdir ve C de reddeder. Kırılan bir kullanım varsa o kullanım C ile zaten çalışmıyordu. |
| R7 | `Parser::finish()` yeni bir genel API; adı C'nin ölü parametresiyle karışabilir | Belgede D4'e açıkça atıf yapılır ve `finish`'in C'de etkisiz olduğu, burada kasıtlı olarak anlam kazandırıldığı yazılır. |

**Açık soru (uygulama planına taşınır):** `helper.rs::escape_size` genel API'de kalmalı mı, yoksa `#[deprecated]` mı edilmeli? Spec'in kararı: **kalır** ve `escaped_len`'e delege eder — çünkü `lib.rs:60`'ta dışa verilmiş bir addır ve kaldırmak genel API kırar.

---

## 10. Kapsam dışı

- **NodeRef / düğüm modeli (B):** `add_child_node`'nun canlandırılması, `deep_clone`, `StreamEvent::Stanza`'nın bağlı ağaç taşıması, attribute upsert/silme. A bunlara **dokunmaz**: A2 yalnızca `node_stack`'i kaldırıp `current`/`root` ayrımını kurar, parent bağlarını değiştirmez. A, B'ye bağımlı değildir ve B'den önce bitirilebilir.
  - **Not:** `stream.rs:151`'deki `root_rc.borrow().clone()` (madde 4c) B'nin işidir; A3'te `stream.rs::on_cdata` düzeltilir ama teslim biçimi değişmez.
- **Filtre ve IQ yönlendirme (C).**
- **Yeni entegrasyon API'leri (D), kripto (E), C ABI (F), modern katman (G).**
- **C reposunda değişiklik.** C yalnızca oracle'dır.
- **`ParserLimits`'in C ile hizalanması.** C'de limit yoktur (§1.6); bu Rust'a özgü bir güvenlik katmanıdır ve A4 onu kendi belgelenmiş sözleşmesine uydurur.
