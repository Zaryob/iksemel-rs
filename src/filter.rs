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

use crate::{IksNode, IksType, Jid, NodeRef};

/// The primary XMPP stanza categories (legacy compatibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StanzaType {
    Iq,
    Message,
    Presence,
    Other,
}

impl StanzaType {
    /// Determines stanza type from its tag name.
    pub fn from_tag(tag: &str) -> Self {
        match tag {
            "iq" => StanzaType::Iq,
            "message" => StanzaType::Message,
            "presence" => StanzaType::Presence,
            _ => StanzaType::Other,
        }
    }
}

/// XMPP packet type according to C `ikspaktype` (`include/iksemel.h:308-314`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IksPacketType {
    #[default]
    None = 0,
    Message = 1,
    Presence = 2,
    Iq = 3,
    Subscription = 4, // C IKS_PAK_S10N
}

/// XMPP stanza subtype according to C `iksubtype` (`include/iksemel.h:316-335`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IksSubtype {
    #[default]
    None = 0,
    Error = 1,
    Chat = 2,
    Groupchat = 3,
    Headline = 4,
    Get = 5,
    Set = 6,
    Result = 7,
    Subscribe = 8,
    Subscribed = 9,
    Unsubscribe = 10,
    Unsubscribed = 11,
    Probe = 12,
    Available = 13,
    Unavailable = 14,
}

/// XMPP presence show type according to C `ikshowtype` (`include/iksemel.h:337-344`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IksShowType {
    #[default]
    Unavailable = 0,
    Available = 1,
    Chat = 2,
    Away = 3,
    Xa = 4,
    Dnd = 5,
}

/// Return code from filter hooks according to C `iksfilterret` (`include/iksemel.h:378-381`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FilterStatus {
    #[default]
    Pass = 0,
    Eat = 1,
}

/// Classified packet metadata representing C `ikspak` (`include/iksemel.h:346-355`).
#[derive(Debug, Clone)]
pub struct IksPacket {
    pub packet_type: IksPacketType,
    pub subtype: IksSubtype,
    pub show: IksShowType,
    pub id: Option<String>,
    pub from: Option<Jid>,
    pub ns: Option<String>,
    pub query: Option<NodeRef>,
    pub node: NodeRef,
}

impl IksPacket {
    /// Classifies an `IksNode` as an XMPP packet adhering to C `iks_packet` (`jabber.c:68-159`).
    pub fn from_node(node: &IksNode) -> Self {
        let node_ref = NodeRef::from(node.clone());
        Self::from_node_ref(&node_ref)
    }

