use iksemel::{
    AsyncConnection, IksNode, StreamManagementState, XMLNS_STREAM_MANAGEMENT, build_sm_ack,
    build_sm_enable, build_sm_request_ack, build_sm_resume, is_sm_stanza, parse_sm_ack,
    parse_sm_enabled, parse_sm_resumed,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn test_sm_state_inbound_and_outbound_counting() {
    let mut sm = StreamManagementState::new();
    assert_eq!(sm.inbound_h, 0);
    assert_eq!(sm.outbound_h, 0);
    assert!(!sm.enabled);

    sm.enabled = true;
    sm.sm_id = Some("sess-1234".to_string());
    assert!(sm.enabled);
    assert_eq!(sm.sm_id, Some("sess-1234".to_string()));

    // Stanzas increment outbound count
    let msg1 = IksNode::new_tag("message");
    let msg2 = IksNode::new_tag("presence");
    let iq1 = IksNode::new_tag("iq");

    sm.queue_outbound_stanza(msg1);
    sm.queue_outbound_stanza(msg2);
    sm.queue_outbound_stanza(iq1);

    assert_eq!(sm.outbound_h, 3);
    assert_eq!(sm.unacked_queue.len(), 3);

    // Inbound stanzas increment inbound count
    sm.handle_inbound_stanza();
    sm.handle_inbound_stanza();
    assert_eq!(sm.inbound_h, 2);

    // Process ack for h=2
    sm.process_ack(2);
    assert_eq!(sm.unacked_queue.len(), 1);
    assert_eq!(sm.unacked_queue[0].name().unwrap(), "iq");

    // Process ack for h=3
    sm.process_ack(3);
    assert!(sm.unacked_queue.is_empty());

    // Reset clears state
    sm.reset();
    assert_eq!(sm.inbound_h, 0);
    assert_eq!(sm.outbound_h, 0);
    assert!(!sm.enabled);
    assert!(sm.sm_id.is_none());
    assert!(sm.unacked_queue.is_empty());
}

#[test]
fn test_sm_stanzas_builders_and_parsers() {
    // Enable without resume
    let enable_no_resume = build_sm_enable(false, None);
    assert_eq!(enable_no_resume.name().unwrap(), "enable");
    assert_eq!(
        enable_no_resume.find_attrib("xmlns").unwrap(),
        XMLNS_STREAM_MANAGEMENT
    );
    assert!(enable_no_resume.find_attrib("resume").is_none());

    // Enable with resume and max
    let enable_with_resume = build_sm_enable(true, Some(600));
    assert_eq!(enable_with_resume.find_attrib("resume").unwrap(), "true");
    assert_eq!(enable_with_resume.find_attrib("max").unwrap(), "600");

    // Enabled response parsing
    let enabled_doc = iksemel::DomParser::parse_str(
        r#"<enabled xmlns="urn:xmpp:sm:3" id="some-id" resume="true" max="300"/>"#,
    )
    .unwrap();
    let enabled_node = enabled_doc.borrow();
    let enabled_data = parse_sm_enabled(&enabled_node).expect("Should parse enabled stanza");
    assert_eq!(enabled_data.id, Some("some-id".to_string()));
    assert!(enabled_data.resume);
    assert_eq!(enabled_data.max, Some(300));

    // Resume stanza
    let resume = build_sm_resume("some-prev-id", 42);
    assert_eq!(resume.name().unwrap(), "resume");
    assert_eq!(resume.find_attrib("previd").unwrap(), "some-prev-id");
    assert_eq!(resume.find_attrib("h").unwrap(), "42");

    // Resumed stanza parsing
    let resumed_doc = iksemel::DomParser::parse_str(
        r#"<resumed xmlns="urn:xmpp:sm:3" previd="some-prev-id" h="42"/>"#,
    )
    .unwrap();
    let resumed_node = resumed_doc.borrow();
    let resumed_data = parse_sm_resumed(&resumed_node).expect("Should parse resumed stanza");
    assert_eq!(resumed_data.previd, "some-prev-id");
    assert_eq!(resumed_data.h, 42);

    // Ack stanza (<a>)
    let ack = build_sm_ack(105);
    assert_eq!(ack.name().unwrap(), "a");
    assert_eq!(ack.find_attrib("h").unwrap(), "105");
    assert_eq!(parse_sm_ack(&ack), Some(105));

    // Request ack (<r>)
    let req = build_sm_request_ack();
    assert_eq!(req.name().unwrap(), "r");
    assert_eq!(req.find_attrib("xmlns").unwrap(), XMLNS_STREAM_MANAGEMENT);

    // Identify SM elements
    assert!(is_sm_stanza(&enable_no_resume));
    assert!(is_sm_stanza(&enabled_node));
    assert!(is_sm_stanza(&resume));
    assert!(is_sm_stanza(&resumed_node));
    assert!(is_sm_stanza(&ack));
    assert!(is_sm_stanza(&req));

    let normal_msg = IksNode::new_tag("message");
    assert!(!is_sm_stanza(&normal_msg));
}

#[tokio::test]
async fn test_async_split_concurrent_send_recv() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind listener");
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        // Send stream open header
        let header = r#"<stream:stream xmlns="jabber:client" xmlns:stream="http://etherx.jabber.org/streams">"#;
        socket.write_all(header.as_bytes()).await.unwrap();

        let mut buf = [0u8; 1024];
        let n = socket.read(&mut buf).await.unwrap();
        let received = String::from_utf8_lossy(&buf[..n]);
        assert!(received.contains("<client-ping/>"));

        // Respond with server pong
        let resp = r#"<server-pong status="ok"/>"#;
        socket.write_all(resp.as_bytes()).await.unwrap();
    });

    let client = AsyncConnection::connect(
        "127.0.0.1",
        addr.port(),
        "example.com",
        Some(Duration::from_secs(5)),
    )
    .await
    .expect("Client connect failed");

    let (sender, mut receiver) = client.split().expect("Split failed");

    // Spawn concurrent sender task
    let sender_handle = tokio::spawn(async move {
        sender
            .send_raw("<client-ping/>")
            .await
            .expect("Failed to send ping");
    });

    // Receive stanza asynchronously on receiver half
    let response_stanza = receiver
        .recv_stanza()
        .await
        .expect("Failed to receive pong");
    assert_eq!(response_stanza.name().as_deref().unwrap(), "server-pong");
    assert_eq!(
        response_stanza.find_attrib("status").as_deref().unwrap(),
        "ok"
    );

    sender_handle.await.unwrap();
    server_task.await.unwrap();
}
