/* 
            iksemel - XML parser for Rust
          Copyright (C) 2026 Süleyman Poyraz
  This code is free software; you can redistribute it and/or
  modify it under the terms of the Affero General Public License
  as published by the Free Software Foundation; either version 3
  of the License, or (at your option) any later version.
*/

use std::str::FromStr;
use crate::{IksError, IksNode, Jid, Result};

// ============================================================================
// XEP-0199: XMPP Ping
// ============================================================================

pub const XMLNS_PING: &str = "urn:xmpp:ping";

/// Builds an XEP-0199 Ping IQ stanza.
pub fn build_ping(id: &str, to: Option<&str>) -> IksNode {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "get");
    iq.add_attribute("id", id);
    if let Some(target) = to {
        iq.add_attribute("to", target);
    }

    let mut ping = IksNode::new_tag("ping");
    ping.add_attribute("xmlns", XMLNS_PING);
    iq.add_child(ping);

    iq
}

/// Checks whether an incoming stanza is an XEP-0199 Ping request.
pub fn is_ping(stanza: &IksNode) -> bool {
    if stanza.name() != Some("iq") || stanza.find_attrib("type") != Some("get") {
        return false;
    }
    if let Some(ping) = stanza.find("ping") {
        ping.borrow().find_attrib("xmlns") == Some(XMLNS_PING)
    } else {
        false
    }
}

/// Builds an XEP-0199 Pong response for a given incoming Ping IQ.
pub fn build_pong(ping_iq: &IksNode) -> Result<IksNode> {
    let id = ping_iq.find_attrib("id").ok_or(IksError::BadXml)?;
    let mut pong = IksNode::new_tag("iq");
    pong.add_attribute("type", "result");
    pong.add_attribute("id", id);

    if let Some(from) = ping_iq.find_attrib("from") {
        pong.add_attribute("to", from);
    }

    Ok(pong)
}

// ============================================================================
// XEP-0030: Service Discovery (Disco)
// ============================================================================

pub const XMLNS_DISCO_INFO: &str = "http://jabber.org/protocol/disco#info";
pub const XMLNS_DISCO_ITEMS: &str = "http://jabber.org/protocol/disco#items";

/// A service or entity identity reported in a disco#info query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiscoIdentity {
    pub category: String,
    pub type_name: String,
    pub name: Option<String>,
}

/// Information about features and identities supported by an XMPP entity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiscoInfo {
    pub node: Option<String>,
    pub identities: Vec<DiscoIdentity>,
    pub features: Vec<String>,
}

impl DiscoInfo {
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.iter().any(|f| f == feature)
    }
}

/// An individual item reported in a disco#items query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiscoItem {
    pub jid: Jid,
    pub name: Option<String>,
    pub node: Option<String>,
}

/// A list of items associated with an XMPP entity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DiscoItems {
    pub node: Option<String>,
    pub items: Vec<DiscoItem>,
}

/// Builds an XEP-0030 disco#info query stanza.
pub fn build_disco_info_query(id: &str, to: &str, node: Option<&str>) -> IksNode {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "get");
    iq.add_attribute("id", id);
    iq.add_attribute("to", to);

    let mut query = IksNode::new_tag("query");
    query.add_attribute("xmlns", XMLNS_DISCO_INFO);
    if let Some(n) = node {
        query.add_attribute("node", n);
    }
    iq.add_child(query);

    iq
}

/// Builds an XEP-0030 disco#items query stanza.
pub fn build_disco_items_query(id: &str, to: &str, node: Option<&str>) -> IksNode {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "get");
    iq.add_attribute("id", id);
    iq.add_attribute("to", to);

    let mut query = IksNode::new_tag("query");
    query.add_attribute("xmlns", XMLNS_DISCO_ITEMS);
    if let Some(n) = node {
        query.add_attribute("node", n);
    }
    iq.add_child(query);

    iq
}

