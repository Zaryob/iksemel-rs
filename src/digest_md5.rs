//! Legacy DIGEST-MD5 (RFC 2831), authentication-only qop.
use crate::{Connection, IksError, IksNode, Result, base64_decode, base64_encode};
use md5::{Digest, Md5};

pub fn md5_hash(data: &[u8]) -> [u8; 16] {
    Md5::digest(data).into()
}
pub fn md5_hex(data: &[u8]) -> String {
    hex::encode(md5_hash(data))
}
#[derive(Clone, Default)]
pub struct Md5Context(Md5);
impl Md5Context {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn reset(&mut self) {
        self.0 = Md5::new();
    }
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }
    pub fn digest(&self) -> [u8; 16] {
        self.0.clone().finalize().into()
    }
    pub fn hex(&self) -> String {
        hex::encode(self.digest())
    }
}
#[derive(Clone, Default)]
pub struct Sha1Context(sha1::Sha1);
impl Sha1Context {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn reset(&mut self) {
        self.0 = sha1::Sha1::new();
    }
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }
    pub fn digest(&self) -> [u8; 20] {
        self.0.clone().finalize().into()
    }
    pub fn hex(&self) -> String {
        hex::encode(self.digest())
    }
}
pub fn random_nonce() -> Result<String> {
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce).map_err(|_| IksError::NetUnknown)?;
    Ok(base64_encode(&nonce))
}
fn directives(input: &str) -> Result<Vec<(String, String)>> {
    if input.len() > 16384 {
        return Err(IksError::BadXml);
    }
    let mut out = Vec::new();
    let mut rest = input.trim();
    while !rest.is_empty() {
        let (key, value) = rest.split_once('=').ok_or(IksError::BadXml)?;
        let key = key.trim();
        if key.is_empty() || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-') {
            return Err(IksError::BadXml);
        }
        rest = value.trim_start();
        let value;
        if rest.starts_with('"') {
            let mut text = String::new();
            let mut escaped = false;
            let mut end = None;
            for (i, c) in rest[1..].char_indices() {
                if escaped {
                    text.push(c);
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    end = Some(i + 2);
                    break;
                } else if c.is_control() {
                    return Err(IksError::BadXml);
                } else {
                    text.push(c);
                }
            }
            rest = &rest[end.ok_or(IksError::BadXml)?..];
            value = text;
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            value = rest[..end].trim().to_string();
            if value.is_empty() || value.chars().any(|c| c.is_control() || c == '"') {
                return Err(IksError::BadXml);
            }
            rest = &rest[end..];
        }
        out.push((key.to_string(), value));
        rest = rest.trim_start();
        if rest.is_empty() {
            break;
        }
        rest = rest.strip_prefix(',').ok_or(IksError::BadXml)?.trim_start();
        if rest.is_empty() {
            return Err(IksError::BadXml);
        }
    }
    Ok(out)
}
fn quote(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn credential(value: &str, utf8: bool) -> Result<Vec<u8>> {
    if utf8 {
        return Ok(value.as_bytes().to_vec());
    }
    value
        .chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| IksError::BadXml))
        .collect()
}
pub struct DigestMd5Client {
    username: String,
    password: String,
    uri: String,
    cnonce: String,
    expected: Option<String>,
    verified: bool,
}
impl DigestMd5Client {
    pub fn new(username: &str, password: &str, service: &str, host: &str) -> Result<Self> {
        Ok(Self::with_cnonce(
            username,
            password,
            service,
            host,
            &random_nonce()?,
        ))
    }
    pub fn with_cnonce(
        username: &str,
        password: &str,
        service: &str,
        host: &str,
        cnonce: &str,
    ) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
            uri: format!("{service}/{host}"),
            cnonce: cnonce.into(),
            expected: None,
            verified: false,
        }
    }
    pub fn process_challenge(&mut self, challenge: &str) -> Result<String> {
        if self.expected.is_some() {
            return Err(IksError::NetRwErr);
        }
        let fields = directives(challenge)?;
        let field = |key: &str| -> Result<Option<&str>> {
            let mut values = fields
                .iter()
                .filter(|(k, _)| k == key)
                .map(|(_, v)| v.as_str());
            let first = values.next();
            if key != "realm" && values.next().is_some() {
                return Err(IksError::BadXml);
            }
            Ok(first)
        };
        let nonce = field("nonce")?
            .filter(|n| !n.is_empty())
            .ok_or(IksError::BadXml)?;
        if field("algorithm")? != Some("md5-sess") {
            return Err(IksError::NetNotSupp);
        }
        if !field("qop")?
            .unwrap_or("auth")
            .split(',')
            .any(|q| q.trim() == "auth")
        {
            return Err(IksError::NetNotSupp);
        }
        let realm = field("realm")?.unwrap_or("");
        let utf8 = match field("charset")? {
            None => false,
            Some("utf-8") => true,
            _ => return Err(IksError::NetNotSupp),
        };
        let mut initial = credential(&self.username, utf8)?;
        initial.push(b':');
        initial.extend(credential(realm, utf8)?);
        initial.push(b':');
        initial.extend(credential(&self.password, utf8)?);
        let mut a1 = md5_hash(&initial).to_vec();
        a1.extend(format!(":{nonce}:{}", self.cnonce).as_bytes());
        let ha1 = md5_hex(&a1);
        let response = |method: &str| {
            md5_hex(
                format!(
                    "{ha1}:{nonce}:00000001:{}:auth:{}",
                    self.cnonce,
                    md5_hex(format!("{method}:{}", self.uri).as_bytes())
                )
                .as_bytes(),
            )
        };
        let result = format!(
            "username=\"{}\",realm=\"{}\",nonce=\"{}\",cnonce=\"{}\",nc=00000001,qop=auth,digest-uri=\"{}\",response={}{}",
            quote(&self.username),
            quote(realm),
            quote(nonce),
            quote(&self.cnonce),
            quote(&self.uri),
            response("AUTHENTICATE"),
            if utf8 { ",charset=utf-8" } else { "" }
        );
        self.expected = Some(response(""));
        Ok(result)
    }
    pub fn verify_rspauth(&mut self, challenge: &str) -> Result<()> {
        if self.verified {
            return Err(IksError::NetRwErr);
        }
        let fields = directives(challenge)?;
        let values: Vec<_> = fields.iter().filter(|(k, _)| k == "rspauth").collect();
        if values.len() != 1 {
            return Err(IksError::BadXml);
        }
        let expected = self.expected.as_ref().ok_or(IksError::NetRwErr)?;
        let actual = &values[0].1;
        if actual.len() != expected.len()
            || actual
                .bytes()
                .zip(expected.bytes())
                .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                != 0
        {
            return Err(IksError::NetRwErr);
        }
        self.verified = true;
        Ok(())
    }
}
fn sasl_node(name: &str, text: Option<&str>) -> IksNode {
    let mut node = IksNode::new_tag(name);
    node.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-sasl");
    if let Some(text) = text {
        node.insert_cdata(base64_encode(text.as_bytes()));
    }
    node
}
fn decoded(node: &crate::NodeRef) -> Result<String> {
    String::from_utf8(base64_decode(&node.text())?).map_err(|_| IksError::BadXml)
}
pub fn authenticate_digest_md5(conn: &mut Connection, user: &str, password: &str) -> Result<()> {
    let mut client = DigestMd5Client::new(user, password, "xmpp", conn.domain())?;
    let mut auth = sasl_node("auth", None);
    auth.add_attribute("mechanism", "DIGEST-MD5");
    conn.send_stanza(&auth)?;
    let challenge = conn.recv_stanza()?;
    if challenge.name().as_deref() != Some("challenge") {
        return Err(IksError::NetRwErr);
    }
    let response = client.process_challenge(&decoded(&challenge)?)?;
    conn.send_stanza(&sasl_node("response", Some(&response)))?;
    let proof = conn.recv_stanza()?;
    if proof.name().as_deref() != Some("challenge") {
        return Err(IksError::NetRwErr);
    }
    client.verify_rspauth(&decoded(&proof)?)?;
    conn.send_stanza(&sasl_node("response", None))?;
    if conn.recv_stanza()?.name().as_deref() != Some("success") {
        return Err(IksError::NetRwErr);
    }
    conn.start_stream()?;
    Ok(())
}
pub async fn authenticate_digest_md5_async(
    conn: &mut crate::AsyncConnection,
    user: &str,
    password: &str,
) -> Result<()> {
    let mut client = DigestMd5Client::new(user, password, "xmpp", conn.domain())?;
    let mut auth = sasl_node("auth", None);
    auth.add_attribute("mechanism", "DIGEST-MD5");
    conn.send_stanza(&auth).await?;
    let challenge = conn.recv_stanza().await?;
    if challenge.name().as_deref() != Some("challenge") {
        return Err(IksError::NetRwErr);
    }
    let response = client.process_challenge(&decoded(&challenge)?)?;
    conn.send_stanza(&sasl_node("response", Some(&response)))
        .await?;
    let proof = conn.recv_stanza().await?;
    if proof.name().as_deref() != Some("challenge") {
        return Err(IksError::NetRwErr);
    }
    client.verify_rspauth(&decoded(&proof)?)?;
    conn.send_stanza(&sasl_node("response", None)).await?;
    if conn.recv_stanza().await?.name().as_deref() != Some("success") {
        return Err(IksError::NetRwErr);
    }
    conn.start_stream().await?;
    Ok(())
}
