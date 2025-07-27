use iksemel::{DomParser, Parser, Result, SaxHandler, TagType};

struct EventCounter {
    open_count: usize,
    close_count: usize,
    single_count: usize,
    cdata_bytes: usize,
}

impl EventCounter {
    fn new() -> Self {
        EventCounter {
            open_count: 0,
            close_count: 0,
            single_count: 0,
            cdata_bytes: 0,
        }
    }
}

impl SaxHandler for EventCounter {
    fn on_tag(&mut self, _name: &str, _attrs: &[(String, String)], tag_type: TagType) -> Result<()> {
        match tag_type {
            TagType::Open => self.open_count += 1,
            TagType::Close => self.close_count += 1,
            TagType::Single => self.single_count += 1,
        }
        Ok(())
    }

    fn on_cdata(&mut self, data: &str) -> Result<()> {
        self.cdata_bytes += data.len();
        Ok(())
    }
}

#[test]
fn test_deeply_nested_xml() {
    let depth = 500;
    let mut xml = String::with_capacity(depth * 30);
    for i in 0..depth {
        xml.push_str(&format!("<node_{}>", i));
    }
    xml.push_str("Deep payload");
    for i in (0..depth).rev() {
        xml.push_str(&format!("</node_{}>", i));
    }

    let handler = EventCounter::new();
    let mut parser = Parser::new(handler);
    parser.parse(&xml).expect("parse deep xml");
    parser.parse("").expect("flush parser");

    assert_eq!(parser.handler().open_count, depth);
    assert_eq!(parser.handler().close_count, depth);
    assert_eq!(parser.handler().cdata_bytes, "Deep payload".len());
}

#[test]
fn test_many_attributes_and_entities() {
    let mut xml = String::from("<root ");
    for i in 0..200 {
        xml.push_str(&format!("attr_{}=\"val_{} &amp; &lt;&gt; &quot;\" ", i, i));
    }
    xml.push_str("/>");

    let dom = DomParser::parse_str(&xml).expect("parse attributes");
    let root = dom.borrow();
    assert_eq!(root.attributes().len(), 200);
    assert_eq!(
        root.find_attrib("attr_42"),
        Some("val_42 & <> \"")
    );
}

#[test]
fn test_pathological_1byte_chunks() {
    let xml = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        "<!-- multi-line\ncomment\nwith - dashes -->",
        "<root lang=\"en\">",
        "  <title>Title &amp; Subtitle</title>",
        "  <![CDATA[Unescaped <data> inside CDATA]]>",
        "  <self_closing item=\"1\"/>",
        "</root>"
    );

    let handler = EventCounter::new();
    let mut parser = Parser::new(handler);

    // Feed in 1-byte chunks to stress parser state transitions
    for b in xml.bytes() {
        let byte_slice = [b];
        let single_char = std::str::from_utf8(&byte_slice).unwrap();
        parser.parse(single_char).expect("parse single byte chunk");
    }
    parser.parse("").expect("flush parser");

    assert_eq!(parser.handler().open_count, 2); // root, title
    assert_eq!(parser.handler().close_count, 2);
    assert_eq!(parser.handler().single_count, 1); // self_closing
}

#[test]
fn test_large_cdata_and_unicode_emojis() {
    let large_text = "🦀 Rust is fast! 🚀 Özel Türkçe karakterler: ğüşıöç ĞÜŞİÖÇ. ".repeat(200);
    let xml = format!("<doc><text>{}</text><![CDATA[{}]]></doc>", large_text, large_text);

    let dom = DomParser::parse_str(&xml).expect("parse unicode xml");
    let root = dom.borrow();

    assert_eq!(root.name(), Some("doc"));
    let text_child = root.find("text").expect("find text element");
    assert_eq!(
        text_child.borrow().children()[0].borrow().content(),
        Some(large_text.as_str())
    );
}
