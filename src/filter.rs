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

    /// Finds the value of an attribute by name on the underlying stanza node.
    pub fn find_attrib(&self, name: &str) -> Option<String> {
        self.node.find_attrib(name)
    }

    /// Finds the first child tag matching `name` on the underlying stanza node.
    pub fn find(&self, name: &str) -> Option<NodeRef> {
        self.node.find(name)
    }

    /// Finds a tag matching `name` and returns its first child CDATA content.
    pub fn find_cdata(&self, name: &str) -> Option<String> {
        self.node.find_cdata(name)
    }

    /// Gets the tag name of the underlying stanza node.
    pub fn name(&self) -> Option<String> {
        self.node.name()
    }
}

/// Trait converting handler return types into `FilterStatus`.
pub trait IntoFilterStatus {
    fn into_filter_status(self) -> FilterStatus;
}

impl IntoFilterStatus for FilterStatus {
    fn into_filter_status(self) -> FilterStatus {
        self
    }
}

impl IntoFilterStatus for bool {
    fn into_filter_status(self) -> FilterStatus {
        FilterStatus::Pass
    }
}

impl IntoFilterStatus for () {
    fn into_filter_status(self) -> FilterStatus {
        FilterStatus::Pass
    }
}

/// Unique identifier for a registered filter rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleId(pub usize);

/// Trait converting various types to `IksPacketType`.
pub trait IntoPacketType {
    fn into_packet_type(self) -> IksPacketType;
}

impl IntoPacketType for IksPacketType {
    fn into_packet_type(self) -> IksPacketType {
        self
    }
}

impl IntoPacketType for StanzaType {
    fn into_packet_type(self) -> IksPacketType {
        match self {
            StanzaType::Iq => IksPacketType::Iq,
            StanzaType::Message => IksPacketType::Message,
            StanzaType::Presence => IksPacketType::Presence,
            StanzaType::Other => IksPacketType::None,
        }
    }
}

/// Trait converting various types to `IksSubtype`.
pub trait IntoSubtype {
    fn into_subtype(self) -> IksSubtype;
}

impl IntoSubtype for IksSubtype {
    fn into_subtype(self) -> IksSubtype {
        self
    }
}

impl IntoSubtype for &str {
    fn into_subtype(self) -> IksSubtype {
        match self {
            "error" => IksSubtype::Error,
            "chat" => IksSubtype::Chat,
            "groupchat" => IksSubtype::Groupchat,
            "headline" => IksSubtype::Headline,
            "get" => IksSubtype::Get,
            "set" => IksSubtype::Set,
            "result" => IksSubtype::Result,
            "subscribe" => IksSubtype::Subscribe,
            "subscribed" => IksSubtype::Subscribed,
            "unsubscribe" => IksSubtype::Unsubscribe,
            "unsubscribed" => IksSubtype::Unsubscribed,
            "probe" => IksSubtype::Probe,
            "available" => IksSubtype::Available,
            "unavailable" => IksSubtype::Unavailable,
            _ => IksSubtype::None,
        }
    }
}

impl IntoSubtype for String {
    fn into_subtype(self) -> IksSubtype {
        self.as_str().into_subtype()
    }
}

/// Builder for constructing matching criteria for filter rules.
#[derive(Default, Clone, Debug)]
pub struct RuleBuilder {
    pub packet_type: Option<IksPacketType>,
    pub subtype: Option<IksSubtype>,
    pub id: Option<String>,
    pub from: Option<Jid>,
    pub from_partial: Option<Jid>,
    pub ns: Option<String>,
}

impl RuleBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_type<T: IntoPacketType>(mut self, packet_type: T) -> Self {
        self.packet_type = Some(packet_type.into_packet_type());
        self
    }

    pub fn with_subtype<S: IntoSubtype>(mut self, subtype: S) -> Self {
        self.subtype = Some(subtype.into_subtype());
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

    pub fn with_from_partial(mut self, from_partial: Jid) -> Self {
        self.from_partial = Some(from_partial);
        self
    }

    pub fn with_ns<S: Into<String>>(mut self, ns: S) -> Self {
        self.ns = Some(ns.into());
        self
    }
}

struct InternalRule {
    id: RuleId,
    criteria: RuleBuilder,
    handler: Box<dyn Fn(&IksPacket) -> FilterStatus + Send + Sync>,
}

/// A packet filter that dispatches stanzas to registered rule handlers adhering to C `filter.c:115-167`.
pub struct PacketFilter {
    rules: Vec<InternalRule>,
    next_id: usize,
}

impl PacketFilter {
    /// Creates a new empty `PacketFilter`.
    pub fn new() -> Self {
        PacketFilter {
            rules: Vec::new(),
            next_id: 1,
        }
    }

