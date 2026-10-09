use iksemel::{DomParser, IksType, NodeRef, StreamEvent, StreamParser};

/// 4a: Herhangi bir NodeRef kökünden eklenen çocuk düğümlerin parent bağı garantilenmelidir.
#[test]
fn test_parity_4a_parent_link_guaranteed_from_root() {
    let root = NodeRef::new_tag("root");
    let child = root.add_child(NodeRef::new_tag("child"));
    let grandchild = child.add_child(NodeRef::new_tag("grandchild"));

    assert_eq!(child.parent(), Some(root.clone()));
    assert_eq!(grandchild.parent(), Some(child.clone()));
    assert_eq!(grandchild.root(), root);
    assert_eq!(child.root(), root);
    assert_eq!(root.root(), root);
    assert_eq!(root.parent(), None);
}

/// 4b: `clone_subtree` ile klonlanan alt ağaçta parent ve sibling bağları eksiksiz yeniden kurulmalıdır.
#[test]
fn test_parity_4b_clone_subtree_preserves_tree_links() {
    let root = NodeRef::new_tag("root");
    let _child1 = root.add_child(NodeRef::new_tag("c1"));
    let child2 = root.add_child(NodeRef::new_tag("c2"));
    let grandchild = child2.add_child(NodeRef::new_tag("gc"));

    assert_eq!(grandchild.parent(), Some(child2.clone()));

    let cloned_root = root.clone_subtree();
    assert_eq!(cloned_root.parent(), None);
    assert_eq!(cloned_root.children().len(), 2);

    let cloned_c1 = cloned_root.first_child().expect("cloned c1");
    let cloned_c2 = cloned_c1.next().expect("cloned c2");

    assert_eq!(cloned_c1.name().as_deref(), Some("c1"));
    assert_eq!(cloned_c2.name().as_deref(), Some("c2"));
    assert_eq!(cloned_c1.parent(), Some(cloned_root.clone()));
    assert_eq!(cloned_c2.parent(), Some(cloned_root.clone()));
    assert_eq!(cloned_c2.prev(), Some(cloned_c1.clone()));

    let cloned_gc = cloned_c2.first_child().expect("cloned gc");
    assert_eq!(cloned_gc.name().as_deref(), Some("gc"));
    assert_eq!(cloned_gc.parent(), Some(cloned_c2.clone()));
    assert_eq!(cloned_gc.root(), cloned_root);

    // Klonlanan ağaç orijinalinden bağımsız olmalıdır (referans eşitliği yok)
    assert_ne!(cloned_gc.parent(), Some(child2));
}

/// 4c: `StreamEvent::Stanza` üzerinden alınan stanza çocuklarında parent ve sibling bağları korunmalıdır.
#[test]
fn test_parity_4c_stream_event_stanza_node_ref_links() {
    let mut parser = StreamParser::new();
    let _ = parser
        .parse_chunk(
            "<stream:stream xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams'>",
        )
        .expect("stream header");

    let events = parser
        .parse_chunk(
            "<message to='user@example.com' id='m1'><body id='b1'>Hello</body><extra/></message>",
        )
        .expect("stanza chunk");

    assert_eq!(events.len(), 1);
    match &events[0] {
        StreamEvent::Stanza(stanza) => {
            assert_eq!(stanza.name().as_deref(), Some("message"));
            assert_eq!(stanza.find_attrib("id").as_deref(), Some("m1"));

            let body = stanza.find("body").expect("body node");
            let extra = stanza.find("extra").expect("extra node");

            assert_eq!(body.parent(), Some(stanza.clone()));
            assert_eq!(extra.parent(), Some(stanza.clone()));
            assert_eq!(body.next_tag(), Some(extra.clone()));
            assert_eq!(extra.prev_tag(), Some(body.clone()));
            assert_eq!(body.root(), *stanza);
        }
        _ => panic!("Expected Stanza"),
    }
}

