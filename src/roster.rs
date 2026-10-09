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

use std::fmt;
use std::fs;
use std::str::FromStr;

use crate::{Connection, DomParser, IksError, IksNode, IksType, Jid, Result};

/// Subscription state of a roster contact (RFC 6121 Section 2.1.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SubscriptionType {
    None,
    To,
    From,
    Both,
    Remove,
}

impl SubscriptionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionType::None => "none",
            SubscriptionType::To => "to",
            SubscriptionType::From => "from",
            SubscriptionType::Both => "both",
            SubscriptionType::Remove => "remove",
        }
    }
}

impl fmt::Display for SubscriptionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for SubscriptionType {
    type Err = IksError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "none" => Ok(SubscriptionType::None),
            "to" => Ok(SubscriptionType::To),
            "from" => Ok(SubscriptionType::From),
            "both" => Ok(SubscriptionType::Both),
            "remove" => Ok(SubscriptionType::Remove),
            _ => Ok(SubscriptionType::None),
        }
    }
}

/// An individual contact item in an XMPP roster.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RosterItem {
    pub jid: Jid,
    pub name: Option<String>,
    pub subscription: SubscriptionType,
    pub ask: bool,
    pub groups: Vec<String>,
}

impl RosterItem {
    pub fn new(jid: Jid) -> Self {
        RosterItem {
            jid,
            name: None,
            subscription: SubscriptionType::None,
            ask: false,
            groups: Vec::new(),
        }
    }

    /// Parses a `<item>` XML element into a `RosterItem`.
    pub fn from_node(node: &IksNode) -> Result<Self> {
        let jid_str = node.find_attrib("jid").ok_or(IksError::BadXml)?;
        let jid = Jid::new(jid_str)?;
        let name = node.find_attrib("name").map(|s| s.to_string());
        let subscription = node
            .find_attrib("subscription")
            .and_then(|s| SubscriptionType::from_str(s).ok())
            .unwrap_or(SubscriptionType::None);
        let ask = node.find_attrib("ask") == Some("subscribe");

        let mut groups = Vec::new();
        for child in node.children() {
            let c = child.borrow();
            if c.name() == Some("group") {
                if let Some(cdata) = c
                    .children()
                    .iter()
                    .find(|ch| ch.borrow().node_type() == IksType::CData)
                {
                    if let Some(content) = cdata.borrow().content() {
                        groups.push(content.trim().to_string());
                    }
                }
            }
        }

        Ok(RosterItem {
            jid,
            name,
            subscription,
            ask,
            groups,
        })
    }

    /// Converts this item into an `<item>` XML element.
    pub fn to_node(&self) -> IksNode {
        let mut item = IksNode::new_tag("item");
        item.add_attribute("jid", self.jid.bare());
        if let Some(ref name) = self.name {
            item.add_attribute("name", name);
        }
        item.add_attribute("subscription", self.subscription.as_str());
        if self.ask {
            item.add_attribute("ask", "subscribe");
        }

        for group in &self.groups {
            let mut g = IksNode::new_tag("group");
            g.insert_cdata(group);
            item.add_child(g);
        }

        item
    }
}

/// Represents an XMPP Roster collection.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Roster {
    pub items: Vec<RosterItem>,
}

impl Roster {
    pub fn new() -> Self {
        Roster { items: Vec::new() }
    }

    /// Finds a roster item by bare JID.
    pub fn get(&self, jid: &Jid) -> Option<&RosterItem> {
        let bare = jid.bare();
        self.items.iter().find(|i| i.jid.bare() == bare)
    }

    /// Finds a mutable roster item by bare JID.
    pub fn get_mut(&mut self, jid: &Jid) -> Option<&mut RosterItem> {
        let bare = jid.bare();
        self.items.iter_mut().find(|i| i.jid.bare() == bare)
    }