    /// Adds a new rule to the filter.
    pub fn add_rule<F, R>(&mut self, rule: RuleBuilder, handler: F) -> RuleId
    where
        F: Fn(&IksPacket) -> R + Send + Sync + 'static,
        R: IntoFilterStatus,
    {
        let id = RuleId(self.next_id);
        self.next_id += 1;
        self.rules.push(InternalRule {
            id,
            criteria: rule,
            handler: Box::new(move |pak| handler(pak).into_filter_status()),
        });
        id
    }

    /// Removes a rule by its ID. Returns true if the rule was found and removed.
    pub fn remove_rule(&mut self, id: RuleId) -> bool {
        let initial_len = self.rules.len();
        self.rules.retain(|r| r.id != id);
        self.rules.len() < initial_len
    }

    /// Filters a packet according to C `iks_filter_packet` scoring semantics.
    ///
    /// Weights:
    /// - ID = 16
    /// - FROM = 8
    /// - FROM_PARTIAL = 8
    /// - NS = 4
    /// - SUBTYPE = 2
    /// - TYPE = 1
    ///
    /// If any declared criteria fails to match, rule score is 0.
    /// Rules are evaluated in order of descending score. If a handler returns `FilterStatus::Eat`,
    /// dispatch ceases immediately and returns `FilterStatus::Eat`.
    pub fn filter_packet(&self, pak: &IksPacket) -> FilterStatus {
        let (status, _count) = self.filter_packet_internal(pak);
        status
    }

    /// Tests a stanza against all registered rules and dispatches to matching handlers.
    /// Returns the number of rules that matched and handled the stanza.
    pub fn dispatch(&self, stanza: &IksNode) -> usize {
        let pak = IksPacket::from_node(stanza);
        let (_status, count) = self.filter_packet_internal(&pak);
        count
    }

    /// Tests a `NodeRef` stanza against all registered rules and dispatches to matching handlers.
    /// Returns the number of rules that matched and handled the stanza.
    pub fn dispatch_ref(&self, stanza: &NodeRef) -> usize {
        let pak = IksPacket::from_node_ref(stanza);
        let (_status, count) = self.filter_packet_internal(&pak);
        count
    }

    fn filter_packet_internal(&self, pak: &IksPacket) -> (FilterStatus, usize) {
        if self.rules.is_empty() {
            return (FilterStatus::Pass, 0);
        }

        // Pass 1: compute score for each rule
        // C weights: ID=16, FROM=8, FROM_PARTIAL=8, NS=4, SUBTYPE=2, TYPE=1
        // All-or-nothing: if any declared criteria does not match, score is 0.
        // If no criteria declared, score is 0.
        let mut scores: Vec<i32> = Vec::with_capacity(self.rules.len());
        for rule in &self.rules {
            let mut score = 0;
            let mut fail = false;

            if let Some(ref r_type) = rule.criteria.packet_type {
                if *r_type == pak.packet_type {
                    score += 1;
                } else {
                    fail = true;
                }
            }
            if let Some(ref r_subtype) = rule.criteria.subtype {
                if *r_subtype == pak.subtype {
                    score += 2;
                } else {
                    fail = true;
                }
            }
            if let Some(ref r_id) = rule.criteria.id {
                if pak.id.as_deref() == Some(r_id.as_str()) {
                    score += 16;
                } else {
                    fail = true;
                }
            }
            if let Some(ref r_ns) = rule.criteria.ns {
                if pak.ns.as_deref() == Some(r_ns.as_str()) {
                    score += 4;
                } else {
                    fail = true;
                }
            }
            if let Some(ref r_from) = rule.criteria.from {
                if pak.from.as_ref().map(|j| j.full()).as_deref() == Some(r_from.full().as_str()) {
                    score += 8;
                } else {
                    fail = true;
                }
            }
            if let Some(ref r_from_partial) = rule.criteria.from_partial {
                if pak.from.as_ref().map(|j| j.bare()).as_deref() == Some(r_from_partial.bare().as_str()) {
                    score += 8;
                } else {
                    fail = true;
                }
            }

            if fail {
                score = 0;
            }
            scores.push(score);
        }

        // Pass 2: execute rules in order of descending score (> 0), FIFO for ties
        let mut executed_count = 0;
        loop {
            let mut max_score = 0;
            let mut max_idx = None;

            for (idx, &score) in scores.iter().enumerate() {
                if score > max_score {
                    max_score = score;
                    max_idx = Some(idx);
                }
            }

            match max_idx {
                Some(idx) => {
                    scores[idx] = 0;
                    executed_count += 1;
                    let ret = (self.rules[idx].handler)(pak);
                    if ret == FilterStatus::Eat {
                        return (FilterStatus::Eat, executed_count);
                    }
                }
                None => break,
            }
        }

        (FilterStatus::Pass, executed_count)
    }
}

