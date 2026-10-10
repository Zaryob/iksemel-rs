/*
            iksemel - XML parser for Rust
          Copyright (C) 2024 Süleyman Poyraz
 This code is free software; you can redistribute it and/or
 modify it under the terms of the GNU Lesser General Public License
 as published by the Free Software Foundation; either version 2.1
 of the License, or (at your option) any later version.
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU Lesser General Public License for more details.
*/

use native_tls::TlsStream;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::{IksError, IksNode, NodeRef, Result, StreamEvent, StreamParser};

/// Underlying transport stream (Plain TCP or TLS encrypted).
#[allow(clippy::large_enum_variant)]
pub enum ConnectionStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
    Custom(Box<dyn crate::Transport>),
    SecureCustom(Box<dyn crate::Transport>),
}

impl Read for ConnectionStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ConnectionStream::Plain(s) => s.read(buf),
            ConnectionStream::Tls(s) => s.read(buf),
            ConnectionStream::Custom(s) | ConnectionStream::SecureCustom(s) => s.read(buf),
        }
    }
}

impl Write for ConnectionStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ConnectionStream::Plain(s) => s.write(buf),
            ConnectionStream::Tls(s) => s.write(buf),
            ConnectionStream::Custom(s) | ConnectionStream::SecureCustom(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ConnectionStream::Plain(s) => s.flush(),
            ConnectionStream::Tls(s) => s.flush(),
            ConnectionStream::Custom(s) | ConnectionStream::SecureCustom(s) => s.flush(),
        }
    }
}

use std::collections::VecDeque;

/// A client connection to an XMPP server with StartTLS negotiation support.
pub struct Connection {
    stream: Option<ConnectionStream>,
    parser: StreamParser,
    utf8: crate::utf8::Utf8Carry,
    pending_events: VecDeque<StreamEvent>,
    domain: String,
    timeout: Option<Duration>,
    allow_insecure_tls: bool,
    log_traffic: bool,
    traffic: crate::transport::Traffic,
}

impl Connection {
    pub fn from_transport(stream: impl crate::Transport + 'static, domain: &str) -> Self {
        Self {
            stream: Some(ConnectionStream::Custom(Box::new(stream))),
            parser: StreamParser::new(),
            utf8: crate::utf8::Utf8Carry::new(),
            pending_events: VecDeque::new(),
            domain: domain.to_string(),
            timeout: None,
            allow_insecure_tls: false,
            log_traffic: false,
            traffic: Default::default(),
        }
    }
    pub fn set_log_hook(&mut self, hook: Option<crate::LogHook>) {
        self.traffic.hook(hook);
    }
    pub fn domain(&self) -> &str {
        &self.domain
    }
    pub fn byte_counts(&self) -> crate::ByteCounts {
        self.traffic.counts()
    }
    pub fn set_certificate_verification(&mut self, verify: bool) {
        self.allow_insecure_tls = !verify;
    }
    /// Connects to an XMPP server via TCP.
    ///
    /// # Arguments
    /// * `host` - Server hostname or IP address
    /// * `port` - Server port (typically 5222 for c2s)
    /// * `domain` - XMPP realm / domain (e.g. `example.com`)
    /// * `timeout` - Socket timeout for network operations
    pub fn connect(host: &str, port: u16, domain: &str, timeout: Option<Duration>) -> Result<Self> {
        let addr = format!("{}:{}", host, port);
        let socket_addrs: Vec<_> = addr
            .to_socket_addrs()
            .map_err(|_| IksError::NetNoDns)?
            .collect();
        if socket_addrs.is_empty() {
            return Err(IksError::NetNoDns);
        }

        let tcp_stream = if let Some(t) = timeout {
            TcpStream::connect_timeout(&socket_addrs[0], t).map_err(|_| IksError::NetNoConn)?
        } else {
            TcpStream::connect(socket_addrs[0]).map_err(|_| IksError::NetNoConn)?
        };

        if let Some(t) = timeout {
            tcp_stream
                .set_read_timeout(Some(t))
                .map_err(|_| IksError::NetRwErr)?;
            tcp_stream
                .set_write_timeout(Some(t))
                .map_err(|_| IksError::NetRwErr)?;
        }

        Ok(Connection {
            stream: Some(ConnectionStream::Plain(tcp_stream)),
            parser: StreamParser::new(),
            utf8: crate::utf8::Utf8Carry::new(),
            pending_events: VecDeque::new(),
            domain: domain.to_string(),
            timeout,
            allow_insecure_tls: false,
            log_traffic: false,
            traffic: Default::default(),
        })
    }

