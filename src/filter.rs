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
 Affero General Public License for more details.
*/

use crate::{IksNode, Jid};

/// The primary XMPP stanza categories.
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
}