impl Default for PacketFilter {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn test_filter_scoring_weights_order() {
        use std::sync::Mutex;

        let execution_order = Arc::new(Mutex::new(Vec::new()));
        let mut filter = PacketFilter::new();

        // Rule 1: Type only (score 1)
        let order_1 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new().with_type(IksPacketType::Iq),
            move |_| {
                order_1.lock().unwrap().push("type_only");
                FilterStatus::Pass
            },
        );

        // Rule 2: Subtype only (score 2)
        let order_2 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new().with_subtype(IksSubtype::Get),
            move |_| {
                order_2.lock().unwrap().push("subtype_only");
                FilterStatus::Pass
            },
        );

        // Rule 3: NS only (score 4)
        let order_3 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new().with_ns("jabber:iq:roster"),
            move |_| {
                order_3.lock().unwrap().push("ns_only");
                FilterStatus::Pass
            },
        );

        // Rule 4: From only (score 8)
        let order_4 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new().with_from(Jid::new("alice@example.com/res").unwrap()),
            move |_| {
                order_4.lock().unwrap().push("from_only");
                FilterStatus::Pass
            },
        );

        // Rule 5: ID only (score 16)
        let order_5 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new().with_id("iq_test_1"),
            move |_| {
                order_5.lock().unwrap().push("id_only");
                FilterStatus::Pass
            },
        );

        // Rule 6: From + NS (score 8 + 4 = 12)
        let order_6 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new()
                .with_from(Jid::new("alice@example.com/res").unwrap())
                .with_ns("jabber:iq:roster"),
            move |_| {
                order_6.lock().unwrap().push("from_and_ns");
                FilterStatus::Pass
            },
        );

        // Rule 7: ID + Subtype (score 16 + 2 = 18)
        let order_7 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new()
                .with_id("iq_test_1")
                .with_subtype(IksSubtype::Get),
            move |_| {
                order_7.lock().unwrap().push("id_and_subtype");
                FilterStatus::Pass
            },
        );

        // Construct matching IQ packet
        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("id", "iq_test_1");
        iq.add_attribute("type", "get");
        iq.add_attribute("from", "alice@example.com/res");
        let mut q = IksNode::new_tag("query");
        q.add_attribute("xmlns", "jabber:iq:roster");
        iq.add_child(q);

        let pak = IksPacket::from_node(&iq);
        let status = filter.filter_packet(&pak);
        assert_eq!(status, FilterStatus::Pass);

        let order = execution_order.lock().unwrap().clone();
        // Expected order:
        // Rule 7 (18) -> Rule 5 (16) -> Rule 6 (12) -> Rule 4 (8) -> Rule 3 (4) -> Rule 2 (2) -> Rule 1 (1)
        assert_eq!(
            order,
            vec![
                "id_and_subtype",
                "id_only",
                "from_and_ns",
                "from_only",
                "ns_only",
                "subtype_only",
                "type_only"
            ]
        );
    }

    #[test]
    fn test_filter_all_or_nothing_and_empty_rule() {
        let mut filter = PacketFilter::new();
        let ran_partial = Arc::new(AtomicUsize::new(0));
        let ran_empty = Arc::new(AtomicUsize::new(0));

        // Rule with ID and Subtype - packet will match ID but NOT Subtype
        let r_partial = ran_partial.clone();
        filter.add_rule(
            RuleBuilder::new()
                .with_id("iq_fail_test")
                .with_subtype(IksSubtype::Result),
            move |_| {
                r_partial.fetch_add(1, Ordering::SeqCst);
                FilterStatus::Pass
            },
        );

        // Empty rule with no criteria
        let r_empty = ran_empty.clone();
        filter.add_rule(RuleBuilder::new(), move |_| {
            r_empty.fetch_add(1, Ordering::SeqCst);
            FilterStatus::Pass
        });

        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("id", "iq_fail_test");
        iq.add_attribute("type", "get"); // doesn't match Result

        let pak = IksPacket::from_node(&iq);
        let status = filter.filter_packet(&pak);
        assert_eq!(status, FilterStatus::Pass);

        // Neither rule should have run!
        assert_eq!(ran_partial.load(Ordering::SeqCst), 0);
        assert_eq!(ran_empty.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_filter_eat_terminates_dispatch() {
        let mut filter = PacketFilter::new();
        let ran_high = Arc::new(AtomicUsize::new(0));
        let ran_low = Arc::new(AtomicUsize::new(0));

        let h = ran_high.clone();
        filter.add_rule(
            RuleBuilder::new().with_id("eat_test"), // score 16
            move |_| {
                h.fetch_add(1, Ordering::SeqCst);
                FilterStatus::Eat // Consumes packet!
            },
        );

        let l = ran_low.clone();
        filter.add_rule(
            RuleBuilder::new().with_type(IksPacketType::Iq), // score 1
            move |_| {
                l.fetch_add(1, Ordering::SeqCst);
                FilterStatus::Pass
            },
        );

        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("id", "eat_test");
        let pak = IksPacket::from_node(&iq);

        let status = filter.filter_packet(&pak);
        assert_eq!(status, FilterStatus::Eat);
        assert_eq!(ran_high.load(Ordering::SeqCst), 1);
        assert_eq!(ran_low.load(Ordering::SeqCst), 0); // Low score rule was not run
    }

    #[test]
    fn test_filter_from_partial_and_fifo_tie() {
        use std::sync::Mutex;

        let mut filter = PacketFilter::new();
        let execution_order = Arc::new(Mutex::new(Vec::new()));

        // Tie rules: both have score 4 (NS)
        let o1 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new().with_ns("test:ns"),
            move |_| {
                o1.lock().unwrap().push("first_ns");
                FilterStatus::Pass
            },
        );

        let o2 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new().with_ns("test:ns"),
            move |_| {
                o2.lock().unwrap().push("second_ns");
                FilterStatus::Pass
            },
        );

        // Partial from rule: score 8
        let o3 = execution_order.clone();
        filter.add_rule(
            RuleBuilder::new().with_from_partial(Jid::new("alice@example.com").unwrap()),
            move |_| {
                o3.lock().unwrap().push("bare_jid");
                FilterStatus::Pass
            },
        );

        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("from", "alice@example.com/mobile");
        let mut q = IksNode::new_tag("query");
        q.add_attribute("xmlns", "test:ns");
        iq.add_child(q);

        let pak = IksPacket::from_node(&iq);
        let status = filter.filter_packet(&pak);
        assert_eq!(status, FilterStatus::Pass);

        // bare_jid (score 8) runs first, then first_ns (score 4) before second_ns (FIFO)
        let order = execution_order.lock().unwrap().clone();
        assert_eq!(order, vec!["bare_jid", "first_ns", "second_ns"]);
    }

    #[test]
    fn test_filter_remove_rule() {
        let mut filter = PacketFilter::new();
        let ran_1 = Arc::new(AtomicUsize::new(0));
        let ran_2 = Arc::new(AtomicUsize::new(0));
        let ran_3 = Arc::new(AtomicUsize::new(0));

        let r1 = ran_1.clone();
        let id1 = filter.add_rule(RuleBuilder::new().with_id("test_id"), move |_| {
            r1.fetch_add(1, Ordering::SeqCst);
            FilterStatus::Pass
        });

        let r2 = ran_2.clone();
        let id2 = filter.add_rule(RuleBuilder::new().with_type(IksPacketType::Iq), move |_| {
            r2.fetch_add(1, Ordering::SeqCst);
            FilterStatus::Pass
        });

        let r3 = ran_3.clone();
        let id3 = filter.add_rule(RuleBuilder::new().with_subtype(IksSubtype::Get), move |_| {
            r3.fetch_add(1, Ordering::SeqCst);
            FilterStatus::Pass
        });

        // Remove rule 2
        assert!(filter.remove_rule(id2));
        // Removing again should return false
        assert!(!filter.remove_rule(id2));
        // Removing non-existent rule should return false
        assert!(!filter.remove_rule(RuleId(9999)));

        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("id", "test_id");
        iq.add_attribute("type", "get");
        let pak = IksPacket::from_node(&iq);

        let status = filter.filter_packet(&pak);
        assert_eq!(status, FilterStatus::Pass);

        // Rule 1 (ID) ran, Rule 3 (Subtype) ran, but Rule 2 (Type) was removed!
        assert_eq!(ran_1.load(Ordering::SeqCst), 1);
        assert_eq!(ran_2.load(Ordering::SeqCst), 0);
        assert_eq!(ran_3.load(Ordering::SeqCst), 1);

        // Remove remaining rules
        assert!(filter.remove_rule(id1));
        assert!(filter.remove_rule(id3));

        // Now none run
        filter.filter_packet(&pak);
        assert_eq!(ran_1.load(Ordering::SeqCst), 1);
        assert_eq!(ran_3.load(Ordering::SeqCst), 1);
    }
}