    /// Classifies a `NodeRef` as an XMPP packet adhering to C `iks_packet` (`jabber.c:68-159`).
    pub fn from_node_ref(node: &NodeRef) -> Self {
        let borrowed = node.borrow();
        let name = borrowed.name().unwrap_or("");
        let id = borrowed.find_attrib("id").map(|s| s.to_string());
        let from = borrowed.find_attrib("from").and_then(|s| Jid::new(s).ok());
        let type_attr = borrowed.find_attrib("type");

        let mut packet_type = IksPacketType::None;
        let mut subtype = IksSubtype::None;
        let mut show = IksShowType::Unavailable;
        let mut ns = None;
        let mut query = None;

        match name {
            "message" => {
                packet_type = IksPacketType::Message;
                subtype = match type_attr {
                    Some("chat") => IksSubtype::Chat,
                    Some("groupchat") => IksSubtype::Groupchat,
                    Some("headline") => IksSubtype::Headline,
                    Some("error") => IksSubtype::Error,
                    _ => IksSubtype::None,
                };
            }
            "presence" => {
                if let Some(t) = type_attr {
                    match t {
                        "unavailable" => {
                            packet_type = IksPacketType::Presence;
                            subtype = IksSubtype::Unavailable;
                            show = IksShowType::Unavailable;
                        }
                        "probe" => {
                            packet_type = IksPacketType::Presence;
                            subtype = IksSubtype::Probe;
                            show = IksShowType::Unavailable;
                        }
                        "subscribe" => {
                            packet_type = IksPacketType::Subscription;
                            subtype = IksSubtype::Subscribe;
                        }
                        "subscribed" => {
                            packet_type = IksPacketType::Subscription;
                            subtype = IksSubtype::Subscribed;
                        }
                        "unsubscribe" => {
                            packet_type = IksPacketType::Subscription;
                            subtype = IksSubtype::Unsubscribe;
                        }
                        "unsubscribed" => {
                            packet_type = IksPacketType::Subscription;
                            subtype = IksSubtype::Unsubscribed;
                        }
                        "error" => {
                            packet_type = IksPacketType::Subscription;
                            subtype = IksSubtype::Error;
                        }
                        _ => {
                            packet_type = IksPacketType::Subscription;
                            subtype = IksSubtype::None;
                        }
                    }
                } else {
                    packet_type = IksPacketType::Presence;
                    subtype = IksSubtype::Available;
                    show = match borrowed.find_cdata("show").as_deref() {
                        Some("chat") => IksShowType::Chat,
                        Some("away") => IksShowType::Away,
                        Some("xa") => IksShowType::Xa,
                        Some("dnd") => IksShowType::Dnd,
                        _ => IksShowType::Available,
                    };
                }
            }
            "iq" => {
                packet_type = IksPacketType::Iq;
                subtype = match type_attr {
                    Some("get") => IksSubtype::Get,
                    Some("set") => IksSubtype::Set,
                    Some("result") => IksSubtype::Result,
                    Some("error") => IksSubtype::Error,
                    _ => IksSubtype::None,
                };
                for child in borrowed.children() {
                    let c = child.borrow();
                    if c.node_type() == IksType::Tag {
                        if let Some(xmlns) = c.find_attrib("xmlns") {
                            ns = Some(xmlns.to_string());
                            query = Some(NodeRef(child.clone()));
                            break;
                        }
                    }
                }
            }
            _ => {}
        }

        drop(borrowed);

        IksPacket {
            packet_type,
            subtype,
            show,
            id,
            from,
            ns,
            query,
            node: node.clone(),
        }
    }
}

/// A filter rule matching incoming XMPP stanzas.
pub struct FilterRule {
    pub stanza_type: Option<StanzaType>,
    pub id: Option<String>,
    pub from: Option<Jid>,
    pub ns: Option<String>,
    pub subtype: Option<String>,
    handler: Box<dyn Fn(&IksNode) -> bool + Send + Sync>,
}

/// A packet filter that dispatches stanzas to registered rule handlers.
pub struct PacketFilter {
    rules: Vec<FilterRule>,
}

impl PacketFilter {
    /// Creates a new empty `PacketFilter`.
    pub fn new() -> Self {
        PacketFilter { rules: Vec::new() }
    }

    /// Adds a new rule to the filter.
    pub fn add_rule<F>(&mut self, rule: RuleBuilder, handler: F)
    where
        F: Fn(&IksNode) -> bool + Send + Sync + 'static,
    {
        self.rules.push(FilterRule {
            stanza_type: rule.stanza_type,
            id: rule.id,
            from: rule.from,
            ns: rule.ns,
            subtype: rule.subtype,
            handler: Box::new(handler),
        });
    }

    /// Tests a stanza against all registered rules and dispatches to matching handlers.
    /// Returns the number of rules that matched and handled the stanza.
    pub fn dispatch(&self, stanza: &IksNode) -> usize {
        let tag_name = stanza.name().unwrap_or("");
        let stanza_type = StanzaType::from_tag(tag_name);
        let id = stanza.find_attrib("id");
        let from_str = stanza.find_attrib("from");
        let from_jid = from_str.and_then(|s| Jid::new(s).ok());
        let subtype = stanza.find_attrib("type");

        let mut handled = 0;
        for rule in &self.rules {
            if let Some(ref st) = rule.stanza_type {
                if *st != stanza_type {
                    continue;
                }
            }
            if let Some(ref rule_id) = rule.id {
                if id != Some(rule_id.as_str()) {
                    continue;
                }
            }
            if let Some(ref rule_from) = rule.from {
                match &from_jid {
                    Some(fj) if fj.matches(rule_from) => {}
                    _ => continue,
                }
            }
            if let Some(ref rule_sub) = rule.subtype {
                if subtype != Some(rule_sub.as_str()) {
                    continue;
                }
            }
            if let Some(ref rule_ns) = rule.ns {
                // Check if any child has xmlns equal to rule_ns or if stanza has it
                let has_ns = stanza.find_attrib("xmlns") == Some(rule_ns.as_str())
                    || stanza
                        .children()
                        .iter()
                        .any(|c| c.borrow().find_attrib("xmlns") == Some(rule_ns.as_str()));
                if !has_ns {
                    continue;
                }
            }

            if (rule.handler)(stanza) {
                handled += 1;
            }
        }
        handled
    }
}