    /// Adds or updates an item in the roster.
    pub fn upsert(&mut self, item: RosterItem) {
        let bare = item.jid.bare();
        if let Some(pos) = self.items.iter().position(|i| i.jid.bare() == bare) {
            self.items[pos] = item;
        } else {
            self.items.push(item);
        }
    }

    /// Removes an item from the roster.
    pub fn remove(&mut self, jid: &Jid) -> Option<RosterItem> {
        let bare = jid.bare();
        if let Some(pos) = self.items.iter().position(|i| i.jid.bare() == bare) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    /// Parses a roster from an XML DOM tree (`<iq>` or `<query xmlns='jabber:iq:roster'>`).
    pub fn from_node(node: &IksNode) -> Result<Self> {
        let query_node = if node.name() == Some("query") {
            Some(node)
        } else {
            None
        };

        let mut items = Vec::new();
        if let Some(q) = query_node {
            for child in q.children() {
                if child.borrow().name() == Some("item") {
                    items.push(RosterItem::from_node(&child.borrow())?);
                }
            }
        } else if let Some(query) = node.find("query") {
            for child in query.borrow().children() {
                if child.borrow().name() == Some("item") {
                    items.push(RosterItem::from_node(&child.borrow())?);
                }
            }
        }

        Ok(Roster { items })
    }

    /// Parses a roster from a `NodeRef`.
    pub fn from_node_ref(node: &crate::NodeRef) -> Result<Self> {
        Self::from_node(&node.borrow())
    }

    /// Serializes the roster into a `<query xmlns='jabber:iq:roster'>` node.
    pub fn to_node(&self) -> IksNode {
        let mut query = IksNode::new_tag("query");
        query.add_attribute("xmlns", "jabber:iq:roster");

        for item in &self.items {
            query.add_child(item.to_node());
        }

        query
    }

    /// Saves the roster to a file as formatted XML.
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let xml = self.to_node().to_string();
        fs::write(path, xml)?;
        Ok(())
    }

    /// Loads the roster from an XML file.
    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let doc_rc = DomParser::parse_str(&content)?;
        Self::from_node_ref(&doc_rc)
    }
}

/// Queries the server for the user's roster (RFC 6121 Section 2.1.3).
pub fn fetch_roster(conn: &mut Connection, iq_id: &str) -> Result<Roster> {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "get");
    iq.add_attribute("id", iq_id);

    let mut query = IksNode::new_tag("query");
    query.add_attribute("xmlns", "jabber:iq:roster");
    iq.add_child(query);

    conn.send_stanza(&iq)?;

    let resp = conn.recv_iq_response(iq_id)?;
    if resp.find_attrib("type").as_deref() != Some("result") {
        return Err(IksError::NetRwErr);
    }

    let roster = Roster::from_node(&resp.borrow());
    roster
}

