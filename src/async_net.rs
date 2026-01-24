/*
            iksemel - XML parser for Rust
          Copyright (C) 2026 Süleyman Poyraz
  This code is free software; you can redistribute it and/or
  modify it under the terms of the Affero General Public License
  as published by the Free Software Foundation; either version 3
  of the License, or (at your option) any later version.
*/

use native_tls::TlsConnector;
use std::collections::VecDeque;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_native_tls::{TlsConnector as TokioTlsConnector, TlsStream};

use crate::{base64_encode, IksError, IksNode, Result, StreamEvent, StreamParser};

/// Underlying non-blocking transport stream (Plain TCP or TLS encrypted).
pub enum AsyncConnectionStream {
    Plain(TcpStream),
    Tls(TlsStream<TcpStream>),
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
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            AsyncConnectionStream::Plain(s) => std::pin::Pin::new(s).poll_flush(cx),
            AsyncConnectionStream::Tls(s) => std::pin::Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            AsyncConnectionStream::Plain(s) => std::pin::Pin::new(s).poll_shutdown(cx),
            AsyncConnectionStream::Tls(s) => std::pin::Pin::new(s).poll_shutdown(cx),
        }
    }
}

/// An asynchronous non-blocking client connection to an XMPP server built on Tokio.
pub struct AsyncConnection {
    stream: Option<AsyncConnectionStream>,
    parser: StreamParser,
    pending_events: VecDeque<StreamEvent>,
    domain: String,
    timeout: Option<Duration>,
    allow_insecure_tls: bool,
    log_traffic: bool,
}

impl AsyncConnection {
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
            pending_events: VecDeque::new(),
            domain: domain.to_string(),
            timeout,
            allow_insecure_tls: false,
            log_traffic: false,
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
        Ok(())
    }

    /// Sends the opening `<stream:stream>` XML declaration.
    pub async fn start_stream(&mut self) -> Result<()> {
        self.parser.reset();
        self.pending_events.clear();

        let header = format!(
            "<?xml version=\"1.0\"?><stream:stream to=\"{}\" xmlns=\"jabber:client\" xmlns:stream=\"http://etherx.jabber.org/streams\" version=\"1.0\">",
            self.domain
        );
        self.send_raw(&header).await
    }

    /// Serializes and sends an XML stanza.
    pub async fn send_stanza(&mut self, node: &IksNode) -> Result<()> {
        self.send_raw(&node.to_string()).await
    }

    /// Receives the next stream event, reading chunks from the socket as needed.
    pub async fn recv_event(&mut self) -> Result<StreamEvent> {
        loop {
            if let Some(event) = self.pending_events.pop_front() {
                return Ok(event);
            }

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

            let chunk_str = std::str::from_utf8(&buf[..n]).map_err(|_| IksError::BadXml)?;
            if self.log_traffic {
                eprintln!("[XMPP ASYNC IN] {}", chunk_str);
            }

            let events = self.parser.parse_chunk(chunk_str)?;
            self.pending_events.extend(events);
        }
    }

    /// Receives the next complete XML stanza, skipping stream headers.
    pub async fn recv_stanza(&mut self) -> Result<IksNode> {
        loop {
            match self.recv_event().await? {
                StreamEvent::Stanza(node) => return Ok(node),
                StreamEvent::StreamEnd => return Err(IksError::NetDropped),
                _ => {}
            }
        }
    }

    /// Performs the asynchronous RFC 6120 StartTLS upgrade handshake.
    pub async fn start_tls(&mut self) -> Result<()> {
        let starttls_stanza = "<starttls xmlns=\"urn:ietf:params:xml:ns:xmpp-tls\"/>";
        self.send_raw(starttls_stanza).await?;

        let response = self.recv_stanza().await?;
        if response.name() != Some("proceed") {
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

    if resp.name() == Some("success") {
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
    let resp = conn.recv_stanza().await?;

    if resp.find_attrib("type") == Some("result") {
        if let Some(jid) = resp.find_path_text(&["bind", "jid"]) {
            return Ok(jid);
        }
    }

    Err(IksError::NetUnknown)
}
