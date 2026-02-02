use iksemel::{
    authenticate_scram_sha1_async, base64_encode, AsyncConnection, ScramClient, ScramHash,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn test_rfc_5802_scram_sha1_specification_vector() {
    // Exact test vector from RFC 5802 Section 5:
    // C: n,,n=user,r=fyko+d2lbbFgONRv9qkxdawL
    // S: r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j,s=QSXCR+Q6sek8bf92,i=4096
    // C: c=biws,r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j,p=v0X8v3Bz2T0CJGbJQyF0X+HI4Ts=
    // S: v=rmF9pqV8S7suAoZWja4dJRkFsKQ=

    let mut client =
        ScramClient::new(ScramHash::Sha1, "user", "pencil").with_nonce("fyko+d2lbbFgONRv9qkxdawL");

    let first = client.client_first_message();
    assert_eq!(first, "n,,n=user,r=fyko+d2lbbFgONRv9qkxdawL");

    let server_challenge = "r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j,s=QSXCR+Q6sek8bf92,i=4096";
    let final_msg = client.process_challenge(server_challenge).unwrap();
    assert_eq!(
        final_msg,
        "c=biws,r=fyko+d2lbbFgONRv9qkxdawL3rfcNHYJY1ZVvWVs7j,p=v0X8v3Bz2T0CJGbJQyF0X+HI4Ts="
    );

    let server_success = "v=rmF9pqV8S7suAoZWja4dJRkFsKQ=";
    assert!(client.verify_success(server_success).is_ok());

    // Tampered server signature must be rejected
    let bad_server_success = "v=badSignature12345678901234567=";
    assert!(client.verify_success(bad_server_success).is_err());
}

#[test]
fn test_scram_sha256_handshake_and_escaping() {
    let mut client = ScramClient::new(ScramHash::Sha256, "user=name,test", "secretpass")
        .with_nonce("client_nonce_123");

    let first = client.client_first_message();
    assert_eq!(first, "n,,n=user=3Dname=2Ctest,r=client_nonce_123");

    // Server sends challenge with matched nonce
    let challenge = "r=client_nonce_123_srv_nonce_456,s=c2FsdHNhbHQ=,i=4096";
    let final_msg = client.process_challenge(challenge).unwrap();
    assert!(final_msg.starts_with("c=biws,r=client_nonce_123_srv_nonce_456,p="));

    // Tampered challenge nonce should fail
    let mut bad_client =
        ScramClient::new(ScramHash::Sha256, "alice", "pass").with_nonce("expected_nonce");
    let mismatch_challenge = "r=wrong_nonce_prefix,s=c2FsdA==,i=4096";
    assert!(bad_client.process_challenge(mismatch_challenge).is_err());
}

#[tokio::test]
async fn test_mock_async_scram_sha1_handshake() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];

        // 1. Initial stream header from client
        let _ = socket.read(&mut buf).await.unwrap();
        let stream_resp = concat!(
            "<?xml version='1.0'?><stream:stream from='localhost' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams' version='1.0'>",
            "<stream:features><mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><mechanism>SCRAM-SHA-1</mechanism></mechanisms></stream:features>"
        );
        socket.write_all(stream_resp.as_bytes()).await.unwrap();

        // 2. Client sends <auth mechanism="SCRAM-SHA-1">...
        let n = socket.read(&mut buf).await.unwrap();
        let req = String::from_utf8_lossy(&buf[..n]);
        assert!(req.contains("SCRAM-SHA-1"));

        // Extract client nonce from request and send challenge
        let start_pos = req.find('>').unwrap() + 1;
        let end_pos = req.find("</auth>").unwrap();
        let inner_b64 = &req[start_pos..end_pos];
        let decoded_bytes = iksemel::base64_decode(inner_b64.trim()).unwrap();
        let decoded = String::from_utf8(decoded_bytes).unwrap();
        let client_nonce = decoded.split("r=").nth(1).unwrap().trim();

        let server_nonce = format!("{}server123", client_nonce);
        let challenge_str = format!("r={},s=QSXCR+Q6sek8bf92,i=4096", server_nonce);
        let challenge_b64 = base64_encode(challenge_str.as_bytes());
        let challenge_stanza = format!(
            "<challenge xmlns='urn:ietf:params:xml:ns:xmpp-sasl'>{}</challenge>",
            challenge_b64
        );
        socket.write_all(challenge_stanza.as_bytes()).await.unwrap();

        // 3. Client sends <response>...
        let n = socket.read(&mut buf).await.unwrap();
        let resp = String::from_utf8_lossy(&buf[..n]);
        assert!(resp.contains("<response"));

        // Send failure
        let failure_stanza =
            "<failure xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><not-authorized/></failure>";
        socket.write_all(failure_stanza.as_bytes()).await.unwrap();
    });

    let mut conn = AsyncConnection::connect(
        "127.0.0.1",
        addr.port(),
        "localhost",
        Some(Duration::from_secs(5)),
    )
    .await
    .unwrap();

    conn.start_stream().await.unwrap();
    let _features = conn.recv_stanza().await.unwrap();

    // Authenticate with server returning failure
    let result = authenticate_scram_sha1_async(&mut conn, "user", "wrong_password").await;
    assert!(result.is_err());

    let _ = server_task.await;
}