/// Pushes (sets) roster items to the server.
pub fn sync_roster(conn: &mut Connection, roster: &Roster) -> Result<()> {
    for (i, item) in roster.items.iter().enumerate() {
        let iq_id = format!("roster_sync_{}", i);
        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("type", "set");
        iq.add_attribute("id", &iq_id);

        let mut query = IksNode::new_tag("query");
        query.add_attribute("xmlns", "jabber:iq:roster");
        query.add_child(item.to_node());
        iq.add_child(query);

        conn.send_stanza(&iq)?;
        let resp = conn.recv_iq_response(&iq_id)?;
        if resp.find_attrib("type").as_deref() != Some("result") {
            return Err(IksError::NetRwErr);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roster_serialization_and_deserialization() {
        let mut roster = Roster::new();

        let mut item1 = RosterItem::new(Jid::new("alice@example.com").unwrap());
        item1.name = Some("Alice".to_string());
        item1.subscription = SubscriptionType::Both;
        item1.groups.push("Friends".to_string());
        item1.groups.push("Work".to_string());
        roster.upsert(item1);

        let mut item2 = RosterItem::new(Jid::new("bob@example.com").unwrap());
        item2.name = Some("Bob".to_string());
        item2.subscription = SubscriptionType::To;
        item2.ask = true;
        roster.upsert(item2);

        let query_node = roster.to_node();
        assert_eq!(query_node.name(), Some("query"));
        assert_eq!(query_node.find_attrib("xmlns"), Some("jabber:iq:roster"));

        let parsed = Roster::from_node(&query_node).unwrap();
        assert_eq!(parsed.items.len(), 2);

        let alice = parsed.get(&Jid::new("alice@example.com").unwrap()).unwrap();
        assert_eq!(alice.name.as_deref(), Some("Alice"));
        assert_eq!(alice.subscription, SubscriptionType::Both);
        assert_eq!(alice.groups, vec!["Friends", "Work"]);
        assert!(!alice.ask);

        let bob = parsed.get(&Jid::new("bob@example.com").unwrap()).unwrap();
        assert_eq!(bob.name.as_deref(), Some("Bob"));
        assert_eq!(bob.subscription, SubscriptionType::To);
        assert!(bob.ask);
    }

    #[test]
    fn test_roster_upsert_and_remove() {
        let mut roster = Roster::new();
        let jid = Jid::new("carol@example.com").unwrap();

        let mut item = RosterItem::new(jid.clone());
        item.name = Some("Carol".to_string());
        roster.upsert(item);
        assert_eq!(roster.items.len(), 1);

        let mut updated = RosterItem::new(jid.clone());
        updated.name = Some("Carol Smith".to_string());
        roster.upsert(updated);
        assert_eq!(roster.items.len(), 1);
        assert_eq!(
            roster.get(&jid).unwrap().name.as_deref(),
            Some("Carol Smith")
        );

        let removed = roster.remove(&jid);
        assert!(removed.is_some());
        assert_eq!(roster.items.len(), 0);
    }

    #[test]
    fn test_fetch_roster_interleaved_stanzas() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];

            // 1. Initial client stream header
            let _ = socket.read(&mut buf).unwrap();
            let server_hdr = "<?xml version='1.0'?><stream:stream xmlns:stream='http://etherx.jabber.org/streams' xmlns='jabber:client' from='example.com' version='1.0'>";
            socket.write_all(server_hdr.as_bytes()).unwrap();

            // 2. Read roster get IQ
            let n = socket.read(&mut buf).unwrap();
            let req = std::str::from_utf8(&buf[..n]).unwrap();
            assert!(req.contains("id=\"roster_req_1\""));

            // 3. Send interleaved presence and message BEFORE roster IQ response!
            let response = concat!(
                "<presence from='friend@example.com'><show>chat</show></presence>",
                "<message from='boss@example.com'><body>urgent</body></message>",
                "<iq id='roster_req_1' type='result'>",
                "  <query xmlns='jabber:iq:roster'>",
                "    <item jid='alice@example.com' name='Alice' subscription='both'/>",
                "  </query>",
                "</iq>"
            );
            socket.write_all(response.as_bytes()).unwrap();
        });

        let mut conn = Connection::connect(
            "127.0.0.1",
            port,
            "example.com",
            Some(Duration::from_secs(5)),
        )
        .unwrap();
        conn.start_stream().unwrap();

        // fetch_roster should correlate with roster_req_1 despite interleaved presence and message
        let roster = fetch_roster(&mut conn, "roster_req_1").expect("fetch_roster succeeds");
        assert_eq!(roster.items.len(), 1);
        assert_eq!(roster.items[0].jid.bare(), "alice@example.com");

        // The interleaved presence and message MUST be preserved in connection pending queue
        let st1 = conn.recv_stanza().expect("presence received");
        assert_eq!(st1.name().as_deref(), Some("presence"));
        assert_eq!(
            st1.find_attrib("from").as_deref(),
            Some("friend@example.com")
        );

        let st2 = conn.recv_stanza().expect("message received");
        assert_eq!(st2.name().as_deref(), Some("message"));
        assert_eq!(st2.find_attrib("from").as_deref(), Some("boss@example.com"));

        handle.join().unwrap();
    }
}
