/*
            iksemel - XML parser for Rust
          Copyright (C) 2024 Süleyman Poyraz
 This code is free software; you can redistribute it and/or
 modify it under the terms of the Affero General Public License
 as published by the Free Software Foundation; either version 3
 of the License, or (at your option) any later version.
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
*/

use crate::{base64_encode, sha1_hex, Connection, IksError, IksNode, Jid, Result};

/// Supported SASL mechanisms for XMPP authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaslMechanism {
    Plain,
    Anonymous,
}

impl SaslMechanism {
    pub fn as_str(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
            SaslMechanism::Anonymous => "ANONYMOUS",
        }
    }
}

/// Parses advertised SASL mechanisms from a `<stream:features>` stanza.
pub fn parse_features_mechanisms(features: &IksNode) -> Vec<String> {
    let mut mechanisms = Vec::new();
    if let Some(mechs_node) = features.children().iter().find(|c| {
        let n = c.borrow();
        n.name() == Some("mechanisms")
            && n.find_attrib("xmlns") == Some("urn:ietf:params:xml:ns:xmpp-sasl")
    }) {
        for child in mechs_node.borrow().children() {
            let child_ref = child.borrow();
            if child_ref.name() == Some("mechanism") {
                if let Some(cdata) = child_ref
                    .children()
                    .iter()
                    .find(|c| c.borrow().content().is_some())
                {
                    if let Some(name) = cdata.borrow().content() {
                        mechanisms.push(name.trim().to_string());
                    }
                }
            }
        }
    }
    mechanisms
}

/// Authenticates using SASL PLAIN (RFC 6120 Section 6, RFC 4616).
///
/// On successful authentication, automatically resets the stream parser
/// and restarts the XMPP stream.
pub fn authenticate_plain(
    conn: &mut Connection,
    authcid: &str,
    password: &str,
    authzid: Option<&str>,
) -> Result<()> {
    let mut raw_auth = Vec::new();
    if let Some(azid) = authzid {
        raw_auth.extend_from_slice(azid.as_bytes());
    }
    raw_auth.push(0);
    raw_auth.extend_from_slice(authcid.as_bytes());
    raw_auth.push(0);
    raw_auth.extend_from_slice(password.as_bytes());

    let encoded = base64_encode(&raw_auth);

    let mut auth_node = IksNode::new_tag("auth");
    auth_node.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-sasl");
    auth_node.add_attribute("mechanism", "PLAIN");
    auth_node.insert_cdata(encoded);

    conn.send_stanza(&auth_node)?;

    let response = conn.recv_stanza()?;
    match response.name() {
        Some("success") => {
            // Stream restart is mandatory after SASL success (RFC 6120 6.4.6)
            conn.start_stream()?;
            Ok(())
        }
        Some("failure") => Err(IksError::NetRwErr),
        _ => Err(IksError::NetUnknown),
    }
}

/// Performs resource binding (RFC 6120 Section 7).
///
/// Returns the full JID assigned by the server.
pub fn bind_resource(conn: &mut Connection, resource: Option<&str>) -> Result<Jid> {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", "bind_1");

    let mut bind = IksNode::new_tag("bind");
    bind.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-bind");

    if let Some(res) = resource {
        let mut res_node = IksNode::new_tag("resource");
        res_node.insert_cdata(res);
        bind.add_child(res_node);
    }

    iq.add_child(bind);
    conn.send_stanza(&iq)?;

    let resp = conn.recv_stanza()?;
    if resp.find_attrib("type") != Some("result") {
        return Err(IksError::NetRwErr);
    }

    let bind_child = resp.find("bind").ok_or(IksError::BadXml)?;
    let jid_str = bind_child
        .borrow()
        .find_cdata("jid")
        .ok_or(IksError::BadXml)?;
    Jid::new(&jid_str)
}

/// Establishes an XMPP session (RFC 3921 / XEP-0186).
pub fn establish_session(conn: &mut Connection) -> Result<()> {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", "sess_1");

    let mut session = IksNode::new_tag("session");
    session.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-session");
    iq.add_child(session);

    conn.send_stanza(&iq)?;

    let resp = conn.recv_stanza()?;
    if resp.find_attrib("type") == Some("result") {
        Ok(())
    } else {
        Err(IksError::NetRwErr)
    }
}

