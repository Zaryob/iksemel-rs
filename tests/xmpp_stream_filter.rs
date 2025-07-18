use iksemel::{Jid, PacketFilter, RuleBuilder, StanzaType, StreamEvent, StreamParser};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn test_stream_chunked_parsing_and_filtering() {
    let mut parser = StreamParser::new();
    let mut filter = PacketFilter::new();

    let iq_count = Arc::new(AtomicUsize::new(0));
    let msg_count = Arc::new(AtomicUsize::new(0));
    let presence_count = Arc::new(AtomicUsize::new(0));
    let roster_count = Arc::new(AtomicUsize::new(0));

    let iq_c = iq_count.clone();
    filter.add_rule(
        RuleBuilder::new()
            .with_type(StanzaType::Iq)
            .with_id("iq_roster_1")
            .with_subtype("result"),
        move |stanza| {
            iq_c.fetch_add(1, Ordering::SeqCst);
            assert_eq!(stanza.find_attrib("id"), Some("iq_roster_1"));
            true
        },
    );

    let roster_c = roster_count.clone();
    filter.add_rule(
        RuleBuilder::new()
            .with_ns("jabber:iq:roster"),
        move |stanza| {
            roster_c.fetch_add(1, Ordering::SeqCst);
            let query = stanza.find("query").expect("query tag present");
            assert_eq!(query.borrow().find_attrib("xmlns"), Some("jabber:iq:roster"));
            true
        },
    );

    let msg_c = msg_count.clone();
    filter.add_rule(
        RuleBuilder::new()
            .with_type(StanzaType::Message)
            .with_subtype("chat"),
        move |stanza| {
            msg_c.fetch_add(1, Ordering::SeqCst);
            let body = stanza.find_cdata("body").unwrap();
            assert_eq!(body, "Hello from Alice!");
            true
        },
    );

    let pres_c = presence_count.clone();
    let alice_jid = Jid::new("alice@example.com").unwrap();
    filter.add_rule(
        RuleBuilder::new()
            .with_type(StanzaType::Presence)
            .with_from(alice_jid),
        move |stanza| {
            pres_c.fetch_add(1, Ordering::SeqCst);
            let show = stanza.find_cdata("show").unwrap();
            assert_eq!(show, "chat");
            true
        },
    );

    let stream_xml = concat!(
        "<stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' to='example.com' version='1.0'>",
        "<stream:features>",
        "  <mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'>",
        "    <mechanism>PLAIN</mechanism>",
        "  </mechanisms>",
        "</stream:features>",
        "<iq id='iq_roster_1' type='result'>",
        "  <query xmlns='jabber:iq:roster'>",
        "    <item jid='bob@example.com' name='Bob' subscription='both'/>",
        "  </query>",
        "</iq>",
        "<presence from='alice@example.com/mobile'>",
        "  <show>chat</show>",
        "  <status>Ready to chat</status>",
        "</presence>",
        "<message from='alice@example.com' to='bob@example.com' type='chat'>",
        "  <body>Hello from Alice!</body>",
        "</message>",
        "</stream:stream>"
    );

    let mut stream_started = false;
    let mut stream_ended = false;
    let mut stanzas_parsed = 0;

    // Feed in small arbitrary chunk slices (5 bytes each) to simulate network frames
    for chunk in stream_xml.as_bytes().chunks(5) {
        let chunk_str = std::str::from_utf8(chunk).unwrap();
        let events = parser.parse_chunk(chunk_str).expect("chunk parse success");
        for event in events {
            match event {
                StreamEvent::StreamStart(node) => {
                    assert_eq!(node.name(), Some("stream:stream"));
                    assert_eq!(node.find_attrib("to"), Some("example.com"));
                    stream_started = true;
                }
                StreamEvent::Stanza(stanza) => {
                    stanzas_parsed += 1;
                    filter.dispatch(&stanza);
                }
                StreamEvent::StreamEnd => {
                    stream_ended = true;
                }
            }
        }
    }

    assert!(stream_started);
    assert!(stream_ended);
    assert_eq!(stanzas_parsed, 4); // features, iq, presence, message
    assert_eq!(iq_count.load(Ordering::SeqCst), 1);
    assert_eq!(roster_count.load(Ordering::SeqCst), 1);
    assert_eq!(presence_count.load(Ordering::SeqCst), 1);
    assert_eq!(msg_count.load(Ordering::SeqCst), 1);
}
