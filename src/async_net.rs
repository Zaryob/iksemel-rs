/*
            iksemel - XML parser for Rust
          Copyright (C) 2026 Süleyman Poyraz
 This code is free software; you can redistribute it and/or
 modify it under the terms of the GNU Lesser General Public License
 as published by the Free Software Foundation; either version 2.1
 of the License, or (at your option) any later version.
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU Lesser General Public License for more details.
*/

use native_tls::TlsConnector;
use std::collections::VecDeque;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_native_tls::{TlsConnector as TokioTlsConnector, TlsStream};

use crate::{
    base64_decode, base64_encode, IksError, IksNode, NodeRef, Result, StreamEvent, StreamParser,
};

/// A thread-safe handle for sending stanzas or raw XML concurrently from any Tokio task.
#[derive(Clone)]
pub struct AsyncSender {
    writer: std::sync::Arc<tokio::sync::Mutex<tokio::io::WriteHalf<AsyncConnectionStream>>>,
}

impl AsyncSender {
    /// Sends a formatted XML stanza.
    pub async fn send_stanza(&self, stanza: &IksNode) -> Result<()> {
        self.send_raw(&stanza.to_string()).await
    }

    /// Sends raw text to the remote server.
    pub async fn send_raw(&self, raw: &str) -> Result<()> {
        let mut guard = self.writer.lock().await;
        guard.write_all(raw.as_bytes()).await?;
        guard.flush().await?;
        Ok(())
    }
}

/// A handle for receiving parsed XML stanzas asynchronously.
pub struct AsyncReceiver {
    reader: tokio::io::ReadHalf<AsyncConnectionStream>,
    parser: StreamParser,
    utf8: crate::utf8::Utf8Carry,
    pending_events: VecDeque<StreamEvent>,
    timeout: Option<Duration>,
}

impl AsyncReceiver {
    /// Receives the next incoming stream event.
    pub async fn recv_event(&mut self) -> Result<StreamEvent> {
        loop {
            if let Some(event) = self.pending_events.pop_front() {
                return Ok(event);
            }

            let mut buf = [0u8; 4096];
            let read_fut = self.reader.read(&mut buf);
            let n = if let Some(t) = self.timeout {
                tokio::time::timeout(t, read_fut)
                    .await
                    .map_err(|_| IksError::NetRwErr)?
                    .map_err(|_| IksError::NetRwErr)?
            } else {
                read_fut.await.map_err(|_| IksError::NetRwErr)?
            };

            if n == 0 {
                return Err(IksError::NetDropped);
            }

            let chunk_str = self.utf8.feed(&buf[..n])?;
            let events = self.parser.parse_chunk(chunk_str)?;
            self.pending_events.extend(events);
        }
    }

    /// Receives the next incoming XML stanza.
    pub async fn recv_stanza(&mut self) -> Result<NodeRef> {
        loop {
            match self.recv_event().await? {
                StreamEvent::Stanza(stanza) => return Ok(stanza),
                StreamEvent::StreamEnd => return Err(IksError::NetDropped),
                StreamEvent::Error(node) => return Err(IksError::StreamError(node.to_string())),
                StreamEvent::StreamStart(_) => continue,
            }
        }
    }
}

/// Underlying non-blocking transport stream (Plain TCP or TLS encrypted).
#[allow(clippy::large_enum_variant)]
pub enum AsyncConnectionStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
    Custom(Box<dyn crate::AsyncTransport>),
}