    /// Creates a Connection from an existing `TcpStream` (useful for testing and custom sockets).
    pub fn from_tcp_stream(tcp_stream: TcpStream, domain: &str) -> Self {
        Connection {
            stream: Some(ConnectionStream::Plain(tcp_stream)),
            parser: StreamParser::new(),
            utf8: crate::utf8::Utf8Carry::new(),
            pending_events: VecDeque::new(),
            domain: domain.to_string(),
            timeout: None,
            allow_insecure_tls: false,
            log_traffic: false,
            traffic: Default::default(),
        }
    }

    /// Gets the current network socket timeout.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Updates the network socket timeout.
    pub fn set_timeout(&mut self, timeout: Option<Duration>) -> Result<()> {
        self.timeout = timeout;
        if let Some(ref mut stream) = self.stream {
            match stream {
                ConnectionStream::Custom(_) | ConnectionStream::SecureCustom(_) => {
                    return Err(IksError::NetNotSupp);
                }
                ConnectionStream::Plain(s) => {
                    s.set_read_timeout(timeout)
                        .map_err(|_| IksError::NetRwErr)?;
                    s.set_write_timeout(timeout)
                        .map_err(|_| IksError::NetRwErr)?;
                }
                ConnectionStream::Tls(s) => {
                    s.get_ref()
                        .set_read_timeout(timeout)
                        .map_err(|_| IksError::NetRwErr)?;
                    s.get_ref()
                        .set_write_timeout(timeout)
                        .map_err(|_| IksError::NetRwErr)?;
                }
            }
        }
        Ok(())
    }

    /// Sets whether to accept self-signed or invalid TLS certificates.
    pub fn set_allow_insecure_tls(&mut self, allow: bool) {
        self.allow_insecure_tls = allow;
    }

    /// Sets whether to print exchanged XML stanzas to stdout.
    pub fn set_log_traffic(&mut self, log: bool) {
        self.log_traffic = log;
    }

    /// Sends raw text over the network socket.
    pub fn send_raw(&mut self, data: &str) -> Result<()> {
        if self.log_traffic {
            println!("SEND: {}", data);
        }
        let stream = self.stream.as_mut().ok_or(IksError::NetDropped)?;
        stream
            .write_all(data.as_bytes())
            .map_err(|_| IksError::NetRwErr)?;
        stream.flush().map_err(|_| IksError::NetRwErr)?;
        self.traffic
            .record(crate::Direction::Outgoing, data.as_bytes());
        Ok(())
    }

    /// Serializes and sends an XML stanza.
    pub fn send_stanza(&mut self, stanza: &IksNode) -> Result<()> {
        self.send_raw(&stanza.to_string())
    }

    /// Initiates or restarts the root XMPP stream header.
    pub fn start_stream(&mut self) -> Result<StreamEvent> {
        self.parser.reset();
        self.utf8.reset();
        self.pending_events.clear();

        let header = format!(
            "<?xml version='1.0'?><stream:stream to='{}' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams' version='1.0'>",
            self.domain
        );
        self.send_raw(&header)?;

        loop {
            let event = self.recv_event()?;
            if let StreamEvent::StreamStart(_) = event {
                return Ok(event);
            }
        }
    }

    /// Waits for and receives the next `StreamEvent`.
    pub fn recv_event(&mut self) -> Result<StreamEvent> {
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(event);
        }

