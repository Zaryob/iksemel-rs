use iksemel::{
    ChatState, DomParser, IksNode, XMLNS_DISCO_INFO, XMLNS_DISCO_ITEMS, XMLNS_MUC, XMLNS_MUC_USER,
    XMLNS_PING, XMLNS_PUBSUB, XMLNS_PUBSUB_EVENT, attach_chat_state, build_chat_state,
    build_disco_info_query, build_disco_items_query, build_muc_join, build_muc_leave, build_ping,
    build_pong, build_pubsub_publish, build_pubsub_subscribe, build_pubsub_unsubscribe,
    extract_chat_state, extract_muc_status_codes, extract_pubsub_items, is_muc_presence, is_ping,
    parse_disco_info_response, parse_disco_items_response,
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
    assert_eq!(
        q_child.borrow().find_attrib("xmlns"),
        Some(XMLNS_DISCO_INFO)
    );
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
    assert_eq!(
        q_child.borrow().find_attrib("xmlns"),
        Some(XMLNS_DISCO_ITEMS)
    );
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

#[test]
fn test_xep_0045_muc_operations() {
    // Join room with password and history
    let join_presence = build_muc_join(
        "lounge@conference.example.org/alice",
        Some("secret123"),
        Some(500),
    );
    assert_eq!(join_presence.name(), Some("presence"));
    assert_eq!(
        join_presence.find_attrib("to"),
        Some("lounge@conference.example.org/alice")
    );
    assert!(is_muc_presence(&join_presence));

    let x = join_presence.find("x").expect("x element in muc join");
    assert_eq!(x.borrow().find_attrib("xmlns"), Some(XMLNS_MUC));
    assert_eq!(
        x.borrow().find("password").map(|p| p.borrow().text()),
        Some("secret123".to_string())
    );
    let history = x.borrow().find("history").expect("history element");
    assert_eq!(history.borrow().find_attrib("maxchars"), Some("500"));

    // Leave room
    let leave_presence =
        build_muc_leave("lounge@conference.example.org/alice", Some("Going offline"));
    assert_eq!(leave_presence.find_attrib("type"), Some("unavailable"));
    assert_eq!(
        leave_presence.find("status").map(|s| s.borrow().text()),
        Some("Going offline".to_string())
    );

    // Parse status codes from MUC#user presence
    let raw_presence = r#"<presence from="lounge@conference.example.org/alice" to="alice@example.org/laptop">
        <x xmlns="http://jabber.org/protocol/muc#user">
            <item affiliation="owner" role="moderator"/>
            <status code="110"/>
            <status code="201"/>
        </x>
    </presence>"#;

    let doc = DomParser::parse_str(raw_presence).expect("parse presence");
    assert!(is_muc_presence(&doc.borrow()));
    let x_user = doc.borrow().find("x").expect("x element");
    assert_eq!(x_user.borrow().find_attrib("xmlns"), Some(XMLNS_MUC_USER));

    let codes = extract_muc_status_codes(&doc.borrow());
    assert_eq!(codes, vec![110, 201]);
}

#[test]
fn test_xep_0060_pubsub_operations() {
    // Build publish stanza with payload
    let mut payload = IksNode::new_tag("entry");
    payload.add_attribute("xmlns", "http://www.w3.org/2005/Atom");
    let mut title = IksNode::new_tag("title");
    title.insert_cdata("Test Article");
    payload.add_child(title);

    let pub_iq = build_pubsub_publish(
        "pub1",
        "pubsub.example.com",
        "news",
        Some("item-999"),
        Some(payload),
    );
    assert_eq!(pub_iq.find_attrib("type"), Some("set"));
    assert_eq!(pub_iq.find_attrib("id"), Some("pub1"));
    assert_eq!(pub_iq.find_attrib("to"), Some("pubsub.example.com"));

    let pubsub = pub_iq.find("pubsub").expect("pubsub element");
    assert_eq!(pubsub.borrow().find_attrib("xmlns"), Some(XMLNS_PUBSUB));
    let publish = pubsub.borrow().find("publish").expect("publish element");
    assert_eq!(publish.borrow().find_attrib("node"), Some("news"));
    let item = publish.borrow().find("item").expect("item element");
    assert_eq!(item.borrow().find_attrib("id"), Some("item-999"));

    // Subscribe
    let sub_iq = build_pubsub_subscribe("sub1", "pubsub.example.com", "news", "alice@example.com");
    assert_eq!(sub_iq.find_attrib("id"), Some("sub1"));

    // Unsubscribe
    let unsub_iq =
        build_pubsub_unsubscribe("unsub1", "pubsub.example.com", "news", "alice@example.com");
    assert_eq!(unsub_iq.find_attrib("id"), Some("unsub1"));

    // Parse incoming event notification
    let raw_event = r#"<message from="pubsub.example.com" to="alice@example.com">
        <event xmlns="http://jabber.org/protocol/pubsub#event">
            <items node="weather">
                <item id="current">
                    <report temp="22" unit="C"/>
                </item>
            </items>
        </event>
    </message>"#;

    let event_doc = DomParser::parse_str(raw_event).expect("parse pubsub event");
    let event = event_doc.borrow().find("event").expect("event element");
    assert_eq!(
        event.borrow().find_attrib("xmlns"),
        Some(XMLNS_PUBSUB_EVENT)
    );

    let (node_name, items) =
        extract_pubsub_items(&event_doc.borrow()).expect("extract pubsub items");
    assert_eq!(node_name, "weather");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id.as_deref(), Some("current"));
    let report_payload = items[0].payload.as_ref().expect("payload exists");
    assert_eq!(report_payload.name(), Some("report"));
    assert_eq!(report_payload.find_attrib("temp"), Some("22"));
    assert_eq!(report_payload.find_attrib("unit"), Some("C"));
}
