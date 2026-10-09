//! Stanza builders with the C iksemel field order and default behavior.
use crate::{IksNode, IksShowType, IksSubtype, PacketJid};
fn text_child(node: &mut IksNode, name: &str, text: &str) {
    let mut child = IksNode::new_tag(name);
    if !text.is_empty() {
        child.insert_cdata(text);
    }
    node.add_child(child);
}
pub fn make_message(kind: IksSubtype, to: Option<&str>, body: Option<&str>) -> IksNode {
    let mut node = IksNode::new_tag("message");
    let kind = match kind {
        IksSubtype::Chat => Some("chat"),
        IksSubtype::Groupchat => Some("groupchat"),
        IksSubtype::Headline => Some("headline"),
        _ => None,
    };
    if let Some(kind) = kind {
        node.add_attribute("type", kind);
    }
    if let Some(to) = to {
        node.add_attribute("to", to);
    }
    if let Some(body) = body {
        text_child(&mut node, "body", body);
    }
    node
}
pub fn make_subscription(kind: IksSubtype, to: Option<&str>, status: Option<&str>) -> IksNode {
    let mut node = IksNode::new_tag("presence");
    let kind = match kind {
        IksSubtype::Subscribe => Some("subscribe"),
        IksSubtype::Subscribed => Some("subscribed"),
        IksSubtype::Unsubscribe => Some("unsubscribe"),
        IksSubtype::Unsubscribed => Some("unsubscribed"),
        IksSubtype::Probe => Some("probe"),
        _ => None,
    };
    if let Some(kind) = kind {
        node.add_attribute("type", kind);
    }
    if let Some(to) = to {
        node.add_attribute("to", to);
    }
    if let Some(status) = status {
        text_child(&mut node, "status", status);
    }
    node
}
pub fn make_presence(show: IksShowType, status: Option<&str>) -> IksNode {
    let mut node = IksNode::new_tag("presence");
    let text = match show {
        IksShowType::Chat => Some("chat"),
        IksShowType::Away => Some("away"),
        IksShowType::Xa => Some("xa"),
        IksShowType::Dnd => Some("dnd"),
        _ => None,
    };
    if show == IksShowType::Unavailable {
        node.add_attribute("type", "unavailable");
    }
    if let Some(text) = text {
        text_child(&mut node, "show", text);
    }
    if let Some(status) = status {
        text_child(&mut node, "status", status);
    }
    node
}
pub fn make_iq(kind: IksSubtype, ns: Option<&str>) -> IksNode {
    let mut node = IksNode::new_tag("iq");
    let text = match kind {
        IksSubtype::Get => Some("get"),
        IksSubtype::Set => Some("set"),
        IksSubtype::Result => Some("result"),
        IksSubtype::Error => Some("error"),
        _ => None,
    };
    if let Some(text) = text {
        node.add_attribute("type", text);
    }
    let mut q = IksNode::new_tag("query");
    if let Some(ns) = ns {
        q.add_attribute("xmlns", ns);
    }
    node.add_child(q);
    node
}
pub fn make_auth(jid: &PacketJid, password: &str, stream_id: Option<&str>) -> IksNode {
    let mut node = IksNode::new_tag("iq");
    node.add_attribute("type", "set");
    let mut q = IksNode::new_tag("query");
    q.add_attribute("xmlns", "jabber:iq:auth");
    text_child(&mut q, "username", jid.user.as_deref().unwrap_or(""));
    text_child(&mut q, "resource", jid.resource.as_deref().unwrap_or(""));
    if let Some(id) = stream_id {
        text_child(
            &mut q,
            "digest",
            &crate::sha1_hex(format!("{id}{password}").as_bytes()),
        );
    } else {
        text_child(&mut q, "password", password);
    }
    node.add_child(q);
    node
}
pub fn make_resource_bind(jid: &PacketJid) -> IksNode {
    let mut node = IksNode::new_tag("iq");
    node.add_attribute("type", "set");
    let mut q = IksNode::new_tag("bind");
    q.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-bind");
    if let Some(r) = jid.resource.as_deref().filter(|r| !r.is_empty()) {
        text_child(&mut q, "resource", r);
    }
    node.add_child(q);
    node
}
pub fn make_session() -> IksNode {
    let mut node = IksNode::new_tag("iq");
    node.add_attribute("type", "set");
    let mut q = IksNode::new_tag("session");
    q.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-session");
    node.add_child(q);
    node
}
