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

#![forbid(unsafe_code)]

pub mod async_net;
mod constants;
pub mod crypto;
mod dom;
mod escape;
pub mod filter;
mod helper;
mod utf8;
pub mod jid;
pub mod net;
mod parser;
pub mod roster;
pub mod sasl;
pub mod stream;
mod utility;
pub mod writer;
pub mod xep;

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};
use thiserror::Error;

pub use async_net::{
    authenticate_plain_async, authenticate_scram_sha1_async, authenticate_scram_sha256_async,
    bind_resource_async, AsyncConnection, AsyncConnectionStream, AsyncReceiver, AsyncSender,
};
pub use constants::{memory, xml};
pub use crypto::{
    base64_decode, base64_encode, hmac_sha1, hmac_sha256, pbkdf2_hmac_sha1, pbkdf2_hmac_sha256,
    sha1_hash, sha1_hex, sha256_hash, sha256_hex,
};
pub use dom::DomParser;
pub use filter::{
    FilterStatus, IksPacket, IksPacketType, IksShowType, IksSubtype, PacketFilter, RuleBuilder,
    StanzaType,
};
pub use helper::{align_size, calculate_chunk_growth, escape_size, unescape_size};
pub use jid::Jid;
pub use net::{Connection, ConnectionStream};
pub use parser::{is_xml_name_char, is_xml_whitespace, Parser, ParserLimits, SaxHandler};
pub use roster::{fetch_roster, sync_roster, Roster, RosterItem, SubscriptionType};
pub use sasl::{
    authenticate_non_sasl, authenticate_plain, authenticate_scram_sha1, authenticate_scram_sha256,
    bind_resource, establish_session, parse_features_mechanisms, SaslMechanism, ScramClient,
    ScramHash,
};
pub use stream::{StreamEvent, StreamParser};
pub use utility::{
    escape, escape_cow, str_casecmp, str_cat, str_dup, str_len, unescape, unescape_cow,
};
pub use writer::XmlWriter;
pub use xep::{
    attach_chat_state, build_carbons_disable, build_carbons_enable, build_chat_state,
    build_disco_info_query, build_disco_items_query, build_muc_join, build_muc_leave, build_ping,
    build_pong, build_pong_ref, build_pubsub_publish, build_pubsub_subscribe, build_pubsub_unsubscribe,
    build_sm_ack, build_sm_enable, build_sm_request_ack, build_sm_resume, extract_carbon,
    extract_chat_state, extract_mam_result, extract_muc_status_codes, extract_pubsub_items,
    is_muc_presence, is_ping, is_ping_ref, is_sm_stanza, mark_carbon_private, parse_disco_info_response,
    parse_disco_items_response, parse_mam_fin, parse_sm_ack, parse_sm_enabled, parse_sm_enabled_ref, parse_sm_resumed,
    wrap_carbon_received, wrap_carbon_sent, CarbonMessage, ChatState, DataForm, DataFormType,
    DiscoIdentity, DiscoInfo, DiscoItem, DiscoItems, FieldOption, FieldType, FormField, MamFin,
    MamQuery, MamResult, PubSubItem, SmEnabled, SmResumed, StreamManagementState, XMLNS_CARBONS,
    XMLNS_CHAT_STATES, XMLNS_DATA_FORMS, XMLNS_DELAY, XMLNS_DISCO_INFO, XMLNS_DISCO_ITEMS,
    XMLNS_FORWARD, XMLNS_MAM, XMLNS_MUC, XMLNS_MUC_USER, XMLNS_PING, XMLNS_PUBSUB,
    XMLNS_PUBSUB_EVENT, XMLNS_RSM, XMLNS_STREAM_MANAGEMENT,
};

/// Represents the type of an XML node in the DOM tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IksType {
    /// No specific type
    None,
    /// XML element tag
    Tag,
    /// XML attribute
    Attribute,
    /// Character data (text content)
    CData,
}

/// Represents the type of an XML tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    /// Opening tag (e.g., `<tag>`)
    Open,
    /// Closing tag (e.g., `</tag>`)
    Close,
    /// Self-closing tag (e.g., `<tag/>`)
    Single,
}

/// Error types that can occur during XML parsing and processing.
#[derive(Error, Debug)]
pub enum IksError {
    /// Memory allocation failed
    #[error("Out of memory")]
    NoMem,
    /// Invalid XML syntax
    #[error("Invalid XML")]
    BadXml,
    /// Invalid JID syntax
    #[error("Invalid JID")]
    BadJid,
    /// Invalid Base64 data
    #[error("Invalid Base64")]
    BadBase64,
    /// Error returned from a hook function
    #[error("Hook returned error")]
    Hook,
    /// Network DNS resolution failed
    #[error("Network DNS error")]
    NetNoDns,
    /// Network socket creation failed
    #[error("Network socket error")]
    NetNoSock,
    /// Network connection failed
    #[error("Network connection error")]
    NetNoConn,
    /// Network read/write error
    #[error("Network read/write error")]
    NetRwErr,
    /// Network operation not supported
    #[error("Network operation not supported")]
    NetNotSupp,
    /// TLS operation failed
    #[error("TLS operation failed")]
    NetTlsFail,
    /// Network connection dropped
    #[error("Network connection dropped")]
    NetDropped,
    /// Unknown network error
    #[error("Unknown network error")]
    NetUnknown,
    /// File not found
    #[error("File not found")]
    FileNoFile,
    /// File access denied
    #[error("File access denied")]
    FileNoAccess,
    /// File read/write error
    #[error("File read/write error")]
    FileRwErr,
    /// Maximum XML element nesting depth exceeded
    #[error("Maximum nesting depth exceeded")]
    MaxDepthExceeded,
    /// Maximum entity expansions exceeded
    #[error("Maximum entity expansions exceeded")]
    MaxEntityExpansionsExceeded,
    /// Maximum attribute count per element exceeded
    #[error("Maximum attributes exceeded")]
    MaxAttributesExceeded,
    /// Maximum token size exceeded
    #[error("Maximum token size exceeded")]
    MaxTokenSizeExceeded,
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for iksemel operations
pub type Result<T> = std::result::Result<T, IksError>;

/// Represents a node in the XML DOM tree.
///
/// This structure provides a complete representation of an XML document,
/// including elements, attributes, and text content. It supports:
/// - Parent-child relationships
/// - Sibling navigation
/// - Attribute management
/// - Text content
///
/// # Examples
///
/// ```
/// use iksemel::{IksNode, IksType};
///
/// // Create a new tag node
/// let mut root = IksNode::new_tag("root");
///
/// // Add an attribute
/// root.add_attribute("version", "1.0");
///
/// // Add a child node
/// let mut child = IksNode::new_tag("child");
/// child.set_content("Hello World");
/// root.add_child(child);
/// ```
#[derive(Debug)]
pub struct IksNode {
    node_type: IksType,
    name: Option<String>,
    content: Option<String>,
    attributes: Vec<(String, String)>,
    children: Vec<Rc<RefCell<IksNode>>>,
    parent: Option<Weak<RefCell<IksNode>>>,
    next: Option<Rc<RefCell<IksNode>>>,
    prev: Option<Weak<RefCell<IksNode>>>,
    self_ref: Option<Weak<RefCell<IksNode>>>,
}

impl IksNode {
    /// Creates a new XML node of the specified type.
    ///
    /// # Arguments
    ///
    /// * `node_type` - The type of node to create
    ///
    /// # Returns
    ///
    /// A new `IksNode` instance
    pub fn new(node_type: IksType) -> Self {
        IksNode {
            node_type,
            name: None,
            content: None,
            attributes: Vec::with_capacity(memory::INITIAL_ATTR_CAPACITY),
            children: Vec::with_capacity(memory::INITIAL_CHILD_CAPACITY),
            parent: None,
            next: None,
            prev: None,
            self_ref: None,
        }
    }

