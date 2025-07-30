use iksemel::{DomParser, IksNode, IksType, XmlWriter};
use std::io::Cursor;

#[test]
fn test_writer_and_to_string_equivalence() {
    let xml = r#"<message from="user@example.com" to="friend@example.com" type="chat"><subject>Greetings &amp; Salutations</subject><body>Hello &lt;world&gt;! "Test"</body><thread>12345</thread></message>"#;
    let node = DomParser::parse_str(xml).expect("Failed to parse XML");

    let to_string_out = node.borrow().to_string();

    let mut buf = Vec::new();
    node.borrow().write_to(&mut buf).expect("Failed to write node");
    let writer_out = String::from_utf8(buf).expect("Invalid UTF-8");

    assert_eq!(to_string_out, writer_out);
}

#[test]
fn test_pretty_printing_roundtrip() {
    let original_xml = r#"<iq from="client@example.com/res" id="roster_1" type="get"><query xmlns="jabber:iq:roster"><item jid="nurse@example.com" name="Nurse" subscription="both"><group>Hospital</group><group>Staff</group></item><item jid="doctor@example.com" name="Doctor" subscription="to"/></query></iq>"#;

    let parsed = DomParser::parse_str(original_xml).expect("parse original");
    let pretty2 = parsed.borrow().to_pretty_string(2);
    let pretty4 = parsed.borrow().to_pretty_string(4);

    assert!(pretty2.contains("  <query"));
    assert!(pretty2.contains("    <item"));
    assert!(pretty4.contains("    <query"));
    assert!(pretty4.contains("        <item"));

    // Round-trip re-parsing must produce an equivalent semantic DOM tree
    let re_parsed2 = DomParser::parse_str(&pretty2).expect("parse pretty2");
    let re_parsed4 = DomParser::parse_str(&pretty4).expect("parse pretty4");

    assert_eq!(re_parsed2.borrow().name(), Some("iq"));
    assert_eq!(re_parsed4.borrow().name(), Some("iq"));

    let q2 = re_parsed2.borrow().find("query").expect("query in parsed2");
    let q4 = re_parsed4.borrow().find("query").expect("query in parsed4");

    assert_eq!(q2.borrow().find_attrib("xmlns"), Some("jabber:iq:roster"));
    assert_eq!(q4.borrow().find_attrib("xmlns"), Some("jabber:iq:roster"));

    let items2: Vec<_> = q2.borrow().child_tags().into_iter().filter(|n| n.borrow().name() == Some("item")).collect();
    let items4: Vec<_> = q4.borrow().child_tags().into_iter().filter(|n| n.borrow().name() == Some("item")).collect();

    assert_eq!(items2.len(), 2);
    assert_eq!(items4.len(), 2);
    assert_eq!(items2[0].borrow().find_attrib("name"), Some("Nurse"));
    assert_eq!(items4[0].borrow().find_attrib("name"), Some("Nurse"));
    assert_eq!(items2[1].borrow().find_attrib("name"), Some("Doctor"));
    assert_eq!(items4[1].borrow().find_attrib("name"), Some("Doctor"));
}

#[test]
fn test_streaming_writer_into_cursor() {
    let mut cursor = Cursor::new(Vec::new());
    let mut writer = XmlWriter::new(&mut cursor);

    let mut root = IksNode::new_tag("bookstore");
    for i in 1..=50 {
        let mut book = IksNode::new_tag("book");
        book.add_attribute("id", format!("b{}", i));
        book.add_attribute("category", "fiction");

        let mut title = IksNode::new_tag("title");
        let mut title_cdata = IksNode::new(IksType::CData);
        title_cdata.set_content(format!("Novel Volume #{}", i));
        title.add_child(title_cdata);

        let mut price = IksNode::new_tag("price");
        let mut price_cdata = IksNode::new(IksType::CData);
        price_cdata.set_content(format!("{:.2}", 9.99 + (i as f64)));
        price.add_child(price_cdata);

        book.add_child(title);
        book.add_child(price);
        root.add_child(book);
    }

    writer.write_node(&root).expect("write failed");

    let bytes = cursor.into_inner();
    let text = String::from_utf8(bytes).expect("utf8 string");
    assert!(text.starts_with("<bookstore>"));
    assert!(text.ends_with("</bookstore>"));
    assert!(text.contains(r#"<book id="b1" category="fiction">"#));
    assert!(text.contains(r#"<book id="b50" category="fiction">"#));
    assert!(text.contains("<title>Novel Volume #50</title>"));

    // Verify parser can parse it back
    let reloaded = DomParser::parse_str(&text).expect("reload failed");
    let count = reloaded.borrow().child_tags().len();
    assert_eq!(count, 50);
}

#[test]
fn test_dom_traversal_queries() {
    let xml = r#"<catalog>
        <product id="p1" active="true">
            <name>Widget A</name>
            <tags>
                <tag>hardware</tag>
                <tag>tool</tag>
            </tags>
        </product>
        <product id="p2" active="false">
            <name>Widget B</name>
            <tags>
                <tag>software</tag>
            </tags>
        </product>
        <product id="p3" active="true">
            <name>Widget C</name>
            <tags>
                <tag>hardware</tag>
                <tag>gadget</tag>
            </tags>
        </product>
    </catalog>"#;

    let root = DomParser::parse_str(xml).expect("parse catalog");

    // Test find
    let first_prod = root.borrow().find("product").expect("find first product");
    assert_eq!(first_prod.borrow().find_attrib("id"), Some("p1"));

    // Test child_tags
    let all_prods: Vec<_> = root
        .borrow()
        .child_tags()
        .into_iter()
        .filter(|n| n.borrow().name() == Some("product"))
        .collect();
    assert_eq!(all_prods.len(), 3);

    // Filter active products
    let active_prods: Vec<_> = all_prods
        .iter()
        .filter(|p| p.borrow().find_attrib("active") == Some("true"))
        .collect();
    assert_eq!(active_prods.len(), 2);
    assert_eq!(active_prods[0].borrow().find_attrib("id"), Some("p1"));
    assert_eq!(active_prods[1].borrow().find_attrib("id"), Some("p3"));

    // Test find_with_attrib
    let p2 = root
        .borrow()
        .find_with_attrib(Some("product"), "id", "p2")
        .expect("find p2");
    assert_eq!(p2.borrow().find_attrib("active"), Some("false"));

    // Test text() helper
    let name_node = first_prod.borrow().find("name").expect("find name node");
    assert_eq!(name_node.borrow().text(), "Widget A");

    // Collect all tag texts across all products
    let mut tags = Vec::new();
    for prod in &all_prods {
        if let Some(tag_container) = prod.borrow().find("tags") {
            for tag in tag_container.borrow().child_tags() {
                if tag.borrow().name() == Some("tag") {
                    tags.push(tag.borrow().text());
                }
            }
        }
    }
    assert_eq!(tags, vec!["hardware", "tool", "software", "hardware", "gadget"]);
}

#[test]
fn test_dom_modification_and_cloning() {
    let mut root = IksNode::new_tag("root");
    let mut child1 = IksNode::new_tag("child");
    child1.add_attribute("idx", "1");
    let mut child2 = IksNode::new_tag("child");
    child2.add_attribute("idx", "2");

    root.add_child(child1);
    root.add_child(child2);

    let cloned = root.clone();
    assert_eq!(cloned.to_string(), root.to_string());

    // Verify children count
    let children = cloned.child_tags();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].borrow().find_attrib("idx"), Some("1"));
    assert_eq!(children[1].borrow().find_attrib("idx"), Some("2"));
}
