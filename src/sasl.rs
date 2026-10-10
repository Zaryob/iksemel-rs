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

use crate::{
    Connection, IksError, IksNode, Jid, Result, base64_decode, base64_encode, hmac_sha1,
    hmac_sha256, pbkdf2_hmac_sha1, pbkdf2_hmac_sha256, sha1_hash, sha1_hex, sha256_hash,
};

/// Supported SASL mechanisms for XMPP authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaslMechanism {
    Plain,
    Anonymous,
    ScramSha1,
    ScramSha256,
}

impl SaslMechanism {
    pub fn as_str(&self) -> &'static str {
        match self {
            SaslMechanism::Plain => "PLAIN",
            SaslMechanism::Anonymous => "ANONYMOUS",
            SaslMechanism::ScramSha1 => "SCRAM-SHA-1",
            SaslMechanism::ScramSha256 => "SCRAM-SHA-256",
        }
    }
}

/// Supported hash algorithm for SCRAM authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScramHash {
    Sha1,
    Sha256,
}

fn escape_username(username: &str) -> String {
    username.replace('=', "=3D").replace(',', "=2C")
}

fn generate_nonce() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.as_nanos();
    let hash = sha256_hash(&nanos.to_be_bytes());
    base64_encode(&hash[..18])
}

/// Client-side state machine for SCRAM authentication (RFC 5802 / RFC 7677).
pub struct ScramClient {
    hash: ScramHash,
    authcid: String,
    password: String,
    client_nonce: String,
    client_first_bare: String,
    server_first: String,
    server_signature: Vec<u8>,
}

impl ScramClient {
    pub fn new(hash: ScramHash, authcid: impl Into<String>, password: impl Into<String>) -> Self {
        let authcid = authcid.into();
        let password = password.into();
        let client_nonce = generate_nonce();
        let client_first_bare = format!("n={},r={}", escape_username(&authcid), client_nonce);
        Self {
            hash,
            authcid,
            password,
            client_nonce,
            client_first_bare,
            server_first: String::new(),
            server_signature: Vec::new(),
        }
    }

    /// Sets a custom client nonce (primarily for deterministic testing with RFC vectors).
    pub fn with_nonce(mut self, nonce: impl Into<String>) -> Self {
        self.client_nonce = nonce.into();
        self.client_first_bare = format!(
            "n={},r={}",
            escape_username(&self.authcid),
            self.client_nonce
        );
        self
    }

    /// Generates the client-first-message (RFC 5802 Section 3).
    pub fn client_first_message(&self) -> String {
        format!("n,,{}", self.client_first_bare)
    }

    /// Processes the server's challenge (server-first-message) and returns the client-final-message.
    pub fn process_challenge(&mut self, challenge_raw: &str) -> Result<String> {
        self.server_first = challenge_raw.trim().to_string();

        let mut combined_nonce = None;
        let mut salt_b64 = None;
        let mut iterations = None;

        for part in self.server_first.split(',') {
            if let Some(r) = part.strip_prefix("r=") {
                combined_nonce = Some(r);
            } else if let Some(s) = part.strip_prefix("s=") {
                salt_b64 = Some(s);
            } else if let Some(i) = part.strip_prefix("i=") {
                iterations = i.parse::<u32>().ok();
            }
        }

        let combined_nonce = combined_nonce.ok_or(IksError::NetRwErr)?;
        let salt_b64 = salt_b64.ok_or(IksError::NetRwErr)?;
        let iterations = iterations.ok_or(IksError::NetRwErr)?;

        if !combined_nonce.starts_with(&self.client_nonce) {
            return Err(IksError::NetRwErr);
        }
        if iterations == 0 {
            return Err(IksError::NetRwErr);
        }

        let salt_bytes = base64_decode(salt_b64)?;
        let client_final_without_proof = format!("c=biws,r={}", combined_nonce);
        let auth_message = format!(
            "{},{},{}",
            self.client_first_bare, self.server_first, client_final_without_proof
        );

        let proof_b64 = match self.hash {
            ScramHash::Sha1 => {
                let mut salted_password = [0u8; 20];
                pbkdf2_hmac_sha1(
                    self.password.as_bytes(),
                    &salt_bytes,
                    iterations,
                    &mut salted_password,
                );

                let client_key = hmac_sha1(&salted_password, b"Client Key");
                let stored_key = sha1_hash(&client_key);
                let client_signature = hmac_sha1(&stored_key, auth_message.as_bytes());

                let mut client_proof = [0u8; 20];
                for i in 0..20 {
                    client_proof[i] = client_key[i] ^ client_signature[i];
                }

                let server_key = hmac_sha1(&salted_password, b"Server Key");
                let server_signature = hmac_sha1(&server_key, auth_message.as_bytes());
                self.server_signature = server_signature.to_vec();

                base64_encode(&client_proof)
            }
            ScramHash::Sha256 => {
                let mut salted_password = [0u8; 32];
                pbkdf2_hmac_sha256(
                    self.password.as_bytes(),
                    &salt_bytes,
                    iterations,
                    &mut salted_password,
                );

                let client_key = hmac_sha256(&salted_password, b"Client Key");
                let stored_key = sha256_hash(&client_key);
                let client_signature = hmac_sha256(&stored_key, auth_message.as_bytes());

                let mut client_proof = [0u8; 32];
                for i in 0..32 {
                    client_proof[i] = client_key[i] ^ client_signature[i];
                }

                let server_key = hmac_sha256(&salted_password, b"Server Key");
                let server_signature = hmac_sha256(&server_key, auth_message.as_bytes());
                self.server_signature = server_signature.to_vec();

                base64_encode(&client_proof)
            }
        };

        Ok(format!("{},p={}", client_final_without_proof, proof_b64))
    }