/// 5: Attribute upsert ve remove attribute (C iksemel paritesi)
#[test]
fn test_parity_5_attribute_upsert_and_removal() {
    let node = NodeRef::new_tag("item");

    // Upsert testi: aynı isme tekrar atama güncellemelidir
    node.add_attribute("key", "val1");
    assert_eq!(node.find_attrib("key").as_deref(), Some("val1"));
    assert_eq!(node.attributes().len(), 1);

    node.add_attribute("key", "val2");
    assert_eq!(node.find_attrib("key").as_deref(), Some("val2"));
    assert_eq!(node.attributes().len(), 1);

    node.add_attribute("other", "abc");
    assert_eq!(node.attributes().len(), 2);

    // Silme testi
    assert!(node.remove_attribute("key"));
    assert_eq!(node.find_attrib("key"), None);
    assert_eq!(node.attributes().len(), 1);
    assert_eq!(node.find_attrib("other").as_deref(), Some("abc"));

    // Olmayan anahtar silinemez
    assert!(!node.remove_attribute("nonexistent"));
}

/// Eksik DOM API'leri: `hide()` ile ağaçtan ayrılma ve kardeş bağlarının yeniden kurulması
#[test]
fn test_parity_dom_hide() {
    let root = NodeRef::new_tag("root");
    let c1 = root.add_child(NodeRef::new_tag("c1"));
    let c2 = root.add_child(NodeRef::new_tag("c2"));
    let c3 = root.add_child(NodeRef::new_tag("c3"));

    assert_eq!(root.children().len(), 3);
    assert_eq!(c1.next(), Some(c2.clone()));
    assert_eq!(c2.next(), Some(c3.clone()));
    assert_eq!(c3.prev(), Some(c2.clone()));

    // c2'yi gizle/ayır
    c2.hide();

    assert_eq!(root.children().len(), 2);
    assert_eq!(c1.next(), Some(c3.clone()));
    assert_eq!(c3.prev(), Some(c1.clone()));
    assert_eq!(c2.parent(), None);
    assert_eq!(c2.next(), None);
    assert_eq!(c2.prev(), None);
}

/// Eksik DOM API'leri: `set_content` (iks_set_cdata) çocukları temizler
#[test]
fn test_parity_dom_set_content_clears_children() {
    let parent = NodeRef::new_tag("parent");
    parent.add_child(NodeRef::new_tag("c1"));
    parent.add_child(NodeRef::new_tag("c2"));
    assert_eq!(parent.children().len(), 2);

    parent.set_content("replaced-text");
    assert_eq!(parent.children().len(), 0);
    assert_eq!(parent.text(), "replaced-text");
}

/// Eksik DOM API'leri: `append_cdata` ve `prepend_cdata`
#[test]
fn test_parity_dom_append_and_prepend_cdata() {
    let parent = NodeRef::new_tag("parent");
    let mid = parent.add_child(NodeRef::new_tag("mid"));

    let before = mid.prepend_cdata("before-mid").expect("prepend");
    let after = mid.append_cdata("after-mid").expect("append");

    assert_eq!(before.parent(), Some(parent.clone()));
    assert_eq!(after.parent(), Some(parent.clone()));
    assert_eq!(before.next(), Some(mid.clone()));
    assert_eq!(mid.next(), Some(after.clone()));
    assert_eq!(after.prev(), Some(mid.clone()));
    assert_eq!(mid.prev(), Some(before.clone()));

    assert_eq!(before.node_type(), IksType::CData);
    assert_eq!(after.node_type(), IksType::CData);
}

/// DomParser entegrasyonu: NodeRef dönüşü ve gezinme
#[test]
fn test_parity_dom_parser_returns_noderef() {
    let xml = r#"<iq type="get" id="disco1"><query xmlns="http://jabber.org/protocol/disco#info"><identity category="client" type="pc" name="Exodus"/></query></iq>"#;
    let dom = DomParser::parse_str(xml).expect("parse str");

    assert_eq!(dom.name().as_deref(), Some("iq"));
    let query = dom.find("query").expect("query");
    assert_eq!(query.parent(), Some(dom.clone()));

    let identity = query.find("identity").expect("identity");
    assert_eq!(identity.parent(), Some(query.clone()));
    assert_eq!(identity.root(), dom);
    assert_eq!(identity.find_attrib("category").as_deref(), Some("client"));
}