    /// Creates a new tag node with the specified name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tag
    ///
    /// # Returns
    ///
    /// A new `IksNode` instance of type `Tag`
    pub fn new_tag<S: Into<String>>(name: S) -> Self {
        IksNode {
            node_type: IksType::Tag,
            name: Some(name.into()),
            content: None,
            attributes: Vec::with_capacity(memory::INITIAL_ATTR_CAPACITY),
            children: Vec::with_capacity(memory::INITIAL_CHILD_CAPACITY),
            parent: None,
            next: None,
            prev: None,
            self_ref: None,
        }
    }

    /// Creates a new CDATA node with the specified text content.
    pub fn new_cdata<S: Into<String>>(content: S) -> Self {
        IksNode {
            node_type: IksType::CData,
            name: None,
            content: Some(content.into()),
            attributes: Vec::new(),
            children: Vec::new(),
            parent: None,
            next: None,
            prev: None,
            self_ref: None,
        }
    }

    /// Creates a new XML node with preallocated capacities for attributes and children.
    pub fn with_capacity(node_type: IksType, attr_cap: usize, child_cap: usize) -> Self {
        IksNode {
            node_type,
            name: None,
            content: None,
            attributes: Vec::with_capacity(attr_cap),
            children: Vec::with_capacity(child_cap),
            parent: None,
            next: None,
            prev: None,
            self_ref: None,
        }
    }

    /// Creates a new tag node with preallocated capacities for attributes and children.
    pub fn with_capacity_tag<S: Into<String>>(name: S, attr_cap: usize, child_cap: usize) -> Self {
        IksNode {
            node_type: IksType::Tag,
            name: Some(name.into()),
            content: None,
            attributes: Vec::with_capacity(attr_cap),
            children: Vec::with_capacity(child_cap),
            parent: None,
            next: None,
            prev: None,
            self_ref: None,
        }
    }

    /// Wraps this node in an `Rc<RefCell<IksNode>>` and initializes internal self reference.
    pub fn into_rc(self) -> Rc<RefCell<Self>> {
        let rc = Rc::new(RefCell::new(self));
        rc.borrow_mut().self_ref = Some(Rc::downgrade(&rc));
        rc
    }

    /// Gets the parent node of this node.
    ///
    /// # Returns
    ///
    /// An `Option` containing the parent node if it exists
    pub fn parent(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.parent.as_ref().and_then(|w| w.upgrade())
    }

    /// Gets the next sibling node.
    ///
    /// # Returns
    ///
    /// An `Option` containing the next sibling node if it exists
    pub fn next(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.next.clone()
    }

    /// Gets the previous sibling node.
    ///
    /// # Returns
    ///
    /// An `Option` containing the previous sibling node if it exists
    pub fn prev(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.prev.as_ref().and_then(|w| w.upgrade())
    }

    /// Gets the next sibling tag node.
    ///
    /// This method skips any non-tag nodes (like text nodes) and returns
    /// the next sibling that is a tag node.
    ///
    /// # Returns
    ///
    /// An `Option` containing the next sibling tag node if it exists
    pub fn next_tag(&self) -> Option<Rc<RefCell<IksNode>>> {
        let mut next = self.next();
        while let Some(node) = next {
            if node.borrow().node_type == IksType::Tag {
                return Some(node);
            }
            next = node.borrow().next();
        }
        None
    }

    /// Finds the first child node with the specified tag name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tag to find
    ///
    /// # Returns
    ///
    /// An `Option` containing the matching child node if found
    pub fn find(&self, name: &str) -> Option<Rc<RefCell<IksNode>>> {
        self.children
            .iter()
            .find(|child| {
                let child = child.borrow();
                child.node_type == IksType::Tag && child.name.as_deref() == Some(name)
            })
            .cloned()
    }

    /// Finds the first child's CDATA content with the specified tag name.
    ///
    /// C `iks_find_cdata` (`iks.c:450-459`) bulunan düğümün **ilk çocuğunu**
    /// inceler; ilk çocuk CDATA değilse `None` döner.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tag to find
    ///
    /// # Returns
    ///
    /// An `Option` containing the CDATA content if found
    pub fn find_cdata(&self, name: &str) -> Option<String> {
        let node = self.find(name)?;
        let node = node.borrow();
        let first = node.children.first()?;
        let first = first.borrow();
        if first.node_type != IksType::CData {
            return None;
        }
        first.content.clone()
    }

    /// Finds all child tag nodes matching the specified tag name.
    pub fn find_all(&self, name: &str) -> Vec<Rc<RefCell<IksNode>>> {
        self.children
            .iter()
            .filter(|child| {
                let c = child.borrow();
                c.node_type == IksType::Tag && c.name.as_deref() == Some(name)
            })
            .cloned()
            .collect()
    }

    /// Returns all direct child nodes that are tags.
    pub fn child_tags(&self) -> Vec<Rc<RefCell<IksNode>>> {
        self.children
            .iter()
            .filter(|child| child.borrow().node_type == IksType::Tag)
            .cloned()
            .collect()
    }

    /// Returns the first direct child tag node if any.
    pub fn first_child_tag(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.children
            .iter()
            .find(|child| child.borrow().node_type == IksType::Tag)
            .cloned()
    }

    /// Finds a node by traversing a slice of hierarchical tag names.
    pub fn find_path(&self, path: &[&str]) -> Option<Rc<RefCell<IksNode>>> {
        if path.is_empty() {
            return None;
        }
        let mut current = self.find(path[0])?;
        for &segment in &path[1..] {
            let next = current.borrow().find(segment)?;
            current = next;
        }
        Some(current)
    }

    /// Finds a node by path and returns its text content if found.
    pub fn find_path_text(&self, path: &[&str]) -> Option<String> {
        self.find_path(path).map(|node| node.borrow().text())
    }

    /// Evaluates a path query selector supporting `/`-separated path segments
    /// and optional attribute filters (e.g. `query/item[subscription=both]` or `message/body`).
    pub fn select(&self, query: &str) -> Vec<Rc<RefCell<IksNode>>> {
        let parsed_segments: Vec<SelectorSegment<'_>> = query
            .split('/')
            .filter(|s| !s.is_empty())
            .map(parse_selector_segment)
            .collect();
        if parsed_segments.is_empty() {
            return Vec::new();
        }

        let mut current_set = Vec::new();
        self.match_parsed_segment(&parsed_segments[0], &mut current_set);

        for segment in &parsed_segments[1..] {
            let mut next_set = Vec::new();
            for node in current_set {
                node.borrow().match_parsed_segment(segment, &mut next_set);
            }
            current_set = next_set;
        }

        current_set
    }