/// Authenticates using Non-SASL authentication (XEP-0078).
///
/// Supported for legacy servers and backward compatibility with iksemel C tools.
pub fn authenticate_non_sasl(
    conn: &mut Connection,
    username: &str,
    password: &str,
    resource: &str,
    stream_id: Option<&str>,
) -> Result<()> {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", "auth_legacy");

    let mut query = IksNode::new_tag("query");
    query.add_attribute("xmlns", "jabber:iq:auth");

    let mut user_node = IksNode::new_tag("username");
    user_node.insert_cdata(username);
    query.add_child(user_node);

    let mut res_node = IksNode::new_tag("resource");
    res_node.insert_cdata(resource);
    query.add_child(res_node);

    if let Some(sid) = stream_id {
        // Use SHA-1 digest auth: sha1(stream_id + password)
        let hash_input = format!("{}{}", sid, password);
        let digest_str = sha1_hex(hash_input.as_bytes());
        let mut digest_node = IksNode::new_tag("digest");
        digest_node.insert_cdata(digest_str);
        query.add_child(digest_node);
    } else {
        // Plaintext legacy fallback
        let mut pass_node = IksNode::new_tag("password");
        pass_node.insert_cdata(password);
        query.add_child(pass_node);
    }

    iq.add_child(query);
    conn.send_stanza(&iq)?;

    let resp = conn.recv_stanza()?;
    if resp.find_attrib("type") == Some("result") {
        Ok(())
    } else {
        Err(IksError::NetRwErr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_parse_features_mechanisms() {
        let mut features = IksNode::new_tag("stream:features");
        let mut mechs = IksNode::new_tag("mechanisms");
        mechs.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-sasl");

        let mut m1 = IksNode::new_tag("mechanism");
        m1.insert_cdata("PLAIN");
        mechs.add_child(m1);

        let mut m2 = IksNode::new_tag("mechanism");
        m2.insert_cdata("SCRAM-SHA-1");
        mechs.add_child(m2);

        features.add_child(mechs);

        let found = parse_features_mechanisms(&features);
        assert_eq!(found, vec!["PLAIN".to_string(), "SCRAM-SHA-1".to_string()]);
    }

    #[test]
    fn test_mock_sasl_plain_and_bind_flow() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];

            // 1. Initial stream header from client
            let n = stream.read(&mut buf).unwrap();
            let _ = std::str::from_utf8(&buf[..n]).unwrap();

            // Server responds with stream header + features
            let resp1 = concat!(
                "<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' from='example.com' version='1.0'>",
                "<stream:features><mechanisms xmlns='urn:ietf:params:xml:ns:xmpp-sasl'><mechanism>PLAIN</mechanism></mechanisms></stream:features>"
            );
            stream.write_all(resp1.as_bytes()).unwrap();
            stream.flush().unwrap();

            // 2. Client sends <auth mechanism='PLAIN'>...</auth>
            let n = stream.read(&mut buf).unwrap();
            let auth_req = std::str::from_utf8(&buf[..n]).unwrap();
            assert!(
                auth_req.contains("mechanism='PLAIN'") || auth_req.contains("mechanism=\"PLAIN\"")
            );

            // Server responds with <success/>
            let success = "<success xmlns='urn:ietf:params:xml:ns:xmpp-sasl'/>";
            stream.write_all(success.as_bytes()).unwrap();
            stream.flush().unwrap();

            // 3. Client restarts stream header
            let n = stream.read(&mut buf).unwrap();
            let restart_req = std::str::from_utf8(&buf[..n]).unwrap();
            assert!(restart_req.contains("<stream:stream"));

            // Server responds with new stream header + bind feature
            let resp2 = concat!(
                "<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' from='example.com' version='1.0'>",
                "<stream:features><bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'/></stream:features>"
            );
            stream.write_all(resp2.as_bytes()).unwrap();
            stream.flush().unwrap();

            // 4. Client sends bind iq
            let n = stream.read(&mut buf).unwrap();
            let bind_req = std::str::from_utf8(&buf[..n]).unwrap();
            assert!(bind_req.contains("urn:ietf:params:xml:ns:xmpp-bind"));

            // Server responds with bind result
            let bind_resp = concat!(
                "<iq id='bind_1' type='result'>",
                "<bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'>",
                "<jid>alice@example.com/laptop</jid>",
                "</bind></iq>"
            );
            stream.write_all(bind_resp.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let mut conn = Connection::connect(
            "127.0.0.1",
            port,
            "example.com",
            Some(Duration::from_secs(5)),
        )
        .unwrap();
        conn.start_stream().unwrap();
        let _features = conn.recv_stanza().unwrap();

        authenticate_plain(&mut conn, "alice", "secret123", None).unwrap();
        let _restarted_features = conn.recv_stanza().unwrap();

        let bound_jid = bind_resource(&mut conn, Some("laptop")).unwrap();
        assert_eq!(bound_jid.full(), "alice@example.com/laptop");
        assert_eq!(bound_jid.resource(), Some("laptop"));

        handle.join().unwrap();
    }
}