/// Eksik DOM API'leri: `hide()` ilk (head) ve son (tail) eleman üzerinde
#[test]
fn test_parity_dom_hide_head_and_tail() {
    let root = NodeRef::new_tag("root");
    let c1 = root.add_child(NodeRef::new_tag("c1"));
    let c2 = root.add_child(NodeRef::new_tag("c2"));
    let c3 = root.add_child(NodeRef::new_tag("c3"));

    // Head'i gizle (c1)
    c1.hide();
    assert_eq!(root.children().len(), 2);
    assert_eq!(c2.prev(), None);
    assert_eq!(c2.next(), Some(c3.clone()));
    assert_eq!(c1.parent(), None);

    // Tail'i gizle (c3)
    c3.hide();
    assert_eq!(root.children().len(), 1);
    assert_eq!(c2.next(), None);
    assert_eq!(c2.prev(), None);
    assert_eq!(c3.parent(), None);
}

/// Eksik DOM API'leri: `hide()` tek başına olan çocuk üzerinde
#[test]
fn test_parity_dom_hide_lone_child() {
    let root = NodeRef::new_tag("root");
    let child = root.add_child(NodeRef::new_tag("child"));
    assert_eq!(root.children().len(), 1);

    child.hide();
    assert_eq!(root.children().len(), 0);
    assert_eq!(child.parent(), None);
    assert_eq!(child.prev(), None);
    assert_eq!(child.next(), None);
}

/// Eksik DOM API'leri: `append_cdata` ve `prepend_cdata` sınır durumları
#[test]
fn test_parity_dom_append_prepend_boundary() {
    let root = NodeRef::new_tag("root");
    let first = root.add_child(NodeRef::new_tag("first"));
    let last = root.add_child(NodeRef::new_tag("last"));

    // İlk elemanın önüne prepend
    let pre_first = first.prepend_cdata("before-first").expect("prepend first");
    assert_eq!(pre_first.parent(), Some(root.clone()));
    assert_eq!(pre_first.next(), Some(first.clone()));
    assert_eq!(first.prev(), Some(pre_first.clone()));

    // Son elemanın arkasına append
    let post_last = last.append_cdata("after-last").expect("append last");
    assert_eq!(post_last.parent(), Some(root.clone()));
    assert_eq!(last.next(), Some(post_last.clone()));
    assert_eq!(post_last.prev(), Some(last.clone()));
}

/// Seçiciler (`find_path`, `select`, `child_tags`) üzerinden erişilen düğümlerde parent bağı
#[test]
fn test_parity_dom_selectors_preserve_parent_links() {
    let xml = "<a id='1'><b id='2'><c id='3'>text</c></b></a>";
    let dom = DomParser::parse_str(xml).expect("parse");

    let c = dom.find_path(&["b", "c"]).expect("path b/c");
    assert_eq!(c.name().as_deref(), Some("c"));
    assert_eq!(c.parent().expect("b").name().as_deref(), Some("b"));
    assert_eq!(c.root(), dom);

    let selected = dom.select("b/c");
    assert_eq!(selected.len(), 1);
    assert_eq!(
        selected[0]
            .parent()
            .expect("parent of selected c")
            .name()
            .as_deref(),
        Some("b")
    );
}

/// Derin hiyerarşide `clone_subtree` parent bağlarının tam korunumu
#[test]
fn test_parity_clone_subtree_deep_hierarchy() {
    let root = NodeRef::new_tag("level0");
    let l1 = root.add_child(NodeRef::new_tag("level1"));
    let l2 = l1.add_child(NodeRef::new_tag("level2"));
    let l3 = l2.add_child(NodeRef::new_tag("level3"));
    l3.insert_cdata("deep-content");

    let clone = root.clone_subtree();
    let cl1 = clone.first_tag().expect("l1");
    let cl2 = cl1.first_tag().expect("l2");
    let cl3 = cl2.first_tag().expect("l3");

    assert_eq!(cl3.text(), "deep-content");
    assert_eq!(cl3.parent(), Some(cl2.clone()));
    assert_eq!(cl2.parent(), Some(cl1.clone()));
    assert_eq!(cl1.parent(), Some(clone.clone()));
    assert_eq!(clone.parent(), None);
    assert_eq!(cl3.root(), clone);
}