    /// Evaluates a query selector and returns the first match if any.
    pub fn select_first(&self, query: &str) -> Option<Rc<RefCell<IksNode>>> {
        self.select(query).into_iter().next()
    }

    fn match_parsed_segment(
        &self,
        segment: &SelectorSegment<'_>,
        out: &mut Vec<Rc<RefCell<IksNode>>>,
    ) {
        let tag_name = segment.tag;
        let attr_filter = segment.filter;
        for child in &self.children {
            let c = child.borrow();
            if c.node_type != IksType::Tag {
                continue;
            }
            if tag_name != "*" && c.name.as_deref() != Some(tag_name) {
                continue;
            }
            if let Some((attr_k, attr_v_opt)) = attr_filter {
                if let Some(actual_val) = c.find_attrib(attr_k) {
                    if let Some(expected_val) = attr_v_opt {
                        if actual_val != expected_val {
                            continue;
                        }
                    }
                } else {
                    continue;
                }
            }
            out.push(child.clone());
        }
    }

    /// Recursively extracts all character data (text) from this node and its descendants.
    pub fn text(&self) -> String {
        let mut result = String::new();
        self.collect_text(&mut result);
        result
    }

    fn collect_text(&self, out: &mut String) {
        if let Some(ref text) = self.content {
            out.push_str(text);
        }
        for child in &self.children {
            child.borrow().collect_text(out);
        }
    }

    /// Shrinks the capacity of attributes and children vectors to fit their lengths, recursively.
    pub fn shrink_to_fit(&mut self) {
        self.attributes.shrink_to_fit();
        self.children.shrink_to_fit();
        for child in &self.children {
            child.borrow_mut().shrink_to_fit();
        }
    }

    /// Adds a child node to this node.
    ///
    /// # Arguments
    ///
    /// * `child` - The child node to add
    ///
    /// # Returns
    ///
    /// The added child node wrapped in an `Rc<RefCell<IksNode>>`
    pub fn add_child(&mut self, child: IksNode) -> Rc<RefCell<IksNode>> {
        let child_rc = child.into_rc();

        // Set up parent reference
        if let Some(self_rc) = self.as_rc() {
            child_rc.borrow_mut().parent = Some(Rc::downgrade(&self_rc));
        }

        // Set up sibling references
        if let Some(last_child) = self.children.last() {
            child_rc.borrow_mut().prev = Some(Rc::downgrade(last_child));
            last_child.borrow_mut().next = Some(child_rc.clone());
        }

        self.children.push(child_rc.clone());
        child_rc
    }

    /// Inserts a new tag node as a sibling.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the new tag
    ///
    /// # Returns
    ///
    /// The newly created tag node
    /// Gets the node type.
    pub fn node_type(&self) -> IksType {
        self.node_type
    }

    /// Gets the tag name if this is a tag node.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Gets the namespace prefix of this tag if one exists (e.g., "stream" in "stream:stream").
    pub fn prefix(&self) -> Option<&str> {
        let name = self.name()?;
        let colon = name.find(':')?;
        Some(&name[..colon])
    }

    /// Gets the local name part of this tag, omitting the prefix if present (e.g., "stream" in "stream:stream").
    pub fn local_name(&self) -> Option<&str> {
        let name = self.name()?;
        if let Some(colon) = name.find(':') {
            Some(&name[colon + 1..])
        } else {
            Some(name)
        }
    }

    /// Resolves the XML namespace URI bound to a prefix (or default namespace if prefix is None),
    /// walking up the DOM parent tree according to W3C XMLNS scoping rules.
    pub fn resolve_namespace(&self, prefix: Option<&str>) -> Option<String> {
        // Fast scan on self attributes without format! allocation
        for (k, v) in &self.attributes {
            if match prefix {
                Some(p) => k.strip_prefix("xmlns:") == Some(p),
                None => k == "xmlns",
            } {
                return Some(v.clone());
            }
        }

        // Iterative traversal up parent ancestors
        let mut curr_parent = self.parent();
        while let Some(parent_rc) = curr_parent {
            let p = parent_rc.borrow();
            for (k, v) in &p.attributes {
                if match prefix {
                    Some(pfx) => k.strip_prefix("xmlns:") == Some(pfx),
                    None => k == "xmlns",
                } {
                    return Some(v.clone());
                }
            }
            curr_parent = p.parent();
        }

        None
    }

    /// Resolves the namespace URI of this specific node based on its prefix and parent scope.
    pub fn namespace_uri(&self) -> Option<String> {
        self.resolve_namespace(self.prefix())
    }

    /// Gets the text content if this is a content node.
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    /// Gets the node attributes.
    pub fn attributes(&self) -> &[(String, String)] {
        &self.attributes
    }

    /// Gets the node children.
    pub fn children(&self) -> &[Rc<RefCell<IksNode>>] {
        &self.children
    }

    /// Adds a child node to a parent Rc node, maintaining bidirectional parent and sibling links.
    pub fn add_child_node(parent: &Rc<RefCell<IksNode>>, child: IksNode) -> Rc<RefCell<IksNode>> {
        let child_rc = Rc::new(RefCell::new(child));
        child_rc.borrow_mut().parent = Some(Rc::downgrade(parent));

        let mut p = parent.borrow_mut();
        if let Some(last_child) = p.children.last() {
            child_rc.borrow_mut().prev = Some(Rc::downgrade(last_child));
            last_child.borrow_mut().next = Some(child_rc.clone());
        }

        p.children.push(child_rc.clone());
        child_rc
    }

    /// Inserts a new tag node as a sibling after this node.
    pub fn insert_sibling<S: Into<String>>(&mut self, name: S) -> Option<Rc<RefCell<IksNode>>> {
        let parent_rc = self.parent.as_ref()?.upgrade()?;
        let sibling = IksNode::new_tag(name);
        let sibling_rc = sibling.into_rc();
        sibling_rc.borrow_mut().parent = Some(Rc::downgrade(&parent_rc));

        let mut p = parent_rc.borrow_mut();
        let idx = p
            .children
            .iter()
            .position(|c| std::ptr::eq(c.as_ptr() as *const _, self as *const _))?;

        if let Some(next) = p.children.get(idx + 1) {
            next.borrow_mut().prev = Some(Rc::downgrade(&sibling_rc));
            sibling_rc.borrow_mut().next = Some(next.clone());
        }

        let self_rc = p.children[idx].clone();
        self.next = Some(sibling_rc.clone());
        sibling_rc.borrow_mut().prev = Some(Rc::downgrade(&self_rc));

        p.children.insert(idx + 1, sibling_rc.clone());
        Some(sibling_rc)
    }

