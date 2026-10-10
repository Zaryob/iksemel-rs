//! C oracle ile kaçış paritesi.

use iksemel::{DomParser, IksNode, XmlWriter, escape, escape_cow, escape_size};

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
    for s in [
        "",
        "plain",
        "café ü",
        "€ 😀",
        "&<>'\"",
        "\u{01}\u{7f}",
        "a\0b",
    ] {
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
