use iksemel::{
    attach_chat_state, build_chat_state, build_disco_info_query, build_disco_items_query,
    build_ping, build_pong, extract_chat_state, is_ping, parse_disco_info_response,
    parse_disco_items_response, ChatState, DomParser, IksNode, XMLNS_DISCO_INFO,
    XMLNS_DISCO_ITEMS, XMLNS_PING,
};

#[test]
fn test_xep_0199_ping_and_pong() {
    let ping = build_ping("ping_123", Some("server.example.com"));
    assert_eq!(ping.name(), Some("iq"));
    assert_eq!(ping.find_attrib("type"), Some("get"));
    assert_eq!(ping.find_attrib("id"), Some("ping_123"));
    assert_eq!(ping.find_attrib("to"), Some("server.example.com"));

    let ping_child = ping.find("ping").expect("ping child element");
    assert_eq!(ping_child.borrow().find_attrib("xmlns"), Some(XMLNS_PING));

    assert!(is_ping(&ping));

    // Non-ping stanza must return false
    let not_ping = IksNode::new_tag("presence");
    assert!(!is_ping(&not_ping));

    // Build Pong response
    let mut incoming_ping = ping.clone();
    incoming_ping.add_attribute("from", "client@example.com/res");

    let pong = build_pong(&incoming_ping).expect("build pong");
    assert_eq!(pong.name(), Some("iq"));
    assert_eq!(pong.find_attrib("type"), Some("result"));
    assert_eq!(pong.find_attrib("id"), Some("ping_123"));
    assert_eq!(pong.find_attrib("to"), Some("client@example.com/res"));
}

#[test]
fn test_xep_0030_disco_info() {
    let query = build_disco_info_query("disco_1", "conference.example.org", Some("music_room"));
    assert_eq!(query.find_attrib("type"), Some("get"));
    assert_eq!(query.find_attrib("id"), Some("disco_1"));
    assert_eq!(query.find_attrib("to"), Some("conference.example.org"));

    let q_child = query.find("query").expect("query child");
    assert_eq!(q_child.borrow().find_attrib("xmlns"), Some(XMLNS_DISCO_INFO));
    assert_eq!(q_child.borrow().find_attrib("node"), Some("music_room"));

    // Parse full disco#info response
    let raw_response = r#"<iq from="conference.example.org" id="disco_1" type="result">
        <query xmlns="http://jabber.org/protocol/disco#info" node="music_room">
            <identity category="conference" name="Music Room" type="text"/>
            <identity category="pubsub" type="leaf"/>
            <feature var="http://jabber.org/protocol/muc"/>
            <feature var="urn:xmpp:ping"/>
            <feature var="http://jabber.org/protocol/disco#info"/>
        </query>
    </iq>"#;

    let parsed_doc = DomParser::parse_str(raw_response).expect("parse disco response");
    let info = parse_disco_info_response(&parsed_doc.borrow()).expect("parse disco info");

    assert_eq!(info.node, Some("music_room".to_string()));
    assert_eq!(info.identities.len(), 2);
    assert_eq!(info.identities[0].category, "conference");
    assert_eq!(info.identities[0].name, Some("Music Room".to_string()));
    assert_eq!(info.identities[0].type_name, "text");

    assert_eq!(info.features.len(), 3);
    assert!(info.has_feature("http://jabber.org/protocol/muc"));
    assert!(info.has_feature("urn:xmpp:ping"));
    assert!(!info.has_feature("jabber:iq:version"));
}

#[test]
fn test_xep_0030_disco_items() {
    let query = build_disco_items_query("items_1", "pubsub.example.com", None);
    assert_eq!(query.find_attrib("id"), Some("items_1"));
    assert_eq!(query.find_attrib("to"), Some("pubsub.example.com"));

    let q_child = query.find("query").expect("query child");
    assert_eq!(q_child.borrow().find_attrib("xmlns"), Some(XMLNS_DISCO_ITEMS));
    assert_eq!(q_child.borrow().find_attrib("node"), None);

    let raw_response = r#"<iq from="pubsub.example.com" id="items_1" type="result">
        <query xmlns="http://jabber.org/protocol/disco#items">
            <item jid="pubsub.example.com" name="News Feed" node="news"/>
            <item jid="pubsub.example.com" name="Weather Alerts" node="weather"/>
            <item jid="chat.example.com" name="General Chat"/>
        </query>
    </iq>"#;

    let parsed_doc = DomParser::parse_str(raw_response).expect("parse disco items response");
    let items = parse_disco_items_response(&parsed_doc.borrow()).expect("parse disco items");

    assert_eq!(items.items.len(), 3);
    assert_eq!(items.items[0].jid.domain(), "pubsub.example.com");
    assert_eq!(items.items[0].name, Some("News Feed".to_string()));
    assert_eq!(items.items[0].node, Some("news".to_string()));

    assert_eq!(items.items[2].jid.domain(), "chat.example.com");
    assert_eq!(items.items[2].node, None);
}

#[test]
fn test_xep_0085_chat_states() {
    let states = [
        ChatState::Active,
        ChatState::Composing,
        ChatState::Paused,
        ChatState::Inactive,
        ChatState::Gone,
    ];

    for &state in &states {
        let node = build_chat_state(state);
        assert_eq!(node.name(), Some(state.as_str()));
        assert_eq!(
            node.find_attrib("xmlns"),
            Some("http://jabber.org/protocol/chatstates")
        );
    }

    // Attach to message
    let mut msg = IksNode::new_tag("message");
    msg.add_attribute("to", "peer@example.com");
    msg.add_attribute("type", "chat");

    attach_chat_state(&mut msg, ChatState::Composing);

    let detected_state = extract_chat_state(&msg);
    assert_eq!(detected_state, Some(ChatState::Composing));

    // Message without chat state
    let empty_msg = IksNode::new_tag("message");
    assert_eq!(extract_chat_state(&empty_msg), None);
}