    /// Inserts CDATA content as a child node.
    ///
    /// # Arguments
    ///
    /// * `data` - The text content to insert
    ///
    /// # Returns
    ///
    /// The created CDATA node wrapped in an `Rc<RefCell<IksNode>>`
    pub fn insert_cdata<S: Into<String>>(&mut self, data: S) -> Rc<RefCell<IksNode>> {
        let mut cdata = IksNode::new(IksType::CData);
        cdata.set_content(data);
        self.add_child(cdata)
    }

    /// Adds an attribute to this node. If an attribute with the same name already exists,
    /// its value is updated in place (upsert).
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the attribute
    /// * `value` - The value of the attribute
    pub fn add_attribute<K: Into<String>, V: Into<String>>(&mut self, name: K, value: V) {
        let name_str = name.into();
        let value_str = value.into();
        if let Some((_, v)) = self.attributes.iter_mut().find(|(k, _)| k == &name_str) {
            *v = value_str;
        } else {
            self.attributes.push((name_str, value_str));
        }
    }

    /// Removes an attribute matching `name`. Returns `true` if found and removed.
    pub fn remove_attribute(&mut self, name: &str) -> bool {
        if let Some(pos) = self.attributes.iter().position(|(k, _)| k == name) {
            self.attributes.remove(pos);
            true
        } else {
            false
        }
    }

    /// Sets the content of this node. According to C `iks_set_cdata`, any existing children
    /// are removed before setting the new content.
    ///
    /// # Arguments
    ///
    /// * `content` - The content to set
    pub fn set_content<S: Into<String>>(&mut self, content: S) {
        self.children.clear();
        self.content = Some(content.into());
    }

    /// Detaches this node from its parent and sibling links (C `iks_hide`).
    pub fn hide(&mut self) {
        let prev_rc = self.prev.as_ref().and_then(|w| w.upgrade());
        let next_rc = self.next.clone();

        // 1. Update prev -> next link
        if let Some(ref prev) = prev_rc {
            prev.borrow_mut().next = next_rc.clone();
        }

        // 2. Update next -> prev link
        if let Some(ref next) = next_rc {
            next.borrow_mut().prev = prev_rc.as_ref().map(Rc::downgrade);
        }

        // 3. Remove self from parent's children list
        if let Some(parent_rc) = self.parent.as_ref().and_then(|w| w.upgrade()) {
            let mut p = parent_rc.borrow_mut();
            if let Some(idx) = p
                .children
                .iter()
                .position(|c| std::ptr::eq(c.as_ptr() as *const _, self as *const _))
            {
                p.children.remove(idx);
            }
        }

        // 4. Clear own links
        self.parent = None;
        self.prev = None;
        self.next = None;
    }

    /// Inserts a new tag node before this node in the parent tree.
    pub fn insert_before<S: Into<String>>(&mut self, name: S) -> Option<Rc<RefCell<IksNode>>> {
        let parent_rc = self.parent.as_ref()?.upgrade()?;
        let sibling = IksNode::new_tag(name);
        let sibling_rc = sibling.into_rc();
        sibling_rc.borrow_mut().parent = Some(Rc::downgrade(&parent_rc));

        let mut p = parent_rc.borrow_mut();
        let idx = p
            .children
            .iter()
            .position(|c| std::ptr::eq(c.as_ptr() as *const _, self as *const _))?;

        if idx > 0 {
            if let Some(prev) = p.children.get(idx - 1) {
                prev.borrow_mut().next = Some(sibling_rc.clone());
                sibling_rc.borrow_mut().prev = Some(Rc::downgrade(prev));
            }
        }

        let self_rc = p.children[idx].clone();
        self.prev = Some(Rc::downgrade(&sibling_rc));
        sibling_rc.borrow_mut().next = Some(self_rc);

        p.children.insert(idx, sibling_rc.clone());
        Some(sibling_rc)
    }

    /// Finds an attribute value by name.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the attribute to find
    ///
    /// # Returns
    ///
    /// An `Option` containing the attribute value if found
    pub fn find_attrib(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Finds the first child node with the specified attribute name and value.
    ///
    /// # Arguments
    ///
    /// * `tag_name` - Optional tag name to match
    /// * `attr_name` - The name of the attribute to match
    /// * `value` - The value of the attribute to match
    ///
    /// # Returns
    ///
    /// An `Option` containing the matching child node if found
    pub fn find_with_attrib(
        &self,
        tag_name: Option<&str>,
        attr_name: &str,
        value: &str,
    ) -> Option<Rc<RefCell<IksNode>>> {
        self.children
            .iter()
            .find(|child| {
                let child = child.borrow();
                if child.node_type != IksType::Tag {
                    return false;
                }
                if let Some(name) = tag_name {
                    if child.name.as_deref() != Some(name) {
                        return false;
                    }
                }
                child.find_attrib(attr_name) == Some(value)
            })
            .cloned()
    }

    /// Gets the first child tag node.
    ///
    /// # Returns
    ///
    /// An `Option` containing the first child tag node if it exists
    pub fn first_tag(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.children
            .iter()
            .find(|child| child.borrow().node_type == IksType::Tag)
            .cloned()
    }

    /// Checks if this node has any children.
    ///
    /// # Returns
    ///
    /// `true` if this node has one or more children
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Checks if this node has any attributes.
    ///
    /// # Returns
    ///
    /// `true` if this node has one or more attributes
    pub fn has_attributes(&self) -> bool {
        !self.attributes.is_empty()
    }

    /// Checks if this node has an attribute with the specified name.
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes.iter().any(|(k, _)| k == name)
    }

    /// Gets this node as an Rc if it's part of a parent tree.
    fn as_rc(&self) -> Option<Rc<RefCell<IksNode>>> {
        if let Some(ref w) = self.self_ref {
            if let Some(rc) = w.upgrade() {
                return Some(rc);
            }
        }
        self.parent
            .as_ref()
            .and_then(|w| w.upgrade())
            .and_then(|p| {
                let p_borrow = p.borrow();
                p_borrow
                    .children
                    .iter()
                    .find(|c| std::ptr::eq(c.as_ptr() as *const _, self as *const _))
                    .cloned()
            })
    }
    /// Creates a deep clone of this node and all of its child elements.
    pub fn deep_clone(&self) -> Self {
        let mut cloned = IksNode {
            node_type: self.node_type,
            name: self.name.clone(),
            content: self.content.clone(),
            attributes: self.attributes.clone(),
            children: Vec::with_capacity(self.children.len()),
            parent: None,
            next: None,
            prev: None,
            self_ref: None,
        };
        for child in &self.children {
            cloned.add_child(child.borrow().deep_clone());
        }
        cloned
    }

    /// Serializes this XML node and its tree directly to an IO writer.
    pub fn write_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut xml_writer = XmlWriter::new(writer);
        xml_writer.write_node(self)
    }

    /// Serializes this XML node and its tree into an indented, formatted string.
    pub fn to_pretty_string(&self, indent: usize) -> String {
        let mut buf = Vec::new();
        let mut xml_writer = XmlWriter::new(&mut buf);
        xml_writer.set_pretty(true, indent);
        let _ = xml_writer.write_node(self);
        String::from_utf8(buf).unwrap_or_default()
    }
}

