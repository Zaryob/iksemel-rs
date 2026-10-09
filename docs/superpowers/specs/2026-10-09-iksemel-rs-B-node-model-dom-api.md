# iksemel-rs — Alt proje B: Düğüm Modeli, NodeRef ve DOM Düzenleme API'leri

- **Tarih:** 2026-10-09
- **Durum:** Onay bekliyor. Uygulama planı bu spec onaylandıktan sonra yazılır.
- **Üst belge:** [`2026-10-09-iksemel-rs-c-parity-roadmap.md`](2026-10-09-iksemel-rs-c-parity-roadmap.md) §5.B
- **Referans commit'ler:** iksemel-rs `f943ae3` (Alt-proje A tamamlanmış merge base), iksemel (C) `5abdbce`
- **Kapsanan yol haritası kalemleri:** §2.1 satır **4a, 4b, 4c, 5** ve §2.2 **eksik DOM API'leri** (`iks_hide`, `iks_set_cdata`, `iks_append_cdata`, `iks_prepend_cdata`, `iks_insert_node`, attribute silme)

Bu spec'teki tüm C davranışları, C kaynak kodu (`/Users/zaryob/Development/iksemel/src/iks.c`) okunarak ve doğrulanarak tasarlanmıştır.

---

## 1. Doğrulanmış C Semantiği (Oracle)

C `iksemel` kütüphanesinde düğüm işlemleri `iks.c` içinde şu semantikle yürütülür:

### 1.1 `iks_insert_attrib` ve `iks_remove_attrib` (`iks.c:132-173`)
- `iks_insert_attrib(x, name, value)`:
  - Eğer `name` adlı attribute zaten varsa, değeri `value` ile **yerinde güncellenir** (`IKS_ATTRIB_VALUE(y) = strdup(value)`). Çift attribute oluşturulmaz (**upsert**).
  - Eğer yoksa listenin sonuna yeni attribute eklenir.
  - Eğer `value == NULL` ise, mevcut attribute düğümü ağaçtan sökülür ve silinir (`iks_hide` davranışı).
- Rust karşılığı:
  - `add_attribute(name, value)`: Varsa günceller, yoksa sona ekler (**upsert**).
  - `remove_attribute(name) -> bool`: Varsa siler ve `true` döner; yoksa `false` döner.

### 1.2 `iks_hide` (`iks.c:329-344`)
- Bir düğümü (`IKS_TAG` veya `IKS_CDATA`) bağlı olduğu parent ve sibling zincirinden söker:
  ```c
  if (x->prev) x->prev->next = x->next;
  if (x->next) x->next->prev = x->prev;
  if (x->parent) {
      if (x->parent->children == x) x->parent->children = x->next;
      if (x->parent->last_child == x) x->parent->last_child = x->prev;
  }
  ```
- Düğümün kendisi bellekte kalır ancak parent'ı ve komşuları güncellenir.
- Rust karşılığı: `NodeRef::hide(&self)` ve `IksNode::hide(&mut self)`.

### 1.3 `iks_set_cdata` (`iks.c:210-226`)
- `iks_set_cdata(x, data, len)`:
  - Önce hedef etiketin **tüm mevcut çocuklarını** döngüyle söker/gizler:
    ```c
    while (1) {
        y = iks_child(x);
        if (!y) break;
        iks_hide(y);
    }
    ```
  - Ardından yeni bir CDATA düğümü ekler.
- Rust karşılığı: `set_content(data)` ve `set_cdata(data)` mevcut tüm çocukları temizlemeli ve tek CDATA düğümü yerleştirmelidir.

### 1.4 `iks_append_cdata` ve `iks_prepend_cdata` (`iks.c:273-320`)
- `iks_append_cdata(x, data, len)`: `x` düğümünden **hemen sonra** kardeş CDATA ekler (`x->next` arasına yerleştirir).
- `iks_prepend_cdata(x, data, len)`: `x` düğümünden **hemen önce** kardeş CDATA ekler (`x->prev` arasına yerleştirir).
- Rust karşılığı: `NodeRef::append_cdata(&self, data)` ve `NodeRef::prepend_cdata(&self, data)`.

### 1.5 `iks_insert_node` (`iks.c:175-186`)
- `x` düğümünün son çocuğu olarak `y` düğümünü ekler, `y->parent = x` bağını kurar.
- Rust karşılığı: `NodeRef::add_child(&self, child)` ve `NodeRef::insert_node(&self, child)`.

---

## 2. Mimari Tasarım (`NodeRef`)

