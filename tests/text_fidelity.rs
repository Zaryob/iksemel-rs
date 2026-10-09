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