impl Clone for IksNode {
    fn clone(&self) -> Self {
        self.deep_clone()
    }
}

impl fmt::Display for IksNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.node_type {
            IksType::Tag => {
                write!(f, "<{}", self.name.as_ref().unwrap())?;

                // Write attributes
                for (name, value) in &self.attributes {
                    write!(f, " {}=\"{}\"", name, crate::utility::escape_cow(value))?;
                }

                if self.children.is_empty() && self.content.is_none() {
                    write!(f, "/>")?;
                } else {
                    write!(f, ">")?;

                    // Write content if any
                    if let Some(content) = &self.content {
                        write!(f, "{}", crate::utility::escape_cow(content))?;
                    }

                    // Write children
                    for child in &self.children {
                        write!(f, "{}", child.borrow())?;
                    }

                    write!(f, "</{}>", self.name.as_ref().unwrap())?;
                }
            }
            IksType::CData => {
                if let Some(content) = &self.content {
                    write!(f, "{}", crate::utility::escape_cow(content))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// A reference-counted, interior-mutable wrapper around an XML node.
/// Guarantees that parent and sibling relationships remain connected.
#[derive(Clone, Debug)]
pub struct NodeRef(pub Rc<RefCell<IksNode>>);

impl NodeRef {
    /// Creates a new `NodeRef` of the given type.
    pub fn new(node_type: IksType) -> Self {
        IksNode::new(node_type).into_rc().into()
    }

    /// Creates a new `NodeRef` tag with the specified name.
    pub fn new_tag<S: Into<String>>(name: S) -> Self {
        IksNode::new_tag(name).into_rc().into()
    }

    /// Creates a new `NodeRef` CDATA node with the specified text content.
    pub fn new_cdata<S: Into<String>>(data: S) -> Self {
        IksNode::new_cdata(data).into_rc().into()
    }

    /// Borrows the wrapped node immutably.
    pub fn borrow(&self) -> std::cell::Ref<'_, IksNode> {
        self.0.borrow()
    }

    /// Borrows the wrapped node mutably.
    pub fn borrow_mut(&self) -> std::cell::RefMut<'_, IksNode> {
        self.0.borrow_mut()
    }

    /// Returns a reference to the inner `Rc<RefCell<IksNode>>`.
    pub fn as_rc(&self) -> &Rc<RefCell<IksNode>> {
        &self.0
    }

    /// Gets the node type.
    pub fn node_type(&self) -> IksType {
        self.0.borrow().node_type()
    }

    /// Gets the tag name if this is a tag node.
    pub fn name(&self) -> Option<String> {
        self.0.borrow().name().map(|s| s.to_string())
    }

    /// Gets the namespace prefix of this tag if one exists.
    pub fn prefix(&self) -> Option<String> {
        self.0.borrow().prefix().map(|s| s.to_string())
    }

    /// Gets the local name part of this tag, omitting the prefix if present.
    pub fn local_name(&self) -> Option<String> {
        self.0.borrow().local_name().map(|s| s.to_string())
    }

    /// Recursively extracts all character data (text) from this node and its descendants.
    pub fn text(&self) -> String {
        self.0.borrow().text()
    }

    /// Gets the content of this node if it is a CDATA node.
    pub fn content(&self) -> Option<String> {
        self.0.borrow().content().map(|s| s.to_string())
    }

    /// Gets the attributes of this node.
    pub fn attributes(&self) -> Vec<(String, String)> {
        self.0.borrow().attributes().to_vec()
    }

    /// Checks if this node has any attributes.
    pub fn has_attributes(&self) -> bool {
        self.0.borrow().has_attributes()
    }

    /// Checks if this node has an attribute with the given name.
    pub fn has_attribute(&self, name: &str) -> bool {
        self.0.borrow().has_attribute(name)
    }

    /// Finds the value of an attribute by name.
    pub fn find_attrib(&self, name: &str) -> Option<String> {
        self.0.borrow().find_attrib(name).map(|s| s.to_string())
    }

    /// Adds an attribute to this node (upsert).
    pub fn add_attribute<K: Into<String>, V: Into<String>>(&self, name: K, value: V) {
        self.0.borrow_mut().add_attribute(name, value);
    }

    /// Removes an attribute by name. Returns true if found and removed.
    pub fn remove_attribute(&self, name: &str) -> bool {
        self.0.borrow_mut().remove_attribute(name)
    }

    /// Sets the content of this node, clearing any existing children.
    pub fn set_content<S: Into<String>>(&self, content: S) {
        self.0.borrow_mut().set_content(content);
    }

    /// Sets the CDATA content of this node (alias for set_content).
    pub fn set_cdata<S: Into<String>>(&self, content: S) {
        self.set_content(content);
    }

    /// Serializes this XML node and its tree directly to an IO writer.
    pub fn write_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.0.borrow().write_to(writer)
    }

    /// Serializes this XML node and its tree into an indented, formatted string.
    pub fn to_pretty_string(&self, indent: usize) -> String {
        self.0.borrow().to_pretty_string(indent)
    }

    /// Adds a child node to this node, establishing bidirectional parent and sibling links.
    pub fn add_child(&self, child: impl Into<NodeRef>) -> NodeRef {
        let child_ref = child.into();
        child_ref.0.borrow_mut().parent = Some(Rc::downgrade(&self.0));

        let mut p = self.0.borrow_mut();
        if let Some(last_child) = p.children.last() {
            child_ref.0.borrow_mut().prev = Some(Rc::downgrade(last_child));
            last_child.borrow_mut().next = Some(child_ref.0.clone());
        }

        p.children.push(child_ref.0.clone());
        child_ref
    }

    /// Alias for `add_child` (C `iks_insert_node`).
    pub fn insert_node(&self, child: impl Into<NodeRef>) -> NodeRef {
        self.add_child(child)
    }

    /// Creates a deep copy of this subtree, ensuring all parent and sibling links
    /// in the newly cloned tree are correctly wired.
    pub fn clone_subtree(&self) -> NodeRef {
        let this = self.0.borrow();
        let new_node = NodeRef::new(this.node_type);
        {
            let mut mut_node = new_node.0.borrow_mut();
            mut_node.name = this.name.clone();
            mut_node.content = this.content.clone();
            mut_node.attributes = this.attributes.clone();
        }
        for child in &this.children {
            let child_ref = NodeRef(child.clone());
            let cloned_child = child_ref.clone_subtree();
            new_node.add_child(cloned_child);
        }
        new_node
    }

    /// Gets the parent node if it exists.
    pub fn parent(&self) -> Option<NodeRef> {
        self.0.borrow().parent().map(NodeRef)
    }

    /// Gets the root of the tree by traversing up parent links.
    pub fn root(&self) -> NodeRef {
        let mut cur = self.clone();
        while let Some(p) = cur.parent() {
            cur = p;
        }
        cur
    }

    /// Gets the first child node if any exists.
    pub fn first_child(&self) -> Option<NodeRef> {
        self.0.borrow().children().first().cloned().map(NodeRef)
    }