### 2.1 Temel Sorun (Yol Haritası §4 Karar 1)
Rust'ta `IksNode` bir değer tipi (`struct`) olarak taşındığında:
- Değer olarak oluşturulmuş bir düğümün (`let mut n = IksNode::new_tag("a")`) `Rc` kimliği yoktur.
- `n.add_child(c)` çağrıldığında `c.parent = Some(Weak)` kurulamaz çünkü `n`'e ait bir `Rc` mevcut değildir (`as_rc()` `None` döner).
- `deep_clone()` değer klonladığı için torunların parent ve sibling bağları kopar.
- `StreamEvent::Stanza(IksNode)` stanzayı değer olarak klonladığı için teslim edilen stanza çocuklarında `parent()` her zaman `None` döner; dolayısıyla namespace mirası ve sibling navigasyonu çalışmaz.

### 2.2 Çözüm: `NodeRef` Sarmalayıcısı
Ağaç sahipliği `NodeRef` üzerinden zorunlu kılınır:

```rust
#[derive(Clone, Debug)]
pub struct NodeRef(pub Rc<RefCell<IksNode>>);

impl PartialEq for NodeRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || *self.0.borrow() == *other.0.borrow()
    }
}
```

- `NodeRef` her zaman `Rc<RefCell<IksNode>>` taşır. Dolayısıyla `self` her zaman bir `Rc`'dir.
- `NodeRef::new_tag(name)` ve `NodeRef::new(type)` doğrudan `NodeRef` üretir; iç `IksNode`'un `self_ref` alanı otomatik kurulur.
- `NodeRef::add_child(&self, child: impl Into<NodeRef>) -> NodeRef`:
  - `child`'ın `parent` bağı **koşulsuz ve garantili** olarak `Rc::downgrade(&self.0)` yapılır.
  - Sibling bağları (`prev`, `next`) kurulur.
  - Eklenen çocuk `NodeRef` olarak döner.
- `NodeRef::clone_subtree(&self) -> NodeRef`:
  - Ağacın tam bir kopyasını çıkarır; tüm yeni düğümler `NodeRef` olarak kurulur ve **tüm parent/prev/next bağları kopmaksızın yeni ağaca bağlanır**.
- `StreamEvent::Stanza` artık `NodeRef` taşır:
  ```rust
  pub enum StreamEvent {
      StreamStart(NodeRef),
      Stanza(NodeRef),
      StreamEnd,
  }
  ```
- `DomParser::parse_str(xml) -> Result<NodeRef>` ve `DomParser::document(&self) -> Option<NodeRef>` doğrudan `NodeRef` döndürür.

---

## 3. `NodeRef` API Yüzeyi

`NodeRef`, `IksNode`'un tüm okuma ve manipülasyon işlemlerine ergonomik yönlendiriciler sunar:

```rust
impl NodeRef {
    // Kurucular
    pub fn new(node_type: IksType) -> Self;
    pub fn new_tag(name: impl Into<String>) -> Self;
    pub fn new_cdata(data: impl Into<String>) -> Self;

    // İç erişim
    pub fn borrow(&self) -> std::cell::Ref<'_, IksNode>;
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, IksNode>;
    pub fn as_rc(&self) -> &Rc<RefCell<IksNode>>;

    // Temel sorgular
    pub fn node_type(&self) -> IksType;
    pub fn name(&self) -> Option<String>;
    pub fn prefix(&self) -> Option<String>;
    pub fn local_name(&self) -> Option<String>;
    pub fn text(&self) -> String;

    // Attribute işlemleri
    pub fn add_attribute(&self, name: impl Into<String>, value: impl Into<String>); // Upsert!
    pub fn remove_attribute(&self, name: &str) -> bool;
    pub fn find_attrib(&self, name: &str) -> Option<String>;
    pub fn has_attribute(&self, name: &str) -> bool;
    pub fn attributes(&self) -> Vec<(String, String)>;

    // Çocuk ve Kardeş Ekleme / Çıkarma
    pub fn add_child(&self, child: impl Into<NodeRef>) -> NodeRef;
    pub fn insert_node(&self, child: impl Into<NodeRef>) -> NodeRef;
    pub fn insert_cdata(&self, text: impl Into<String>) -> NodeRef;
    pub fn set_content(&self, text: impl Into<String>); // Çocukları siler + cdata ekler
    pub fn set_cdata(&self, text: impl Into<String>);   // set_content alias
    pub fn append_cdata(&self, text: impl Into<String>) -> Option<NodeRef>;
    pub fn prepend_cdata(&self, text: impl Into<String>) -> Option<NodeRef>;
    pub fn insert_sibling(&self, name: impl Into<String>) -> Option<NodeRef>;
    pub fn insert_before(&self, name: impl Into<String>) -> Option<NodeRef>;
    pub fn hide(&self); // Ağaçtan söker

    // Navigasyon
    pub fn parent(&self) -> Option<NodeRef>;
    pub fn root(&self) -> NodeRef;
    pub fn first_child(&self) -> Option<NodeRef>;
    pub fn first_tag(&self) -> Option<NodeRef>;
    pub fn last_child(&self) -> Option<NodeRef>;
    pub fn next(&self) -> Option<NodeRef>;
    pub fn prev(&self) -> Option<NodeRef>;
    pub fn next_tag(&self) -> Option<NodeRef>;
    pub fn prev_tag(&self) -> Option<NodeRef>;
    pub fn children(&self) -> Vec<NodeRef>;
    pub fn child_tags(&self) -> Vec<NodeRef>;
    pub fn has_children(&self) -> bool;

    // Arama ve Seçici
    pub fn find(&self, name: &str) -> Option<NodeRef>;
    pub fn find_all(&self, name: &str) -> Vec<NodeRef>;
    pub fn find_cdata(&self, name: &str) -> Option<String>;
    pub fn find_path(&self, path: &[&str]) -> Option<NodeRef>;
    pub fn find_path_text(&self, path: &[&str]) -> Option<String>;
    pub fn select(&self, query: &str) -> Vec<NodeRef>;

    // Klonlama ve Serileştirme
    pub fn clone_subtree(&self) -> NodeRef;
    pub fn to_pretty_string(&self, indent: usize) -> String;
    pub fn write_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()>;
}

impl std::fmt::Display for NodeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.borrow())
    }
}

impl From<Rc<RefCell<IksNode>>> for NodeRef {
    fn from(rc: Rc<RefCell<IksNode>>) -> Self {
        NodeRef(rc)
    }
}

impl From<IksNode> for NodeRef {
    fn from(node: IksNode) -> Self {
        node.into_rc().into()
    }
}
```