impl AsyncRead for AsyncConnectionStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            AsyncConnectionStream::Plain(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            AsyncConnectionStream::Tls(s) => std::pin::Pin::new(s).poll_read(cx, buf),
            AsyncConnectionStream::Custom(s) => std::pin::Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for AsyncConnectionStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match self.get_mut() {
            AsyncConnectionStream::Plain(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            AsyncConnectionStream::Tls(s) => std::pin::Pin::new(s).poll_write(cx, buf),
            AsyncConnectionStream::Custom(s) => std::pin::Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            AsyncConnectionStream::Plain(s) => std::pin::Pin::new(s).poll_flush(cx),
            AsyncConnectionStream::Tls(s) => std::pin::Pin::new(s).poll_flush(cx),
            AsyncConnectionStream::Custom(s) => std::pin::Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            AsyncConnectionStream::Plain(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            AsyncConnectionStream::Tls(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            AsyncConnectionStream::Custom(s) => std::pin::Pin::new(s).poll_shutdown(cx),
        }
    }
}

/// An asynchronous non-blocking client connection to an XMPP server built on Tokio.
pub struct AsyncConnection {
    stream: Option<AsyncConnectionStream>,
    parser: StreamParser,
    utf8: crate::utf8::Utf8Carry,
    pending_events: VecDeque<StreamEvent>,
    domain: String,
    timeout: Option<Duration>,
    allow_insecure_tls: bool,
    log_traffic: bool,
    sm: crate::StreamManagementState,
    traffic: crate::transport::Traffic,
}

impl AsyncConnection {
    pub fn from_tcp_stream(stream: TcpStream, domain: &str) -> Self {
        Self::from_transport(stream, domain)
    }
    pub fn from_transport(stream: impl crate::AsyncTransport + 'static, domain: &str) -> Self {
        Self {
            stream: Some(AsyncConnectionStream::Custom(Box::new(stream))),
            parser: StreamParser::new(),
            utf8: crate::utf8::Utf8Carry::new(),
            pending_events: VecDeque::new(),
            domain: domain.to_string(),
            timeout: None,
            allow_insecure_tls: false,
            log_traffic: false,
            sm: Default::default(),
            traffic: Default::default(),
        }
    }
    pub fn set_log_hook(&mut self, hook: Option<crate::LogHook>) {
        self.traffic.hook(hook);
    }
    pub fn byte_counts(&self) -> crate::ByteCounts {
        self.traffic.counts()
    }
    pub fn set_certificate_verification(&mut self, verify: bool) {
        self.allow_insecure_tls = !verify;
    }
    /// Connects asynchronously to an XMPP server via TCP.
    pub async fn connect(
        host: &str,
        port: u16,
        domain: &str,
        timeout: Option<Duration>,
    ) -> Result<Self> {
        let addr = format!("{}:{}", host, port);
        let connect_fut = TcpStream::connect(&addr);
        let tcp_stream = if let Some(t) = timeout {
            tokio::time::timeout(t, connect_fut)
                .await
                .map_err(|_| IksError::NetNoConn)?
                .map_err(|_| IksError::NetNoConn)?
        } else {
            connect_fut.await.map_err(|_| IksError::NetNoConn)?
        };

        Ok(AsyncConnection {
            stream: Some(AsyncConnectionStream::Plain(tcp_stream)),
            parser: StreamParser::new(),
            utf8: crate::utf8::Utf8Carry::new(),
            pending_events: VecDeque::new(),
            domain: domain.to_string(),
            timeout,
            allow_insecure_tls: false,
            log_traffic: false,
            sm: Default::default(),
            traffic: Default::default(),
        })
    }

    /// Allows or disallows self-signed / invalid TLS certificates.
    pub fn set_insecure_tls(&mut self, allow: bool) {
        self.allow_insecure_tls = allow;
    }

    /// Enables logging of raw incoming and outgoing XML traffic.
    pub fn set_log_traffic(&mut self, log: bool) {
        self.log_traffic = log;
    }

    /// Returns the domain name of this connection.
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Sends raw text to the remote server.
    pub async fn send_raw(&mut self, data: &str) -> Result<()> {
        let stream = self.stream.as_mut().ok_or(IksError::NetDropped)?;
        if self.log_traffic {
            eprintln!("[XMPP ASYNC OUT] {}", data);
        }
        let write_fut = async {
            stream.write_all(data.as_bytes()).await?;
            stream.flush().await?;
            Ok::<(), std::io::Error>(())
        };

        if let Some(t) = self.timeout {
            tokio::time::timeout(t, write_fut)
                .await
                .map_err(|_| IksError::NetRwErr)?
                .map_err(|_| IksError::NetRwErr)?;
        } else {
            write_fut.await.map_err(|_| IksError::NetRwErr)?;
        }
        self.traffic
            .record(crate::Direction::Outgoing, data.as_bytes());
        Ok(())
    }

    /// Sends the opening `<stream:stream>` XML declaration.
    pub async fn start_stream(&mut self) -> Result<()> {
        self.parser.reset();
        self.utf8.reset();
        self.pending_events.clear();

        let header = format!(
            "<?xml version=\"1.0\"?><stream:stream to=\"{}\" xmlns=\"jabber:client\" xmlns:stream=\"http://etherx.jabber.org/streams\" version=\"1.0\">",
            self.domain
        );
        self.send_raw(&header).await
    }

    /// Serializes and sends an XML stanza.
    pub async fn send_stanza(&mut self, node: &IksNode) -> Result<()> {
        if self.sm.enabled && matches!(node.name(), Some("iq" | "message" | "presence")) {
            self.sm.queue_outbound_stanza(node.clone());
        }
        self.send_raw(&node.to_string()).await
    }

    /// Receives the next stream event, reading chunks from the socket as needed.
    pub async fn recv_event(&mut self) -> Result<StreamEvent> {
        loop {
            self.handle_sm_controls().await?;
            if let Some(event) = self.pending_events.pop_front() {
                return Ok(event);
            }

            self.read_events().await?;
        }
    }

    async fn read_events(&mut self) -> Result<()> {
        let mut buf = [0u8; 4096];
        let stream = self.stream.as_mut().ok_or(IksError::NetDropped)?;

        let read_fut = stream.read(&mut buf);
        let n = if let Some(t) = self.timeout {
            tokio::time::timeout(t, read_fut)
                .await
                .map_err(|_| IksError::NetRwErr)?
                .map_err(|_| IksError::NetRwErr)?
        } else {
            read_fut.await.map_err(|_| IksError::NetRwErr)?
        };

        if n == 0 {
            return Err(IksError::NetDropped);
        }

        self.traffic.record(crate::Direction::Incoming, &buf[..n]);
        let chunk_str = self.utf8.feed(&buf[..n])?;
        if self.log_traffic {
            eprintln!("[XMPP ASYNC IN] {}", chunk_str);
        }
        let events = self.parser.parse_chunk(chunk_str)?;
        if self.sm.enabled {
            for event in &events {
                if let StreamEvent::Stanza(node) = event {
                    if matches!(node.name().as_deref(), Some("iq" | "message" | "presence")) {
                        self.sm.handle_inbound_stanza();
                    }
                }
            }
        }
        self.pending_events.extend(events);
        Ok(())
    }

    /// Receives the next complete XML stanza, skipping stream headers.
    pub async fn recv_stanza(&mut self) -> Result<NodeRef> {
        loop {
            match self.recv_event().await? {
                StreamEvent::Stanza(node) => return Ok(node),
                StreamEvent::StreamEnd => return Err(IksError::NetDropped),
                StreamEvent::Error(node) => return Err(IksError::StreamError(node.to_string())),
                _ => {}
            }
        }
    }

    /// Waits for an `<iq>` stanza whose `id` attribute matches `expected_id`.
    /// Any non-matching stanzas encountered while waiting are preserved in the pending queue
    /// in their original FIFO order.
    ///
    /// Cancellation safe: unrelated events stay in `pending_events` at every await point, and
    /// `read_events` only mutates connection state after its socket read completes. Dropping this
    /// future (for example via `tokio::time::timeout` or `select!`) never loses buffered stanzas.
    pub async fn recv_iq_response(&mut self, expected_id: &str) -> Result<NodeRef> {
        // Leave unrelated events in the connection, including across future cancellation.
        loop {
            self.handle_sm_controls().await?;
            for (index, event) in self.pending_events.iter().enumerate() {
                match event {
                    StreamEvent::Stanza(stanza)
                        if stanza.name().as_deref() == Some("iq")
                            && stanza.find_attrib("id").as_deref() == Some(expected_id) =>
                    {
                        if let Some(StreamEvent::Stanza(stanza)) = self.pending_events.remove(index)
                        {
                            return Ok(stanza);
                        }
                        unreachable!();
                    }
                    StreamEvent::StreamEnd => return Err(IksError::NetDropped),
                    StreamEvent::Error(node) => {
                        return Err(IksError::StreamError(node.to_string()))
                    }
                    _ => {}
                }
            }
            self.read_events().await?;
        }
    }

    /// Splits this connection into concurrent lock-free sender and receiver halves.
    pub fn split(mut self) -> Result<(AsyncSender, AsyncReceiver)> {
        if self.sm.enabled {
            return Err(IksError::NetNotSupp);
        }
        let stream = self.stream.take().ok_or(IksError::NetDropped)?;
        let (reader, writer) = tokio::io::split(stream);

        let sender = AsyncSender {
            writer: std::sync::Arc::new(tokio::sync::Mutex::new(writer)),
        };
        let receiver = AsyncReceiver {
            reader,
            parser: self.parser,
            utf8: self.utf8,
            pending_events: self.pending_events,
            timeout: self.timeout,
        };

        Ok((sender, receiver))
    }

    /// Splits this connection into concurrent sender and receiver halves.
    pub fn split_channels(self, _buffer_size: usize) -> Result<(AsyncSender, AsyncReceiver)> {
        self.split()
    }

    /// Enables XEP-0198 Stream Management on this connection.
    pub async fn enable_stream_management(
        &mut self,
        resume: bool,
        max_seconds: Option<u32>,
    ) -> Result<crate::xep::SmEnabled> {
        let enable_stanza = crate::xep::build_sm_enable(resume, max_seconds);
        self.send_stanza(&enable_stanza).await?;
        let resp = self.recv_sm_response("enabled").await?;
        let enabled = crate::xep::parse_sm_enabled_ref(&resp).ok_or(IksError::NetUnknown)?;
        self.sm.reset();
        self.sm.enabled = true;
        self.sm.sm_id = if enabled.resume {
            enabled.id.clone()
        } else {
            None
        };
        Ok(enabled)
    }

    async fn recv_sm_response(&mut self, expected: &str) -> Result<NodeRef> {
        loop {
            self.handle_sm_controls().await?;
            for (index, event) in self.pending_events.iter().enumerate() {
                match event {
                    StreamEvent::Stanza(node)
                        if node.find_attrib("xmlns").as_deref()
                            == Some(crate::XMLNS_STREAM_MANAGEMENT)
                            && (node.name().as_deref() == Some(expected)
                                || node.name().as_deref() == Some("failed")) =>
                    {
                        if let Some(StreamEvent::Stanza(node)) = self.pending_events.remove(index) {
                            return if node.name().as_deref() == Some("failed") {
                                Err(IksError::NetRwErr)
                            } else {
                                Ok(node)
                            };
                        }
                        unreachable!();
                    }
                    StreamEvent::StreamEnd => return Err(IksError::NetDropped),
                    StreamEvent::Error(node) => {
                        return Err(IksError::StreamError(node.to_string()))
                    }
                    _ => {}
                }
            }
            self.read_events().await?;
        }
    }

    pub fn stream_management(&self) -> &crate::StreamManagementState {
        &self.sm
    }

    async fn handle_sm_controls(&mut self) -> Result<()> {
        if !self.sm.enabled {
            return Ok(());
        }
        loop {
            let control = self
                .pending_events
                .iter()
                .enumerate()
                .find_map(|(i, event)| {
                    if let StreamEvent::Stanza(node) = event {
                        if node.find_attrib("xmlns").as_deref()
                            == Some(crate::XMLNS_STREAM_MANAGEMENT)
                        {
                            return match node.name().as_deref() {
                                Some("r") => Some((i, None)),
                                Some("a") => Some((
                                    i,
                                    Some(node.find_attrib("h").and_then(|h| h.parse::<u32>().ok())),
                                )),
                                _ => None,
                            };
                        }
                    }
                    None
                });
            let Some((index, h)) = control else {
                return Ok(());
            };
            match h {
                None => self.send_sm_ack(self.sm.inbound_h).await?,
                Some(Some(h)) => self.sm.try_process_ack(h)?,
                Some(None) => return Err(IksError::BadXml),
            }
            self.pending_events.remove(index);
        }
    }

    /// Resume on a new, already-open stream and replay only unacknowledged stanzas.
    pub async fn resume_stream_management(&mut self) -> Result<()> {
        let id = self.sm.sm_id.clone().ok_or(IksError::NetNotSupp)?;
        self.send_stanza(&crate::build_sm_resume(&id, self.sm.inbound_h))
            .await?;
        let response = self.recv_sm_response("resumed").await?;
        let resumed = crate::parse_sm_resumed(&response.borrow()).ok_or(IksError::NetRwErr)?;
        if resumed.previd != id {
            return Err(IksError::BadXml);
        }
        self.sm.try_process_ack(resumed.h)?;
        let replay: Vec<_> = self
            .sm
            .unacked_queue
            .iter()
            .map(ToString::to_string)
            .collect();
        for xml in replay {
            self.send_raw(&xml).await?;
        }
        self.sm.enabled = true;
        Ok(())
    }

    /// Reconnect, restore TLS if it was in use, resume, and replay the outstanding queue.
    pub async fn reconnect_and_resume(&mut self, host: &str, port: u16) -> Result<()> {
        let secure = matches!(self.stream, Some(AsyncConnectionStream::Tls(_)));
        let old_state = self.sm.clone();
        let mut replacement = Self::connect(host, port, &self.domain, self.timeout).await?;
        replacement.allow_insecure_tls = self.allow_insecure_tls;
        replacement.traffic = self.traffic.clone();
        replacement.start_stream().await?;
        let _features = replacement.recv_stanza().await?;
        if secure {
            replacement.start_tls().await?;
            let _ = replacement.recv_stanza().await?;
        }
        replacement.sm = old_state;
        replacement.sm.enabled = false;
        *self = replacement;
        self.resume_stream_management().await
    }

    /// Sends an XEP-0198 stanza acknowledgment for sequence number `h`.
    pub async fn send_sm_ack(&mut self, h: u32) -> Result<()> {
        let ack_stanza = crate::xep::build_sm_ack(h);
        self.send_stanza(&ack_stanza).await
    }

    /// Sends an XEP-0198 acknowledgment request `<r/>`.
    pub async fn request_sm_ack(&mut self) -> Result<()> {
        let req_stanza = crate::xep::build_sm_request_ack();
        self.send_stanza(&req_stanza).await
    }

    /// Performs the asynchronous RFC 6120 StartTLS upgrade handshake.
    pub async fn start_tls(&mut self) -> Result<()> {
        let starttls_stanza = "<starttls xmlns=\"urn:ietf:params:xml:ns:xmpp-tls\"/>";
        self.send_raw(starttls_stanza).await?;

        let response = self.recv_stanza().await?;
        if response.name().as_deref() != Some("proceed") {
            return Err(IksError::NetTlsFail);
        }

        let plain_stream = match self.stream.take() {
            Some(AsyncConnectionStream::Plain(s)) => s,
            _ => return Err(IksError::NetTlsFail),
        };

        let mut builder = TlsConnector::builder();
        if self.allow_insecure_tls {
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
        }
        let connector = builder.build().map_err(|_| IksError::NetTlsFail)?;
        let tokio_connector = TokioTlsConnector::from(connector);

        let tls_fut = tokio_connector.connect(&self.domain, plain_stream);
        let tls_stream = if let Some(t) = self.timeout {
            tokio::time::timeout(t, tls_fut)
                .await
                .map_err(|_| IksError::NetTlsFail)?
                .map_err(|_| IksError::NetTlsFail)?
        } else {
            tls_fut.await.map_err(|_| IksError::NetTlsFail)?
        };

        self.stream = Some(AsyncConnectionStream::Tls(tls_stream));
        self.parser.reset();
        self.utf8.reset();
        self.pending_events.clear();

        self.start_stream().await?;
        Ok(())
    }

    /// Gracefully closes the XMPP stream.
    pub async fn close(&mut self) -> Result<()> {
        let _ = self.send_raw("</stream:stream>").await;
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.shutdown().await;
        }
        Ok(())
    }
}

/// Asynchronously authenticates with SASL PLAIN (RFC 6120 Section 6.3.8).
pub async fn authenticate_plain_async(
    conn: &mut AsyncConnection,
    username: &str,
    password: &str,
    authzid: Option<&str>,
) -> Result<()> {
    let mut payload = Vec::new();
    if let Some(zid) = authzid {
        payload.extend_from_slice(zid.as_bytes());
    }
    payload.push(0);
    payload.extend_from_slice(username.as_bytes());
    payload.push(0);
    payload.extend_from_slice(password.as_bytes());

    let encoded = base64_encode(&payload);
    let auth_xml = format!(
        "<auth xmlns=\"urn:ietf:params:xml:ns:xmpp-sasl\" mechanism=\"PLAIN\">{}</auth>",
        encoded
    );

    conn.send_raw(&auth_xml).await?;
    let resp = conn.recv_stanza().await?;

    if resp.name().as_deref() == Some("success") {
        conn.start_stream().await?;
        Ok(())
    } else {
        Err(IksError::NetUnknown)
    }
}

/// Asynchronously requests resource binding (RFC 6120 Section 7).
pub async fn bind_resource_async(
    conn: &mut AsyncConnection,
    resource: Option<&str>,
) -> Result<String> {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", "bind_async");

    let mut bind = IksNode::new_tag("bind");
    bind.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-bind");

    if let Some(res) = resource {
        let mut res_node = IksNode::new_tag("resource");
        let mut cdata = IksNode::new(crate::IksType::CData);
        cdata.set_content(res);
        res_node.add_child(cdata);
        bind.add_child(res_node);
    }
    iq.add_child(bind);

    conn.send_stanza(&iq).await?;
    let resp = conn.recv_iq_response("bind_async").await?;

    if resp.find_attrib("type").as_deref() == Some("result") {
        if let Some(jid) = resp.find_path_text(&["bind", "jid"]) {
            return Ok(jid);
        }
    }

    Err(IksError::NetUnknown)
}

/// Asynchronously authenticates using SASL SCRAM-SHA-1 (RFC 5802 / RFC 6120).
pub async fn authenticate_scram_sha1_async(
    conn: &mut AsyncConnection,
    authcid: &str,
    password: &str,
) -> Result<()> {
    authenticate_scram_async_internal(conn, crate::sasl::ScramHash::Sha1, authcid, password).await
}

/// Asynchronously authenticates using SASL SCRAM-SHA-256 (RFC 7677 / RFC 6120).
pub async fn authenticate_scram_sha256_async(
    conn: &mut AsyncConnection,
    authcid: &str,
    password: &str,
) -> Result<()> {
    authenticate_scram_async_internal(conn, crate::sasl::ScramHash::Sha256, authcid, password).await
}

async fn authenticate_scram_async_internal(
    conn: &mut AsyncConnection,
    hash: crate::sasl::ScramHash,
    authcid: &str,
    password: &str,
) -> Result<()> {
    let mut client = crate::sasl::ScramClient::new(hash, authcid, password);
    let first_msg = client.client_first_message();
    let first_b64 = base64_encode(first_msg.as_bytes());

    let mech_name = match hash {
        crate::sasl::ScramHash::Sha1 => "SCRAM-SHA-1",
        crate::sasl::ScramHash::Sha256 => "SCRAM-SHA-256",
    };

    let mut auth_node = IksNode::new_tag("auth");
    auth_node.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-sasl");
    auth_node.add_attribute("mechanism", mech_name);
    auth_node.insert_cdata(first_b64);

    conn.send_stanza(&auth_node).await?;
    let challenge_node = conn.recv_stanza().await?;

    if challenge_node.name().as_deref() != Some("challenge") {
        return Err(IksError::NetRwErr);
    }
    let challenge_b64 = challenge_node.text();
    let challenge_raw_bytes = base64_decode(&challenge_b64)?;
    let challenge_raw = String::from_utf8(challenge_raw_bytes).map_err(|_| IksError::BadXml)?;

    let final_msg = client.process_challenge(&challenge_raw)?;
    let final_b64 = base64_encode(final_msg.as_bytes());

    let mut response_node = IksNode::new_tag("response");
    response_node.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-sasl");
    response_node.insert_cdata(final_b64);

    conn.send_stanza(&response_node).await?;
    let success_node = conn.recv_stanza().await?;

    if success_node.name().as_deref() != Some("success") {
        return Err(IksError::NetRwErr);
    }
    let success_b64 = success_node.text();
    let success_raw_bytes = base64_decode(&success_b64)?;
    let success_raw = String::from_utf8(success_raw_bytes).map_err(|_| IksError::BadXml)?;

    client.verify_success(&success_raw)?;

    conn.start_stream().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_async_recv_iq_response_interleaved_stanzas() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server_task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];

            let _ = socket.read(&mut buf).await.unwrap();
            let server_hdr = "<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' from='example.com' version='1.0'>";
            socket.write_all(server_hdr.as_bytes()).await.unwrap();

            let stanzas = concat!(
                "<presence from='alice@example.com'><show>chat</show></presence>",
                "<message from='bob@example.com'><body>hello</body></message>",
                "<iq id='async_iq_1' type='result'><query xmlns='test'/></iq>",
                "<presence from='carol@example.com'><show>dnd</show></presence>"
            );
            socket.write_all(stanzas.as_bytes()).await.unwrap();
        });

        let mut conn = AsyncConnection::connect(
            "127.0.0.1",
            port,
            "example.com",
            Some(std::time::Duration::from_secs(5)),
        )
        .await
        .unwrap();
        conn.start_stream().await.unwrap();

        // Must receive expected IQ despite interleaved presence and message
        let iq = conn.recv_iq_response("async_iq_1").await.unwrap();
        assert_eq!(iq.name().as_deref(), Some("iq"));
        assert_eq!(iq.find_attrib("id").as_deref(), Some("async_iq_1"));
        assert_eq!(iq.find_attrib("type").as_deref(), Some("result"));

        // Interleaved presence and message must still be in pending queue in order
        let st1 = conn.recv_stanza().await.unwrap();
        assert_eq!(st1.name().as_deref(), Some("presence"));
        assert_eq!(
            st1.find_attrib("from").as_deref(),
            Some("alice@example.com")
        );

        let st2 = conn.recv_stanza().await.unwrap();
        assert_eq!(st2.name().as_deref(), Some("message"));
        assert_eq!(st2.find_attrib("from").as_deref(), Some("bob@example.com"));

        let st3 = conn.recv_stanza().await.unwrap();
        assert_eq!(st3.name().as_deref(), Some("presence"));
        assert_eq!(
            st3.find_attrib("from").as_deref(),
            Some("carol@example.com")
        );

        server_task.await.unwrap();
    }
}
