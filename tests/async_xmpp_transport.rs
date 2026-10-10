use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use iksemel::{AsyncConnection, IksNode, authenticate_plain_async, bind_resource_async};

#[tokio::test]
async fn test_async_xmpp_mock_handshake_and_stanzas() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let port = listener.local_addr().unwrap().port();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept connection");

        let mut buf = [0u8; 4096];

        // 1. Receive client initial stream header
        let n = socket.read(&mut buf).await.expect("read stream header");
        let client_stream = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(client_stream.contains("<stream:stream"));

        // Respond with server stream header and SASL features
        let stream_resp = concat!(
            "<?xml version='1.0'?><stream:stream from='example.com' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams' version='1.0'>",
            "<stream:features><mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><mechanism>PLAIN</mechanism></mechanisms></stream:features>"
        );
        socket
            .write_all(stream_resp.as_bytes())
            .await
            .expect("write features");

        // 2. Receive SASL auth
        let n = socket.read(&mut buf).await.expect("read auth");
        let auth_req = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(auth_req.contains("mechanism=\"PLAIN\""));

        // Respond with success
        socket
            .write_all(b"<success xmlns='urn:ietf:params:xml:ns:xmpp-sasl'/>")
            .await
            .expect("write success");

        // 3. Receive second stream header after SASL
        let n = socket.read(&mut buf).await.expect("read stream header 2");
        let stream2 = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(stream2.contains("<stream:stream"));

        // Send post-auth features with bind
        let bind_features = concat!(
            "<?xml version='1.0'?><stream:stream from='example.com' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams' version='1.0'>",
            "<stream:features><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'/></stream:features>"
        );
        socket
            .write_all(bind_features.as_bytes())
            .await
            .expect("write bind features");

        // 4. Receive bind IQ
        let n = socket.read(&mut buf).await.expect("read bind");
        let bind_req = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(bind_req.contains("bind_async"));
        assert!(bind_req.contains("async-res"));

        // Respond with bind result
        let bind_resp = "<iq type='result' id='bind_async'><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'><jid>testuser@example.com/async-res</jid></bind></iq>";
        socket
            .write_all(bind_resp.as_bytes())
            .await
            .expect("write bind result");

        // 5. Receive message stanza from client
        let n = socket.read(&mut buf).await.expect("read client message");
        let client_msg = std::str::from_utf8(&buf[..n]).unwrap();
        assert!(client_msg.contains("Hello async world"));

        // Send a message back to client
        let incoming_msg = "<message from='bot@example.com' to='testuser@example.com' type='chat'><body>Async echo received</body></message>";
        socket
            .write_all(incoming_msg.as_bytes())
            .await
            .expect("write incoming message");

        // 6. Receive closing tag
        let _ = socket.read(&mut buf).await;
    });

    // Client execution
    let mut conn = AsyncConnection::connect(
        "127.0.0.1",
        port,
        "example.com",
        Some(Duration::from_secs(5)),
    )
    .await
    .expect("async connect");

    conn.start_stream().await.expect("start stream");
    let features = conn.recv_stanza().await.expect("recv features");
    assert_eq!(features.local_name().as_deref(), Some("features"));

    // Authenticate
    authenticate_plain_async(&mut conn, "testuser", "secretpass", None)
        .await
        .expect("authenticate async");

    // Receive post-auth features
    let post_features = conn.recv_stanza().await.expect("recv post features");
    assert_eq!(post_features.local_name().as_deref(), Some("features"));

    // Bind resource
    let bound_jid = bind_resource_async(&mut conn, Some("async-res"))
        .await
        .expect("bind resource async");
    assert_eq!(bound_jid, "testuser@example.com/async-res");

    // Send a message
    let mut msg = IksNode::new_tag("message");
    msg.add_attribute("to", "bot@example.com");
    msg.add_attribute("type", "chat");
    let mut body = IksNode::new_tag("body");
    body.insert_cdata("Hello async world");
    msg.add_child(body);

    conn.send_stanza(&msg).await.expect("send stanza");

    // Receive incoming message
    let received = conn.recv_stanza().await.expect("recv stanza");
    assert_eq!(received.name().as_deref(), Some("message"));
    assert_eq!(
        received.find_attrib("from").as_deref(),
        Some("bot@example.com")
    );
    assert_eq!(
        received.find_path_text(&["body"]),
        Some("Async echo received".to_string())
    );

    conn.close().await.expect("close conn");
    server_task.await.expect("server completed");
}

#[tokio::test]
async fn test_async_connection_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let port = listener.local_addr().unwrap().port();

    let _server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        // Read stream header but never respond, simulating stalled server
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await;
        tokio::time::sleep(Duration::from_secs(5)).await;
    });

    let mut conn = AsyncConnection::connect(
        "127.0.0.1",
        port,
        "example.com",
        Some(Duration::from_millis(250)),
    )
    .await
    .expect("connect");

    conn.start_stream().await.expect("start stream");
    let result = conn.recv_stanza().await;
    assert!(result.is_err(), "Expected timeout error on stalled server");
}