impl Default for PacketFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for constructing matching criteria for filter rules.
#[derive(Default)]
pub struct RuleBuilder {
    stanza_type: Option<StanzaType>,
    id: Option<String>,
    from: Option<Jid>,
    ns: Option<String>,
    subtype: Option<String>,
}

impl RuleBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type(mut self, stanza_type: StanzaType) -> Self {
        self.stanza_type = Some(stanza_type);
        self
    }

    pub fn with_id<S: Into<String>>(mut self, id: S) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_from(mut self, from: Jid) -> Self {
        self.from = Some(from);
        self
    }

    pub fn with_ns<S: Into<String>>(mut self, ns: S) -> Self {
        self.ns = Some(ns.into());
        self
    }

    pub fn with_subtype<S: Into<String>>(mut self, subtype: S) -> Self {
        self.subtype = Some(subtype.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_filter_dispatch_id_and_type() {
        let mut filter = PacketFilter::new();
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_clone = counter.clone();
        filter.add_rule(
            RuleBuilder::new()
                .with_type(StanzaType::Iq)
                .with_id("roster_1")
                .with_subtype("result"),
            move |_stanza| {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                true
            },
        );

        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("id", "roster_1");
        iq.add_attribute("type", "result");

        assert_eq!(filter.dispatch(&iq), 1);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Different ID should not trigger
        let mut iq2 = IksNode::new_tag("iq");
        iq2.add_attribute("id", "other_id");
        iq2.add_attribute("type", "result");
        assert_eq!(filter.dispatch(&iq2), 0);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_filter_dispatch_ns() {
        let mut filter = PacketFilter::new();
        let roster_matched = Arc::new(AtomicUsize::new(0));

        let roster_clone = roster_matched.clone();
        filter.add_rule(
            RuleBuilder::new().with_ns("jabber:iq:roster"),
            move |_stanza| {
                roster_clone.fetch_add(1, Ordering::SeqCst);
                true
            },
        );

        let mut iq = IksNode::new_tag("iq");
        let mut query = IksNode::new_tag("query");
        query.add_attribute("xmlns", "jabber:iq:roster");
        iq.add_child(query);

        assert_eq!(filter.dispatch(&iq), 1);
        assert_eq!(roster_matched.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_ikspak_classification_message() {
        let mut msg = IksNode::new_tag("message");
        msg.add_attribute("type", "chat");
        msg.add_attribute("id", "msg-1");
        msg.add_attribute("from", "alice@example.com/phone");

        let pak = IksPacket::from_node(&msg);
        assert_eq!(pak.packet_type, IksPacketType::Message);
        assert_eq!(pak.subtype, IksSubtype::Chat);
        assert_eq!(pak.show, IksShowType::Unavailable);
        assert_eq!(pak.id.as_deref(), Some("msg-1"));
        assert_eq!(pak.from.as_ref().map(|j| j.full()), Some("alice@example.com/phone".to_string()));
        assert_eq!(pak.ns, None);
        assert_eq!(pak.query, None);

        // Message types: groupchat, headline, error, untyped
        for (st_str, expected) in [
            ("groupchat", IksSubtype::Groupchat),
            ("headline", IksSubtype::Headline),
            ("error", IksSubtype::Error),
            ("unknown", IksSubtype::None),
        ] {
            let mut m = IksNode::new_tag("message");
            m.add_attribute("type", st_str);
            let p = IksPacket::from_node(&m);
            assert_eq!(p.packet_type, IksPacketType::Message);
            assert_eq!(p.subtype, expected);
        }

        let m_plain = IksNode::new_tag("message");
        let p_plain = IksPacket::from_node(&m_plain);
        assert_eq!(p_plain.packet_type, IksPacketType::Message);
        assert_eq!(p_plain.subtype, IksSubtype::None);
    }

    #[test]
    fn test_ikspak_classification_presence_and_subscription() {
        // Plain available presence
        let pres = IksNode::new_tag("presence");
        let pak = IksPacket::from_node(&pres);
        assert_eq!(pak.packet_type, IksPacketType::Presence);
        assert_eq!(pak.subtype, IksSubtype::Available);
        assert_eq!(pak.show, IksShowType::Available);

        // Presence with <show>
        for (show_str, expected_show) in [
            ("chat", IksShowType::Chat),
            ("away", IksShowType::Away),
            ("xa", IksShowType::Xa),
            ("dnd", IksShowType::Dnd),
            ("custom", IksShowType::Available), // Unknown show defaults to Available in C
        ] {
            let mut p_node = IksNode::new_tag("presence");
            let mut show_node = IksNode::new_tag("show");
            show_node.add_child(IksNode::new_cdata(show_str));
            p_node.add_child(show_node);

            let p = IksPacket::from_node(&p_node);
            assert_eq!(p.packet_type, IksPacketType::Presence);
            assert_eq!(p.subtype, IksSubtype::Available);
            assert_eq!(p.show, expected_show);
        }

        // Unavailable presence
        let mut unavail = IksNode::new_tag("presence");
        unavail.add_attribute("type", "unavailable");
        let p_unavail = IksPacket::from_node(&unavail);
        assert_eq!(p_unavail.packet_type, IksPacketType::Presence);
        assert_eq!(p_unavail.subtype, IksSubtype::Unavailable);
        assert_eq!(p_unavail.show, IksShowType::Unavailable);

        // Probe presence
        let mut probe = IksNode::new_tag("presence");
        probe.add_attribute("type", "probe");
        let p_probe = IksPacket::from_node(&probe);
        assert_eq!(p_probe.packet_type, IksPacketType::Presence);
        assert_eq!(p_probe.subtype, IksSubtype::Probe);
        assert_eq!(p_probe.show, IksShowType::Unavailable);

        // Subscriptions (IKS_PAK_S10N in C)
        for (s10n_str, expected_sub) in [
            ("subscribe", IksSubtype::Subscribe),
            ("subscribed", IksSubtype::Subscribed),
            ("unsubscribe", IksSubtype::Unsubscribe),
            ("unsubscribed", IksSubtype::Unsubscribed),
            ("error", IksSubtype::Error),
            ("other", IksSubtype::None),
        ] {
            let mut s = IksNode::new_tag("presence");
            s.add_attribute("type", s10n_str);
            let p = IksPacket::from_node(&s);
            assert_eq!(p.packet_type, IksPacketType::Subscription);
            assert_eq!(p.subtype, expected_sub);
        }
    }

    #[test]
    fn test_ikspak_classification_iq_and_ns_rule() {
        // IQ subtypes: get, set, result, error
        for (st_str, expected) in [
            ("get", IksSubtype::Get),
            ("set", IksSubtype::Set),
            ("result", IksSubtype::Result),
            ("error", IksSubtype::Error),
            ("other", IksSubtype::None),
        ] {
            let mut iq = IksNode::new_tag("iq");
            iq.add_attribute("type", st_str);
            let p = IksPacket::from_node(&iq);
            assert_eq!(p.packet_type, IksPacketType::Iq);
            assert_eq!(p.subtype, expected);
        }

        // C jabber.c:146-156: ns is populated ONLY for Iq and ONLY from first tag child's xmlns
        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("type", "get");
        let mut query = IksNode::new_tag("query");
        query.add_attribute("xmlns", "jabber:iq:roster");
        iq.add_child(query);

        let pak = IksPacket::from_node(&iq);
        assert_eq!(pak.packet_type, IksPacketType::Iq);
        assert_eq!(pak.subtype, IksSubtype::Get);
        assert_eq!(pak.ns.as_deref(), Some("jabber:iq:roster"));
        assert!(pak.query.is_some());

        // Message or Presence with xmlns children MUST NOT have pak.ns populated
        let mut msg = IksNode::new_tag("message");
        let mut body = IksNode::new_tag("body");
        body.add_attribute("xmlns", "jabber:client");
        msg.add_child(body);
        let p_msg = IksPacket::from_node(&msg);
        assert_eq!(p_msg.ns, None);

        let mut pres = IksNode::new_tag("presence");
        let mut x = IksNode::new_tag("x");
        x.add_attribute("xmlns", "vcard-temp:x:update");
        pres.add_child(x);
        let p_pres = IksPacket::from_node(&pres);
        assert_eq!(p_pres.ns, None);
    }
}

