use iksemel::{
    CarbonMessage, DomParser, IksNode, MamQuery, XMLNS_CARBONS, XMLNS_MAM, XMLNS_RSM,
    build_carbons_disable, build_carbons_enable, extract_carbon, extract_mam_result,
    mark_carbon_private, parse_mam_fin, wrap_carbon_received, wrap_carbon_sent,
};

#[test]
fn test_xep_0280_carbons_enable_disable_and_private() {
    let enable_iq = build_carbons_enable("carb_1");
    assert_eq!(enable_iq.find_attrib("type").unwrap(), "set");
    assert_eq!(enable_iq.find_attrib("id").unwrap(), "carb_1");

    let enable_child = enable_iq.first_child_tag().unwrap();
    assert_eq!(enable_child.borrow().name().unwrap(), "enable");
    assert_eq!(
        enable_child.borrow().find_attrib("xmlns").unwrap(),
        XMLNS_CARBONS
    );

    let disable_iq = build_carbons_disable("carb_2");
    assert_eq!(disable_iq.find_attrib("type").unwrap(), "set");
    assert_eq!(disable_iq.find_attrib("id").unwrap(), "carb_2");

    let disable_child = disable_iq.first_child_tag().unwrap();
    assert_eq!(disable_child.borrow().name().unwrap(), "disable");
    assert_eq!(
        disable_child.borrow().find_attrib("xmlns").unwrap(),
        XMLNS_CARBONS
    );

    let mut message = IksNode::new_tag("message");
    mark_carbon_private(&mut message);
    let private_child = message.find("private").unwrap();
    assert_eq!(
        private_child.borrow().find_attrib("xmlns").unwrap(),
        XMLNS_CARBONS
    );
}

#[test]
fn test_xep_0280_carbons_wrap_and_extract() {
    // 1. Sent carbon
    let mut sent_payload = IksNode::new_tag("message");
    sent_payload.add_attribute("to", "bob@example.com");
    sent_payload.add_attribute("type", "chat");
    let mut sent_body = IksNode::new_tag("body");
    sent_body.insert_cdata("Sent from my mobile client");
    sent_payload.add_child(sent_body);

    let sent_wrapper = wrap_carbon_sent(&sent_payload);
    let mut outer_sent_msg = IksNode::new_tag("message");
    outer_sent_msg.add_attribute("from", "alice@example.com/mobile");
    outer_sent_msg.add_child(sent_wrapper);

    let extracted_sent = extract_carbon(&outer_sent_msg).expect("Should extract sent carbon");
    match extracted_sent {
        CarbonMessage::Sent(inner) => {
            assert_eq!(inner.find_attrib("to").unwrap(), "bob@example.com");
            assert_eq!(
                inner.find_path_text(&["body"]).unwrap(),
                "Sent from my mobile client"
            );
        }
        CarbonMessage::Received(_) => panic!("Expected Sent carbon"),
    }

    // 2. Received carbon
    let mut recv_payload = IksNode::new_tag("message");
    recv_payload.add_attribute("from", "bob@example.com");
    recv_payload.add_attribute("type", "chat");
    let mut recv_body = IksNode::new_tag("body");
    recv_body.insert_cdata("Incoming message to desktop");
    recv_payload.add_child(recv_body);

    let recv_wrapper = wrap_carbon_received(&recv_payload);
    let mut outer_recv_msg = IksNode::new_tag("message");
    outer_recv_msg.add_attribute("from", "alice@example.com");
    outer_recv_msg.add_child(recv_wrapper);

    let extracted_recv = extract_carbon(&outer_recv_msg).expect("Should extract received carbon");
    match extracted_recv {
        CarbonMessage::Received(inner) => {
            assert_eq!(inner.find_attrib("from").unwrap(), "bob@example.com");
            assert_eq!(
                inner.find_path_text(&["body"]).unwrap(),
                "Incoming message to desktop"
            );
        }
        CarbonMessage::Sent(_) => panic!("Expected Received carbon"),
    }

    // 3. Normal message returns None
    let normal_msg = IksNode::new_tag("message");
    assert!(extract_carbon(&normal_msg).is_none());
}

#[test]
fn test_xep_0313_mam_query_builder() {
    let query = MamQuery::new()
        .with_query_id("qid-101")
        .with_jid("contact@example.com")
        .with_start("2026-02-01T00:00:00Z")
        .with_end("2026-02-08T00:00:00Z")
        .with_rsm_max(50)
        .with_rsm_after("item-499");

    let iq = query.to_iq("mam_req_1");
    assert_eq!(iq.find_attrib("type").unwrap(), "set");
    assert_eq!(iq.find_attrib("id").unwrap(), "mam_req_1");

    let query_node_rc = iq.find("query").expect("Should contain query");
    let query_node = query_node_rc.borrow();
    assert_eq!(query_node.find_attrib("xmlns").unwrap(), XMLNS_MAM);
    assert_eq!(query_node.find_attrib("queryid").unwrap(), "qid-101");

    // Check Data Form (XEP-0004)
    let form_node_rc = query_node.find("x").expect("Should contain data form");
    let form_node = form_node_rc.borrow();
    assert_eq!(form_node.find_attrib("type").unwrap(), "submit");

    // Check RSM (XEP-0059)
    let rsm_node_rc = query_node.find("set").expect("Should contain RSM set");
    let rsm_node = rsm_node_rc.borrow();
    assert_eq!(rsm_node.find_attrib("xmlns").unwrap(), XMLNS_RSM);
    assert_eq!(rsm_node.find_path_text(&["max"]).unwrap(), "50");
    assert_eq!(rsm_node.find_path_text(&["after"]).unwrap(), "item-499");
}

#[test]
fn test_xep_0313_mam_result_extraction() {
    let xml = r#"
    <message to="alice@example.com/desktop" from="alice@example.com">
        <result xmlns="urn:xmpp:mam:2" queryid="qid-101" id="archived-msg-999">
            <forwarded xmlns="urn:xmpp:forward:0">
                <delay xmlns="urn:xmpp:delay" stamp="2026-02-08T09:30:00Z"/>
                <message to="alice@example.com" from="bob@example.com" type="chat">
                    <body>History message from archive</body>
                </message>
            </forwarded>
        </result>
    </message>
    "#;

    let doc = DomParser::parse_str(xml).expect("Should parse message XML");
    let root = doc.borrow();

    let result = extract_mam_result(&root).expect("Should extract MAM result");
    assert_eq!(result.query_id, Some("qid-101".to_string()));
    assert_eq!(result.id, "archived-msg-999");
    assert_eq!(result.timestamp, Some("2026-02-08T09:30:00Z".to_string()));
    assert_eq!(result.message.name().unwrap(), "message");
    assert_eq!(
        result.message.find_path_text(&["body"]).unwrap(),
        "History message from archive"
    );
}

#[test]
fn test_xep_0313_mam_fin_parsing() {
    let xml = r#"
    <iq type="result" id="mam_req_1" to="alice@example.com/desktop">
        <fin xmlns="urn:xmpp:mam:2" complete="true">
            <set xmlns="http://jabber.org/protocol/rsm">
                <first>msg-001</first>
                <last>msg-050</last>
                <count>350</count>
            </set>
        </fin>
    </iq>
    "#;

    let doc = DomParser::parse_str(xml).expect("Should parse IQ XML");
    let root = doc.borrow();

    let fin = parse_mam_fin(&root).expect("Should parse MAM fin");
    assert!(fin.complete);
    assert_eq!(fin.first, Some("msg-001".to_string()));
    assert_eq!(fin.last, Some("msg-050".to_string()));
    assert_eq!(fin.count, Some(350));
}