---

## 4. `IksNode` Değişiklikleri

`IksNode` iç veri yapısı olarak kalır, ancak:
1. `add_attribute`: Sona ekleme yerine `upsert` yapar (mevcut `name` varsa değerini günceller).
2. `remove_attribute(name: &str) -> bool`: Varsa siler, `true` döner.
3. `hide(&mut self)`: Kendisini `parent`, `prev`, `next` bağlarından güvenle ayırır.
4. `set_content(&mut self, content: impl Into<String>)`: Önce `children.clear()` yapar, ardından `content` ayarlar (veya CDATA çocuğu ekler).
5. `add_child`: `#[deprecated(note = "Use NodeRef::add_child to guarantee parent links")]` ile işaretlenir, ancak derlenebilirliği korumak için `as_rc()` üzerinden çalışmaya devam eder.

---

## 5. Göç Planı ve Kapsam

### 5.1 Etkilenecek Dosyalar
- `src/lib.rs`: `NodeRef` struct ve impl, `IksNode` güncellemeleri (`upsert`, `remove_attribute`, `hide`), re-exportlar.
- `src/stream.rs`: `StreamEvent::StreamStart(NodeRef)`, `StreamEvent::Stanza(NodeRef)`, `StreamDispatcher` entegrasyonu.
- `src/dom.rs`: `DomParser::parse_str`, `parse_str_with_limits`, `load_file`, `document` metotlarının `NodeRef` döndürmesi.
- `src/net.rs`: `recv_stanza` ve `recv_event` dönüşlerinin `NodeRef`'e uyarlanması.
- `src/async_net.rs`: `recv_stanza` ve `recv_event` dönüşlerinin `NodeRef`'e uyarlanması.
- `src/xep.rs`: XML inşasında `NodeRef::new_tag` kullanımı.
- `src/roster.rs`: `NodeRef` uyumu.
- `tests/*`: `StreamEvent::Stanza` ve `DomParser::parse_str` çağrı yerlerinin güncellenmesi, yeni `NodeRef` testleri.

---

## 6. Kabul Kriterleri ve Doğrulama

1. **Bağlantı Bütünlüğü (4a, 4b, 4c):**
   - `NodeRef::new_tag("r")` kökünden türetilen herhangi bir çocuğun `child.parent()` çağrısı her zaman `Some(root)` döner.
   - `clone_subtree()` ile klonlanan ağacın torunlarında `parent()` bağları eksiksiz mevcuttur.
   - `StreamEvent::Stanza` üzerinden alınan bir stanza'nın torunlarında `parent()` ve `resolve_namespace()` doğru çalışır.
2. **Attribute Paritesi (5):**
   - `node.add_attribute("a", "1"); node.add_attribute("a", "2");` sonrası attribute sayısı 1'dir ve değeri `"2"`'dir.
   - `node.remove_attribute("a")` `true` döner ve arandığında `None` verir.
3. **Eksik DOM API'leri:**
   - `node.hide()` sonrası kardeşler birbirine bağlanır, parent'ın çocuk listesinden düğüm kalkar.
   - `node.set_content("foo")` eski tüm çocukları temizler.
   - `node.append_cdata("bar")` ve `node.prepend_cdata("baz")` doğru sıralamayla kardeş ekler.
4. **Kalite Kriterleri:**
   - `cargo test --all-features` sıfır hatayla geçer.
   - `cargo clippy --all-targets --all-features` sıfır uyarıyla geçer.
   - `#![forbid(unsafe_code)]` kuralı korunur.