        let mut buf = [0u8; 4096];
        loop {
            let stream = self.stream.as_mut().ok_or(IksError::NetDropped)?;
            let n = stream.read(&mut buf).map_err(|e| {
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock
                {
                    IksError::NetRwErr
                } else {
                    IksError::NetDropped
                }
            })?;

            if n == 0 {
                return Err(IksError::NetDropped);
            }

            self.traffic.record(crate::Direction::Incoming, &buf[..n]);
            let text = self.utf8.feed(&buf[..n])?;
            if self.log_traffic {
                print!("RECV: {}", text);
            }
            let events = self.parser.parse_chunk(text)?;
            self.pending_events.extend(events);

            if let Some(first) = self.pending_events.pop_front() {
                return Ok(first);
            }
        }
    }

    /// Blocks until the next top-level stanza is received.
    pub fn recv_stanza(&mut self) -> Result<NodeRef> {
        loop {
            match self.recv_event()? {
                StreamEvent::Stanza(stanza) => return Ok(stanza),
                StreamEvent::StreamEnd => return Err(IksError::NetDropped),
                StreamEvent::StreamStart(_) => continue,
                StreamEvent::Error(node) => return Err(IksError::StreamError(node.to_string())),
            }
        }
    }

    /// Waits for an `<iq>` stanza whose `id` attribute matches `expected_id`.
    /// Any non-matching stanzas encountered while waiting are preserved in the pending queue
    /// in their original FIFO order.
    pub fn recv_iq_response(&mut self, expected_id: &str) -> Result<NodeRef> {
        let mut skipped = Vec::new();
        let result = loop {
            match self.recv_event() {
                Ok(StreamEvent::Stanza(stanza)) => {
                    let is_match = stanza.name().as_deref() == Some("iq")
                        && stanza.find_attrib("id").as_deref() == Some(expected_id);
                    if is_match {
                        break Ok(stanza);
                    } else {
                        skipped.push(StreamEvent::Stanza(stanza));
                    }
                }
                Ok(StreamEvent::StreamStart(_)) => continue,
                Ok(StreamEvent::Error(node)) => break Err(IksError::StreamError(node.to_string())),
                Ok(StreamEvent::StreamEnd) => break Err(IksError::NetDropped),
                Err(e) => break Err(e),
            }
        };

        for event in skipped.into_iter().rev() {
            self.pending_events.push_front(event);
        }

        result
    }

    /// Negotiates StartTLS on the connection (RFC 6120 Section 5).
    pub fn start_tls(&mut self) -> Result<()> {
        self.start_tls_with(&crate::NativeTlsBackend)
    }
    pub fn start_tls_with(&mut self, backend: &dyn crate::TlsBackend) -> Result<()> {
        self.send_raw("<starttls xmlns='urn:ietf:params:xml:ns:xmpp-tls'/>")?;
        if self.recv_stanza()?.name().as_deref() != Some("proceed") {
            return Err(IksError::NetTlsFail);
        }
        let stream = self.stream.take().ok_or(IksError::NetDropped)?;
        self.stream = Some(ConnectionStream::SecureCustom(backend.upgrade(
            Box::new(stream),
            &self.domain,
            !self.allow_insecure_tls,
        )?));
        self.start_stream()?;
        Ok(())
    }

    /// Gracefully closes the XMPP stream.
    pub fn close(&mut self) -> Result<()> {
        let _ = self.send_raw("</stream:stream>");
        self.stream = None;
        Ok(())
    }

    /// True if currently using TLS encryption.
    pub fn is_tls(&self) -> bool {
        matches!(
            self.stream,
            Some(ConnectionStream::Tls(_) | ConnectionStream::SecureCustom(_))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_mock_connection_stream_exchange() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = thread::spawn(move || {
            let (mut server_stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];

            // 1. Read stream header from client
            let n = server_stream.read(&mut buf).unwrap();
            let client_req = std::str::from_utf8(&buf[..n]).unwrap();
            assert!(client_req.contains("<stream:stream"));

            // 2. Send server stream header + features
            let server_resp = concat!(
                "<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' from='example.com' version='1.0'>",
                "<stream:features><mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><mechanism>PLAIN</mechanism></mechanisms></stream:features>"
            );
            server_stream.write_all(server_resp.as_bytes()).unwrap();

            // 3. Read ping iq
            let n = server_stream.read(&mut buf).unwrap();
            let ping_req = std::str::from_utf8(&buf[..n]).unwrap();
            assert!(ping_req.contains("ping1"));

            // 4. Send pong iq
            let pong_resp = "<iq from='example.com' id='ping1' type='result'/>";
            server_stream.write_all(pong_resp.as_bytes()).unwrap();
        });

        let mut conn = Connection::connect(
            "127.0.0.1",
            port,
            "example.com",
            Some(Duration::from_secs(5)),
        )
        .unwrap();
        let stream_event = conn.start_stream().unwrap();
        match stream_event {
            StreamEvent::StreamStart(node) => {
                assert_eq!(node.name().as_deref(), Some("stream:stream"));
            }
            _ => panic!("Expected StreamStart"),
        }

        // Receive features stanza
        let features = conn.recv_stanza().unwrap();
        assert_eq!(features.name().as_deref(), Some("stream:features"));

        // Send ping
        let mut ping = IksNode::new_tag("iq");
        ping.add_attribute("id", "ping1");
        ping.add_attribute("type", "get");
        ping.add_attribute("to", "example.com");
        conn.send_stanza(&ping).unwrap();

        // Receive pong
        let pong = conn.recv_stanza().unwrap();
        assert_eq!(pong.name().as_deref(), Some("iq"));
        assert_eq!(pong.find_attrib("id").as_deref(), Some("ping1"));
        assert_eq!(pong.find_attrib("type").as_deref(), Some("result"));

        handle.join().unwrap();
    }

    #[test]
    fn test_recv_iq_response_interleaved_stanzas() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = thread::spawn(move || {
            let (mut server_stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];

            // 1. Read stream header
            let _ = server_stream.read(&mut buf).unwrap();

            // 2. Send server stream header
            let server_hdr = "<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' from='example.com' version='1.0'>";
            server_stream.write_all(server_hdr.as_bytes()).unwrap();

            // 3. Send interleaved presence, message, and target IQ, followed by another presence
            let stanzas = concat!(
                "<presence from='alice@example.com'><show>chat</show></presence>",
                "<message from='bob@example.com'><body>hello</body></message>",
                "<iq id='expected_iq' type='result'><query xmlns='test'/></iq>",
                "<presence from='carol@example.com'><show>dnd</show></presence>"
            );
            server_stream.write_all(stanzas.as_bytes()).unwrap();
        });

        let mut conn = Connection::connect(
            "127.0.0.1",
            port,
            "example.com",
            Some(Duration::from_secs(5)),
        )
        .unwrap();
        let _ = conn.start_stream().unwrap();

        // Must receive expected IQ despite interleaved presence and message
        let iq = conn.recv_iq_response("expected_iq").unwrap();
        assert_eq!(iq.name().as_deref(), Some("iq"));
        assert_eq!(iq.find_attrib("id").as_deref(), Some("expected_iq"));
        assert_eq!(iq.find_attrib("type").as_deref(), Some("result"));

        // Interleaved presence and message must still be in pending queue in order
        let st1 = conn.recv_stanza().unwrap();
        assert_eq!(st1.name().as_deref(), Some("presence"));
        assert_eq!(
            st1.find_attrib("from").as_deref(),
            Some("alice@example.com")
        );

        let st2 = conn.recv_stanza().unwrap();
        assert_eq!(st2.name().as_deref(), Some("message"));
        assert_eq!(st2.find_attrib("from").as_deref(), Some("bob@example.com"));

        let st3 = conn.recv_stanza().unwrap();
        assert_eq!(st3.name().as_deref(), Some("presence"));
        assert_eq!(
            st3.find_attrib("from").as_deref(),
            Some("carol@example.com")
        );

        handle.join().unwrap();
    }
}