    /// Gets the first child tag node if any exists.
    pub fn first_tag(&self) -> Option<NodeRef> {
        self.0.borrow().first_tag().map(NodeRef)
    }

    /// Gets the last child node if any exists.
    pub fn last_child(&self) -> Option<NodeRef> {
        self.0.borrow().children().last().cloned().map(NodeRef)
    }

    /// Gets the next sibling node.
    pub fn next(&self) -> Option<NodeRef> {
        self.0.borrow().next().map(NodeRef)
    }

    /// Gets the previous sibling node.
    pub fn prev(&self) -> Option<NodeRef> {
        self.0.borrow().prev().map(NodeRef)
    }

    /// Gets the next sibling that is a tag node.
    pub fn next_tag(&self) -> Option<NodeRef> {
        let mut cur = self.next();
        while let Some(n) = cur {
            if n.node_type() == IksType::Tag {
                return Some(n);
            }
            cur = n.next();
        }
        None
    }

    /// Gets the previous sibling that is a tag node.
    pub fn prev_tag(&self) -> Option<NodeRef> {
        let mut cur = self.prev();
        while let Some(p) = cur {
            if p.node_type() == IksType::Tag {
                return Some(p);
            }
            cur = p.prev();
        }
        None
    }

    /// Gets all direct children as a vector of `NodeRef`.
    pub fn children(&self) -> Vec<NodeRef> {
        self.0.borrow().children().iter().cloned().map(NodeRef).collect()
    }

    /// Gets all direct child tag nodes as a vector of `NodeRef`.
    pub fn child_tags(&self) -> Vec<NodeRef> {
        self.0.borrow().child_tags().into_iter().map(NodeRef).collect()
    }

    /// Checks if this node has any children.
    pub fn has_children(&self) -> bool {
        self.0.borrow().has_children()
    }

    /// Detaches this node from its parent and sibling links (C `iks_hide`).
    pub fn hide(&self) {
        self.0.borrow_mut().hide();
    }

    /// Inserts a CDATA child node.
    pub fn insert_cdata<S: Into<String>>(&self, data: S) -> NodeRef {
        let cdata = NodeRef::new_cdata(data);
        self.add_child(cdata)
    }

    /// Inserts a CDATA node immediately after this node in its parent (C `iks_append_cdata`).
    pub fn append_cdata<S: Into<String>>(&self, data: S) -> Option<NodeRef> {
        let parent = self.parent()?;
        let cdata = NodeRef::new_cdata(data);
        cdata.0.borrow_mut().parent = Some(Rc::downgrade(&parent.0));

        let mut p = parent.0.borrow_mut();
        let idx = p
            .children
            .iter()
            .position(|c| std::ptr::eq(c.as_ptr() as *const _, self.0.as_ptr() as *const _))?;

        if let Some(next) = p.children.get(idx + 1) {
            next.borrow_mut().prev = Some(Rc::downgrade(&cdata.0));
            cdata.0.borrow_mut().next = Some(next.clone());
        }

        cdata.0.borrow_mut().prev = Some(Rc::downgrade(&self.0));
        self.0.borrow_mut().next = Some(cdata.0.clone());

        p.children.insert(idx + 1, cdata.0.clone());
        Some(cdata)
    }

    /// Inserts a CDATA node immediately before this node in its parent (C `iks_prepend_cdata`).
    pub fn prepend_cdata<S: Into<String>>(&self, data: S) -> Option<NodeRef> {
        let parent = self.parent()?;
        let cdata = NodeRef::new_cdata(data);
        cdata.0.borrow_mut().parent = Some(Rc::downgrade(&parent.0));

        let mut p = parent.0.borrow_mut();
        let idx = p
            .children
            .iter()
            .position(|c| std::ptr::eq(c.as_ptr() as *const _, self.0.as_ptr() as *const _))?;

        if idx > 0 {
            if let Some(prev) = p.children.get(idx - 1) {
                prev.borrow_mut().next = Some(cdata.0.clone());
                cdata.0.borrow_mut().prev = Some(Rc::downgrade(prev));
            }
        }

        self.0.borrow_mut().prev = Some(Rc::downgrade(&cdata.0));
        cdata.0.borrow_mut().next = Some(self.0.clone());

        p.children.insert(idx, cdata.0.clone());
        Some(cdata)
    }

    /// Finds the first child tag matching `name`.
    pub fn find(&self, name: &str) -> Option<NodeRef> {
        self.0.borrow().find(name).map(NodeRef)
    }

    /// Finds all direct child tags matching `name`.
    pub fn find_all(&self, name: &str) -> Vec<NodeRef> {
        self.0.borrow().find_all(name).into_iter().map(NodeRef).collect()
    }

    /// Finds a tag matching `name` and returns its first child CDATA content.
    pub fn find_cdata(&self, name: &str) -> Option<String> {
        self.0.borrow().find_cdata(name)
    }

    /// Finds a node by traversing hierarchical tag names.
    pub fn find_path(&self, path: &[&str]) -> Option<NodeRef> {
        self.0.borrow().find_path(path).map(NodeRef)
    }

    /// Finds a node by hierarchical tag names and extracts its text.
    pub fn find_path_text(&self, path: &[&str]) -> Option<String> {
        self.0.borrow().find_path_text(path)
    }

    /// Evaluates a selector query on this node and returns matching `NodeRef`s.
    pub fn select(&self, query: &str) -> Vec<NodeRef> {
        self.0.borrow().select(query).into_iter().map(NodeRef).collect()
    }

    /// Evaluates a selector query and returns the first matching `NodeRef`.
    pub fn select_first(&self, query: &str) -> Option<NodeRef> {
        self.0.borrow().select_first(query).map(NodeRef)
    }
}

impl PartialEq for NodeRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for NodeRef {}

impl std::fmt::Display for NodeRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.borrow())
    }
}

