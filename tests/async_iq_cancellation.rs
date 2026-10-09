use iksemel::{AsyncConnection, IksError};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

const HEADER: &str =
    "<stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client'>";

/// Starts a mock server that sends the header plus `first`, waits for `release`, then sends `second`.
async fn connect_mock(
    first: &'static str,
    second: &'static str,
) -> (
    AsyncConnection,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (release, released) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = socket.read(&mut buf).await.unwrap();
        socket
            .write_all(format!("{HEADER}{first}").as_bytes())
            .await
            .unwrap();
        let _ = released.await;
        socket.write_all(second.as_bytes()).await.unwrap();
        // Keep the socket open until the client is done.
        let _ = socket.read(&mut buf).await;
    });
    let mut conn = AsyncConnection::connect("127.0.0.1", port, "example.com", None)
        .await
        .unwrap();
    conn.start_stream().await.unwrap();
    (conn, release, server)
}

fn id_of(node: &iksemel::NodeRef) -> String {
    node.find_attrib("id").unwrap().to_string()
}

#[tokio::test]
async fn external_timeout_preserves_buffered_stanza() {
    let (mut conn, release, server) = connect_mock(
        "<message id='before'><body>must survive</body></message>",
        "<message id='after'><body>new</body></message>",
    )
    .await;
    let res =
        tokio::time::timeout(Duration::from_millis(100), conn.recv_iq_response("missing")).await;
    assert!(res.is_err(), "expected external timeout");
    release.send(()).unwrap();
    assert_eq!(id_of(&conn.recv_stanza().await.unwrap()), "before");
    assert_eq!(id_of(&conn.recv_stanza().await.unwrap()), "after");
    drop(conn);
    server.await.unwrap();
}

#[tokio::test]
async fn select_cancel_preserves_fifo_order() {
    let (mut conn, release, server) = connect_mock(
        "<message id='m1'/><iq id='other' type='result'/>",
        "<message id='m2'/>",
    )
    .await;
    tokio::select! {
        _ = conn.recv_iq_response("missing") => panic!("must not resolve"),
        _ = tokio::time::sleep(Duration::from_millis(100)) => {}
    }
    release.send(()).unwrap();
    assert_eq!(id_of(&conn.recv_stanza().await.unwrap()), "m1");
    assert_eq!(id_of(&conn.recv_stanza().await.unwrap()), "other");
    assert_eq!(id_of(&conn.recv_stanza().await.unwrap()), "m2");
    drop(conn);
    server.await.unwrap();
}

#[tokio::test]
async fn matching_iq_is_extracted_others_kept() {
    let (mut conn, release, server) = connect_mock(
        "<message id='m1'/><iq id='x' type='result'/><message id='m2'/>",
        "",
    )
    .await;
    release.send(()).unwrap();
    let iq = conn.recv_iq_response("x").await.unwrap();
    assert_eq!(iq.name().as_deref(), Some("iq"));
    assert_eq!(id_of(&iq), "x");
    assert_eq!(id_of(&conn.recv_stanza().await.unwrap()), "m1");
    assert_eq!(id_of(&conn.recv_stanza().await.unwrap()), "m2");
    drop(conn);
    server.await.unwrap();
}

#[tokio::test]
async fn stream_end_while_waiting_returns_net_dropped() {
    let (mut conn, release, server) = connect_mock("<message id='m1'/>", "</stream:stream>").await;
    release.send(()).unwrap();
    let err = conn.recv_iq_response("missing").await.unwrap_err();
    assert!(matches!(err, IksError::NetDropped), "got {err:?}");
    assert_eq!(id_of(&conn.recv_stanza().await.unwrap()), "m1");
    drop(conn);
    server.await.unwrap();
}
