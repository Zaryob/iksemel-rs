//! Çok baytlı bir karakterin ağ paketi sınırına denk gelmesi.
//!
//! Her transport yolu için ayrı test: regresyonun hangi yolda olduğunu
//! göstermelidir.

use std::io::Write;
use std::net::TcpListener;
use std::time::Duration;

use iksemel::{AsyncConnection, Connection, StreamEvent};

/// `<stream:stream ...>` başlığı ve gövdesinde '€' bulunan tek bir mesaj.
const STREAM: &[u8] = b"<stream:stream xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams' to='example.com' version='1.0'><message type='chat'><body>fiyat: \xE2\x82\xAC</body></message>";

/// '€' dizisinin ilk baytının konumu.
fn euro_start() -> usize {
    STREAM.iter().position(|&b| b == 0xE2).expect("euro")
}

/// Sunucu, `payload`'ı `splits` konumlarından bölerek gönderir.
fn serve_split(payload: &'static [u8], splits: Vec<usize>) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().expect("accept");
        let mut prev = 0;
        for s in splits.iter().chain(std::iter::once(&payload.len())) {
            sock.write_all(&payload[prev..*s]).expect("write");
            sock.flush().expect("flush");
            // Parça sınırının gerçekten ayrı bir okuma olması için bekle.
            std::thread::sleep(Duration::from_millis(20));
            prev = *s;
        }
        std::thread::sleep(Duration::from_millis(300));
    });
    port
}

fn first_stanza_body(mut events: impl Iterator<Item = StreamEvent>) -> String {
    let stanza = events
        .find_map(|e| match e {
            StreamEvent::Stanza(s) => Some(s),
            _ => None,
        })
        .expect("stanza gelmeli");
    stanza
        .find_cdata("body")
        .expect("body metni bulunmalı")
}

#[test]
fn sync_connection_reassembles_split_euro() {
    // E2 | 82 AC — dizi ilk bayttan sonra bölünür.
    let port = serve_split(STREAM, vec![euro_start() + 1]);

    let mut conn = Connection::connect("127.0.0.1", port, "example.com", None).expect("connect");
    let mut events = Vec::new();
    for _ in 0..16 {
        let e = conn.recv_event().expect("recv");
        let done = matches!(e, StreamEvent::Stanza(_));
        events.push(e);
        if done {
            break;
        }
    }
    assert_eq!(first_stanza_body(events.into_iter()), "fiyat: €");
}

#[test]
fn sync_connection_reassembles_two_byte_split_euro() {
    // E2 82 | AC — dizi son bayttan önce bölünür.
    let port = serve_split(STREAM, vec![euro_start() + 2]);

    let mut conn = Connection::connect("127.0.0.1", port, "example.com", None).expect("connect");
    let mut events = Vec::new();
    for _ in 0..16 {
        let e = conn.recv_event().expect("recv");
        let done = matches!(e, StreamEvent::Stanza(_));
        events.push(e);
        if done {
            break;
        }
    }
    assert_eq!(first_stanza_body(events.into_iter()), "fiyat: €");
}

#[test]
fn async_connection_reassembles_split_euro() {
    let port = serve_split(STREAM, vec![euro_start() + 1]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let mut conn = AsyncConnection::connect("127.0.0.1", port, "example.com", None)
            .await
            .expect("connect");
        let mut events = Vec::new();
        for _ in 0..16 {
            let e = conn.recv_event().await.expect("recv");
            let done = matches!(e, StreamEvent::Stanza(_));
            events.push(e);
            if done {
                break;
            }
        }
        assert_eq!(first_stanza_body(events.into_iter()), "fiyat: €");
    });
}

#[test]
fn split_receiver_reassembles_split_euro() {
    let port = serve_split(STREAM, vec![euro_start() + 2]);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let conn = AsyncConnection::connect("127.0.0.1", port, "example.com", None)
            .await
            .expect("connect");
        let (_sender, mut receiver) = conn.split().expect("split");
        let mut events = Vec::new();
        for _ in 0..16 {
            let e = receiver.recv_event().await.expect("recv");
            let done = matches!(e, StreamEvent::Stanza(_));
            events.push(e);
            if done {
                break;
            }
        }
        assert_eq!(first_stanza_body(events.into_iter()), "fiyat: €");
    });
}