    /// Verifies the server's final message containing the server signature (v=...).
    pub fn verify_success(&self, success_raw: &str) -> Result<()> {
        let trimmed = success_raw.trim();
        for part in trimmed.split(',') {
            if let Some(v_b64) = part.strip_prefix("v=") {
                let server_sig_bytes = base64_decode(v_b64)?;
                if server_sig_bytes == self.server_signature {
                    return Ok(());
                } else {
                    return Err(IksError::NetRwErr);
                }
            }
        }
        Err(IksError::NetRwErr)
    }
}

/// Authenticates using SASL SCRAM-SHA-1 (RFC 5802 / RFC 6120).
pub fn authenticate_scram_sha1(conn: &mut Connection, authcid: &str, password: &str) -> Result<()> {
    authenticate_scram_internal(conn, ScramHash::Sha1, authcid, password)
}

/// Authenticates using SASL SCRAM-SHA-256 (RFC 7677 / RFC 6120).
pub fn authenticate_scram_sha256(
    conn: &mut Connection,
    authcid: &str,
    password: &str,
) -> Result<()> {
    authenticate_scram_internal(conn, ScramHash::Sha256, authcid, password)
}

fn authenticate_scram_internal(
    conn: &mut Connection,
    hash: ScramHash,
    authcid: &str,
    password: &str,
) -> Result<()> {
    let mut client = ScramClient::new(hash, authcid, password);
    let first_msg = client.client_first_message();
    let first_b64 = base64_encode(first_msg.as_bytes());

    let mech_name = match hash {
        ScramHash::Sha1 => "SCRAM-SHA-1",
        ScramHash::Sha256 => "SCRAM-SHA-256",
    };

    let mut auth_node = IksNode::new_tag("auth");
    auth_node.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-sasl");
    auth_node.add_attribute("mechanism", mech_name);
    auth_node.insert_cdata(first_b64);

    conn.send_stanza(&auth_node)?;
    let challenge_node = conn.recv_stanza()?;

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

    conn.send_stanza(&response_node)?;
    let success_node = conn.recv_stanza()?;

    if success_node.name().as_deref() != Some("success") {
        return Err(IksError::NetRwErr);
    }
    let success_b64 = success_node.text();
    let success_raw_bytes = base64_decode(&success_b64)?;
    let success_raw = String::from_utf8(success_raw_bytes).map_err(|_| IksError::BadXml)?;

    client.verify_success(&success_raw)?;

    // Stream restart is mandatory after SASL success (RFC 6120 6.4.6)
    conn.start_stream()?;
    Ok(())
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
            if child_ref.name() == Some("mechanism")
                && let Some(cdata) = child_ref
                    .children()
                    .iter()
                    .find(|c| c.borrow().content().is_some())
                && let Some(name) = cdata.borrow().content()
            {
                mechanisms.push(name.trim().to_string());
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
    match response.name().as_deref() {
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

    let resp = conn.recv_iq_response("bind_1")?;
    if resp.find_attrib("type").as_deref() != Some("result") {
        return Err(IksError::NetRwErr);
    }

    let bind_child = resp.find("bind").ok_or(IksError::BadXml)?;
    let jid_str = bind_child.find_cdata("jid").ok_or(IksError::BadXml)?;
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

    let resp = conn.recv_iq_response("sess_1")?;
    if resp.find_attrib("type").as_deref() == Some("result") {
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

    let resp = conn.recv_iq_response("auth_legacy")?;
    if resp.find_attrib("type").as_deref() == Some("result") {
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

    #[test]
    fn test_bind_resource_interleaved_stanzas() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];

            // 1. Initial stream header
            let _ = stream.read(&mut buf).unwrap();
            let server_hdr = "<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' from='example.com' version='1.0'>";
            stream.write_all(server_hdr.as_bytes()).unwrap();

            // 2. Read bind iq
            let n = stream.read(&mut buf).unwrap();
            let bind_req = std::str::from_utf8(&buf[..n]).unwrap();
            assert!(bind_req.contains("id='bind_1'") || bind_req.contains("id=\"bind_1\""));

            // 3. Send interleaved presence and message BEFORE bind IQ result
            let interleaved = concat!(
                "<presence from='other@example.com'><show>away</show></presence>",
                "<message from='notify@example.com'><body>welcome</body></message>",
                "<iq id='bind_1' type='result'>",
                "<bind xmlns='urn:ietf:params:xml:ns:xmpp-bind'>",
                "<jid>alice@example.com/mobile</jid>",
                "</bind></iq>"
            );
            stream.write_all(interleaved.as_bytes()).unwrap();
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

        // bind_resource must correlate and succeed despite interleaved presence & message
        let bound_jid = bind_resource(&mut conn, Some("mobile")).expect("bind succeeds");
        assert_eq!(bound_jid.full(), "alice@example.com/mobile");

        // The interleaved stanzas must still be in connection queue
        let pres = conn.recv_stanza().expect("presence received");
        assert_eq!(pres.name().as_deref(), Some("presence"));
        assert_eq!(
            pres.find_attrib("from").as_deref(),
            Some("other@example.com")
        );

        let msg = conn.recv_stanza().expect("message received");
        assert_eq!(msg.name().as_deref(), Some("message"));
        assert_eq!(
            msg.find_attrib("from").as_deref(),
            Some("notify@example.com")
        );

        handle.join().unwrap();
    }
}