impl std::ops::Deref for NodeRef {
    type Target = RefCell<IksNode>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Rc<RefCell<IksNode>>> for NodeRef {
    fn from(rc: Rc<RefCell<IksNode>>) -> Self {
        NodeRef(rc)
    }
}

impl From<NodeRef> for Rc<RefCell<IksNode>> {
    fn from(nr: NodeRef) -> Self {
        nr.0
    }
}

impl From<IksNode> for NodeRef {
    fn from(node: IksNode) -> Self {
        node.into_rc().into()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for IksNode {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for IksNode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DomParser::parse_str(&s)
            .map(|rc| rc.borrow().clone())
            .map_err(serde::de::Error::custom)
    }
}

/// `parent`'a metin ekler; son çocuk CDATA ise **ona ekler**.
///
/// C `iks_insert_cdata`'nın kuralı: son çocuk CDATA ise yeni düğüm açılmaz.
/// Hem DOM hem akış handler'ı bu tek kuralı paylaşır.
pub(crate) fn append_text(parent: &Rc<RefCell<IksNode>>, data: &str) {
    let merge_target = {
        let p = parent.borrow();
        match p.children.last() {
            Some(last) if last.borrow().node_type == IksType::CData => Some(Rc::clone(last)),
            _ => None,
        }
    };

    match merge_target {
        Some(last) => {
            let mut last_ref = last.borrow_mut();
            match last_ref.content.as_mut() {
                Some(content) => content.push_str(data),
                None => last_ref.content = Some(data.to_string()),
            }
        }
        None => {
            let cdata = IksNode::new_cdata(data);
            parent.borrow_mut().add_child(cdata);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SelectorSegment<'a> {
    tag: &'a str,
    filter: Option<(&'a str, Option<&'a str>)>,
}

/// Parses a selector segment like "item[sub=both]" into a `SelectorSegment`
fn parse_selector_segment(s: &str) -> SelectorSegment<'_> {
    let s = s.trim();
    if let (Some(start), Some(end)) = (s.find('['), s.rfind(']')) {
        if start < end {
            let tag = s[..start].trim();
            let inside = s[start + 1..end].trim();
            if let Some(eq) = inside.find('=') {
                let key = inside[..eq].trim();
                let val = inside[eq + 1..]
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"');
                return SelectorSegment {
                    tag,
                    filter: Some((key, Some(val))),
                };
            } else if !inside.is_empty() {
                return SelectorSegment {
                    tag,
                    filter: Some((inside, None)),
                };
            }
            return SelectorSegment { tag, filter: None };
        }
    }
    SelectorSegment {
        tag: s,
        filter: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let mut node = IksNode::new_tag("root");
        assert_eq!(node.node_type, IksType::Tag);
        assert_eq!(node.name, Some("root".to_string()));

        node.add_attribute("attr", "value");
        assert_eq!(node.attributes.len(), 1);

        let mut child = IksNode::new_tag("child");
        child.set_content("text");
        node.add_child(child);

        assert_eq!(node.children.len(), 1);
    }

    #[test]
    fn test_node_display() {
        let mut node = IksNode::new_tag("test");
        node.add_attribute("attr", "value");
        node.set_content("content");

        assert_eq!(node.to_string(), "<test attr=\"value\">content</test>");
    }

    #[test]
    fn test_node_navigation() {
        let root = IksNode::new_tag("root").into_rc();

        let mut child1 = IksNode::new_tag("child1");
        child1.add_attribute("id", "1");
        root.borrow_mut().add_child(child1);

        let mut child2 = IksNode::new_tag("child2");
        child2.add_attribute("id", "2");
        root.borrow_mut().add_child(child2);

        // Test find methods
        let found = root.borrow().find("child1").unwrap();
        assert_eq!(found.borrow().name.as_deref(), Some("child1"));

        let found = root.borrow().find_with_attrib(None, "id", "2").unwrap();
        assert_eq!(found.borrow().name.as_deref(), Some("child2"));

        // Test navigation
        {
            let root_ref = root.borrow();
            let children = &root_ref.children;

            let first = &children[0];
            assert_eq!(first.borrow().name.as_deref(), Some("child1"));
            assert_eq!(
                first.borrow().parent().unwrap().borrow().name.as_deref(),
                Some("root")
            );
            assert!(first.borrow().prev().is_none());

            let second = &children[1];
            assert_eq!(second.borrow().name.as_deref(), Some("child2"));
            assert_eq!(
                second.borrow().parent().unwrap().borrow().name.as_deref(),
                Some("root")
            );
            assert!(second.borrow().next().is_none());

            assert_eq!(
                first.borrow().next().unwrap().borrow().name.as_deref(),
                Some("child2")
            );
            assert_eq!(
                second.borrow().prev().unwrap().borrow().name.as_deref(),
                Some("child1")
            );
        }

        // Test insert_sibling and insert_before
        let child1_node = root.borrow().find("child1").unwrap();
        child1_node.borrow_mut().insert_sibling("child1_5").unwrap();
        assert_eq!(root.borrow().children.len(), 3);
        assert_eq!(
            root.borrow().children[1].borrow().name.as_deref(),
            Some("child1_5")
        );

        let child1_5 = root.borrow().find("child1_5").unwrap();
        child1_5.borrow_mut().insert_before("child1_25").unwrap();
        assert_eq!(root.borrow().children.len(), 4);
        assert_eq!(
            root.borrow().children[1].borrow().name.as_deref(),
            Some("child1_25")
        );
    }

    #[test]
    fn test_cdata_handling() {
        let root = Rc::new(RefCell::new(IksNode::new_tag("root")));

        let mut child = IksNode::new_tag("child");
        let _cdata = child.insert_cdata("Hello World");
        root.borrow_mut().add_child(child);

        let content = root.borrow().find_cdata("child").unwrap();
        assert_eq!(content, "Hello World");
    }

    #[test]
    fn test_attributes() {
        let mut node = IksNode::new_tag("test");
        node.add_attribute("id", "123");
        node.add_attribute("class", "test");

        assert!(node.has_attributes());
        assert_eq!(node.find_attrib("id"), Some("123"));
        assert_eq!(node.find_attrib("class"), Some("test"));
        assert_eq!(node.find_attrib("missing"), None);
    }

    #[test]
    fn test_node_extensions_and_helpers() {
        let mut root = IksNode::with_capacity_tag("root", 4, 4);
        root.add_attribute("k", "v");

        let mut c1 = IksNode::new_tag("item");
        c1.insert_cdata("First ");
        root.add_child(c1);

        let mut c2 = IksNode::new_tag("item");
        c2.insert_cdata("Second");
        root.add_child(c2);

        let mut c3 = IksNode::new_tag("other");
        c3.insert_cdata(" ignored");
        root.add_child(c3);

        // find_all
        let items = root.find_all("item");
        assert_eq!(items.len(), 2);

        // child_tags
        let tags = root.child_tags();
        assert_eq!(tags.len(), 3);

        // text
        assert_eq!(root.text(), "First Second ignored");

        // shrink_to_fit
        root.shrink_to_fit();
    }

    #[test]
    fn test_path_traversal_and_selectors() {
        let mut root = IksNode::new_tag("iq");
        let mut query = IksNode::new_tag("query");
        query.add_attribute("xmlns", "jabber:iq:roster");

        let mut item1 = IksNode::new_tag("item");
        item1.add_attribute("jid", "alice@example.com");
        item1.add_attribute("sub", "both");
        let mut group1 = IksNode::new_tag("group");
        group1.insert_cdata("Friends");
        item1.add_child(group1);

        let mut item2 = IksNode::new_tag("item");
        item2.add_attribute("jid", "bob@example.com");
        item2.add_attribute("sub", "to");

        query.add_child(item1);
        query.add_child(item2);
        root.add_child(query);

        // test first_child_tag
        assert_eq!(
            root.first_child_tag().unwrap().borrow().name(),
            Some("query")
        );

        // test find_path
        let found_item = root.find_path(&["query", "item"]);
        assert!(found_item.is_some());
        assert_eq!(
            found_item.unwrap().borrow().find_attrib("jid"),
            Some("alice@example.com")
        );

        // test find_path_text
        assert_eq!(
            root.find_path_text(&["query", "item", "group"]),
            Some("Friends".to_string())
        );
        assert_eq!(root.find_path_text(&["query", "missing"]), None);

        // test select
        let all_items = root.select("query/item");
        assert_eq!(all_items.len(), 2);

        let both_items = root.select("query/item[sub=both]");
        assert_eq!(both_items.len(), 1);
        assert_eq!(
            both_items[0].borrow().find_attrib("jid"),
            Some("alice@example.com")
        );

        let to_items = root.select("query/item[sub='to']");
        assert_eq!(to_items.len(), 1);
        assert_eq!(
            to_items[0].borrow().find_attrib("jid"),
            Some("bob@example.com")
        );

        let has_jid = root.select("query/item[jid]");
        assert_eq!(has_jid.len(), 2);

        let first = root.select_first("query/item[sub=to]");
        assert!(first.is_some());
        assert_eq!(
            first.unwrap().borrow().find_attrib("jid"),
            Some("bob@example.com")
        );
    }

    #[test]
    fn test_namespace_resolution() {
        let xml = r#"<stream:stream xmlns="jabber:client" xmlns:stream="http://etherx.jabber.org/streams"><message to="bob@example.com"><body>Hello</body></message></stream:stream>"#;
        let doc = DomParser::parse_str(xml).expect("parse xml");
        let root = doc.borrow();

        assert_eq!(root.name(), Some("stream:stream"));
        assert_eq!(root.prefix(), Some("stream"));
        assert_eq!(root.local_name(), Some("stream"));
        assert_eq!(
            root.namespace_uri(),
            Some("http://etherx.jabber.org/streams".to_string())
        );

        let msg = root.find("message").expect("find message");
        let msg_ref = msg.borrow();
        assert_eq!(msg_ref.prefix(), None);
        assert_eq!(msg_ref.local_name(), Some("message"));
        // Inherited default namespace from stream:stream
        assert_eq!(msg_ref.namespace_uri(), Some("jabber:client".to_string()));
        // Inherited stream prefix
        assert_eq!(
            msg_ref.resolve_namespace(Some("stream")),
            Some("http://etherx.jabber.org/streams".to_string())
        );
    }

    #[test]
    fn test_attribute_upsert_and_removal() {
        let mut node = IksNode::new_tag("item");
        node.add_attribute("key", "val1");
        assert_eq!(node.find_attrib("key"), Some("val1"));
        assert_eq!(node.attributes().len(), 1);

        // Upsert: aynı anahtar tekrar eklendiğinde liste uzamamalı, değer güncellenmeli
        node.add_attribute("key", "val2");
        assert_eq!(node.find_attrib("key"), Some("val2"));
        assert_eq!(node.attributes().len(), 1);

        // Silme: mevcut anahtar silinmeli ve true dönmeli
        assert!(node.remove_attribute("key"));
        assert_eq!(node.find_attrib("key"), None);
        assert_eq!(node.attributes().len(), 0);

        // Olmayan anahtar silinemez ve false döner
        assert!(!node.remove_attribute("nonexistent"));
    }

    #[test]
    fn test_set_content_clears_children() {
        let mut parent = IksNode::new_tag("p");
        let child = IksNode::new_tag("em");
        parent.add_child(child);
        assert!(parent.has_children());

        // C iks_set_cdata kuralı: içerik atandığında eski çocuklar silinir
        parent.set_content("new text");
        assert!(!parent.has_children());
        assert_eq!(parent.content(), Some("new text"));
    }

    #[test]
    fn test_iks_node_hide() {
        let root = IksNode::new_tag("root").into_rc();
        let c1 = root.borrow_mut().add_child(IksNode::new_tag("c1"));
        let c2 = root.borrow_mut().add_child(IksNode::new_tag("c2"));
        let c3 = root.borrow_mut().add_child(IksNode::new_tag("c3"));

        assert_eq!(root.borrow().children().len(), 3);
        assert_eq!(c1.borrow().next().unwrap().borrow().name(), Some("c2"));
        assert_eq!(c3.borrow().prev().unwrap().borrow().name(), Some("c2"));

        // c2'yi gizle / sök
        c2.borrow_mut().hide();

        assert_eq!(root.borrow().children().len(), 2);
        assert_eq!(c1.borrow().next().unwrap().borrow().name(), Some("c3"));
        assert_eq!(c3.borrow().prev().unwrap().borrow().name(), Some("c1"));
        assert!(c2.borrow().parent().is_none());
        assert!(c2.borrow().next().is_none());
        assert!(c2.borrow().prev().is_none());
    }

    #[test]
    fn test_noderef_basics() {
        let node = NodeRef::new_tag("item");
        assert_eq!(node.name().as_deref(), Some("item"));
        assert_eq!(node.node_type(), IksType::Tag);

        node.add_attribute("id", "42");
        assert_eq!(node.find_attrib("id").as_deref(), Some("42"));
        assert!(node.has_attribute("id"));
        assert!(!node.has_attribute("missing"));

        // Upsert
        node.add_attribute("id", "43");
        assert_eq!(node.find_attrib("id").as_deref(), Some("43"));
        assert_eq!(node.attributes().len(), 1);

        // Remove
        assert!(node.remove_attribute("id"));
        assert!(!node.has_attribute("id"));

        // Content
        node.set_content("some text");
        assert_eq!(node.content().as_deref(), Some("some text"));
        assert_eq!(node.text(), "some text");

        // Display
        assert_eq!(node.to_string(), "<item>some text</item>");
    }

    #[test]
    fn test_noderef_tree_operations_and_parent_guarantee() {
        let root = NodeRef::new_tag("root");
        let child1 = root.add_child(NodeRef::new_tag("child1"));
        let child2 = root.add_child(NodeRef::new_tag("child2"));
        let subchild = child1.add_child(NodeRef::new_tag("subchild"));

        // 4a garantisi: her çocukta parent() daima Some döner
        assert_eq!(child1.parent(), Some(root.clone()));
        assert_eq!(child2.parent(), Some(root.clone()));
        assert_eq!(subchild.parent(), Some(child1.clone()));
        assert_eq!(subchild.root(), root);

        // Kardeş navigasyonu
        assert_eq!(child1.next(), Some(child2.clone()));
        assert_eq!(child2.prev(), Some(child1.clone()));

        // append_cdata & prepend_cdata
        let cdata_after = child1.append_cdata("mid text").expect("append cdata");
        assert_eq!(child1.next(), Some(cdata_after.clone()));
        assert_eq!(cdata_after.next(), Some(child2.clone()));
        assert_eq!(cdata_after.parent(), Some(root.clone()));

        // 4b garantisi: clone_subtree tüm bağları korur
        let cloned_root = root.clone_subtree();
        assert_ne!(cloned_root, root); // Farklı Rc
        let cloned_c1 = cloned_root.first_tag().expect("first tag");
        assert_eq!(cloned_c1.name().as_deref(), Some("child1"));
        assert_eq!(cloned_c1.parent(), Some(cloned_root.clone())); // Klon parent'ı klon root!
        let cloned_sub = cloned_c1.first_tag().expect("sub tag");
        assert_eq!(cloned_sub.parent(), Some(cloned_c1.clone()));
    }
}