/// Parses an incoming disco#info IQ result into a `DiscoInfo` struct.
pub fn parse_disco_info_response(iq: &IksNode) -> Result<DiscoInfo> {
    let query_rc = iq.find("query").ok_or(IksError::BadXml)?;
    let query = query_rc.borrow();

    if query.find_attrib("xmlns") != Some(XMLNS_DISCO_INFO) {
        return Err(IksError::BadXml);
    }

    let node = query.find_attrib("node").map(|s| s.to_string());
    let mut identities = Vec::new();
    let mut features = Vec::new();

    for child in query.children() {
        let c = child.borrow();
        match c.name() {
            Some("identity") => {
                let category = c.find_attrib("category").unwrap_or_default().to_string();
                let type_name = c.find_attrib("type").unwrap_or_default().to_string();
                let name = c.find_attrib("name").map(|s| s.to_string());
                identities.push(DiscoIdentity {
                    category,
                    type_name,
                    name,
                });
            }
            Some("feature") => {
                if let Some(var) = c.find_attrib("var") {
                    features.push(var.to_string());
                }
            }
            _ => {}
        }
    }

    Ok(DiscoInfo {
        node,
        identities,
        features,
    })
}

/// Parses an incoming disco#items IQ result into a `DiscoItems` struct.
pub fn parse_disco_items_response(iq: &IksNode) -> Result<DiscoItems> {
    let query_rc = iq.find("query").ok_or(IksError::BadXml)?;
    let query = query_rc.borrow();

    if query.find_attrib("xmlns") != Some(XMLNS_DISCO_ITEMS) {
        return Err(IksError::BadXml);
    }

    let node = query.find_attrib("node").map(|s| s.to_string());
    let mut items = Vec::new();

    for child in query.children() {
        let c = child.borrow();
        if c.name() == Some("item") {
            let jid_str = c.find_attrib("jid").ok_or(IksError::BadXml)?;
            let jid = Jid::new(jid_str)?;
            let name = c.find_attrib("name").map(|s| s.to_string());
            let item_node = c.find_attrib("node").map(|s| s.to_string());
            items.push(DiscoItem {
                jid,
                name,
                node: item_node,
            });
        }
    }

    Ok(DiscoItems { node, items })
}

// ============================================================================
// XEP-0085: Chat State Notifications
// ============================================================================

pub const XMLNS_CHAT_STATES: &str = "http://jabber.org/protocol/chatstates";

/// Chat state notifications according to XEP-0085.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChatState {
    Active,
    Composing,
    Paused,
    Inactive,
    Gone,
}

impl ChatState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatState::Active => "active",
            ChatState::Composing => "composing",
            ChatState::Paused => "paused",
            ChatState::Inactive => "inactive",
            ChatState::Gone => "gone",
        }
    }
}

impl FromStr for ChatState {
    type Err = IksError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "active" => Ok(ChatState::Active),
            "composing" => Ok(ChatState::Composing),
            "paused" => Ok(ChatState::Paused),
            "inactive" => Ok(ChatState::Inactive),
            "gone" => Ok(ChatState::Gone),
            _ => Err(IksError::BadXml),
        }
    }
}

/// Builds an XML element representing the chat state.
pub fn build_chat_state(state: ChatState) -> IksNode {
    let mut node = IksNode::new_tag(state.as_str());
    node.add_attribute("xmlns", XMLNS_CHAT_STATES);
    node
}

/// Attaches a chat state element to an existing message stanza.
pub fn attach_chat_state(message: &mut IksNode, state: ChatState) {
    message.add_child(build_chat_state(state));
}

/// Extracts a chat state from a message stanza if present.
pub fn extract_chat_state(message: &IksNode) -> Option<ChatState> {
    for child in message.children() {
        let c = child.borrow();
        if c.find_attrib("xmlns") == Some(XMLNS_CHAT_STATES) {
            if let Some(name) = c.name() {
                if let Ok(state) = ChatState::from_str(name) {
                    return Some(state);
                }
            }
        }
    }
    None
}
