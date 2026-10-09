/*
            iksemel - XML parser for Rust
          Copyright (C) 2026 Süleyman Poyraz
 This code is free software; you can redistribute it and/or
 modify it under the terms of the GNU Lesser General Public License
 as published by the Free Software Foundation; either version 2.1
 of the License, or (at your option) any later version.
 This program is distributed in the hope that it will be useful,
 but WITHOUT ANY WARRANTY; without even the implied warranty of
 MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 GNU Lesser General Public License for more details.
*/

use crate::{IksError, IksNode, Jid, Result};
use std::str::FromStr;

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

/// Checks whether an incoming stanza (as `NodeRef`) is an XEP-0199 Ping request.
pub fn is_ping_ref(stanza: &crate::NodeRef) -> bool {
    is_ping(&stanza.borrow())
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

/// Builds an XEP-0199 Pong response for a given incoming Ping IQ (as `NodeRef`).
pub fn build_pong_ref(ping_iq: &crate::NodeRef) -> Result<IksNode> {
    build_pong(&ping_iq.borrow())
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

// ============================================================================
// XEP-0004: Data Forms
// ============================================================================

pub const XMLNS_DATA_FORMS: &str = "jabber:x:data";

/// Form type according to XEP-0004 Section 3.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DataFormType {
    Form,
    Submit,
    Cancel,
    Result,
}

impl DataFormType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataFormType::Form => "form",
            DataFormType::Submit => "submit",
            DataFormType::Cancel => "cancel",
            DataFormType::Result => "result",
        }
    }
}

impl FromStr for DataFormType {
    type Err = IksError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "form" => Ok(DataFormType::Form),
            "submit" => Ok(DataFormType::Submit),
            "cancel" => Ok(DataFormType::Cancel),
            "result" => Ok(DataFormType::Result),
            _ => Err(IksError::BadXml),
        }
    }
}

/// Field types defined in XEP-0004 Section 3.2.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FieldType {
    Boolean,
    Fixed,
    Hidden,
    JidMulti,
    JidSingle,
    ListMulti,
    ListSingle,
    TextMulti,
    TextPrivate,
    TextSingle,
}

impl FieldType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::Boolean => "boolean",
            FieldType::Fixed => "fixed",
            FieldType::Hidden => "hidden",
            FieldType::JidMulti => "jid-multi",
            FieldType::JidSingle => "jid-single",
            FieldType::ListMulti => "list-multi",
            FieldType::ListSingle => "list-single",
            FieldType::TextMulti => "text-multi",
            FieldType::TextPrivate => "text-private",
            FieldType::TextSingle => "text-single",
        }
    }
}

impl FromStr for FieldType {
    type Err = IksError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "boolean" => Ok(FieldType::Boolean),
            "fixed" => Ok(FieldType::Fixed),
            "hidden" => Ok(FieldType::Hidden),
            "jid-multi" => Ok(FieldType::JidMulti),
            "jid-single" => Ok(FieldType::JidSingle),
            "list-multi" => Ok(FieldType::ListMulti),
            "list-single" => Ok(FieldType::ListSingle),
            "text-multi" => Ok(FieldType::TextMulti),
            "text-private" => Ok(FieldType::TextPrivate),
            "text-single" => Ok(FieldType::TextSingle),
            _ => Err(IksError::BadXml),
        }
    }
}

/// An option within a list-single or list-multi field.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FieldOption {
    pub label: Option<String>,
    pub value: String,
}

impl FieldOption {
    pub fn new(value: impl Into<String>, label: Option<impl Into<String>>) -> Self {
        Self {
            value: value.into(),
            label: label.map(Into::into),
        }
    }
}

/// A field within an XEP-0004 Data Form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FormField {
    pub var: String,
    pub field_type: Option<FieldType>,
    pub label: Option<String>,
    pub desc: Option<String>,
    pub required: bool,
    pub values: Vec<String>,
    pub options: Vec<FieldOption>,
}

impl FormField {
    pub fn new(var: impl Into<String>) -> Self {
        Self {
            var: var.into(),
            field_type: None,
            label: None,
            desc: None,
            required: false,
            values: Vec::new(),
            options: Vec::new(),
        }
    }

    pub fn with_type(mut self, field_type: FieldType) -> Self {
        self.field_type = Some(field_type);
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_desc(mut self, desc: impl Into<String>) -> Self {
        self.desc = Some(desc.into());
        self
    }

    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.values.push(value.into());
        self
    }

    pub fn with_values<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for val in values {
            self.values.push(val.into());
        }
        self
    }

    pub fn add_value(&mut self, value: impl Into<String>) {
        self.values.push(value.into());
    }

    pub fn with_option(
        mut self,
        value: impl Into<String>,
        label: Option<impl Into<String>>,
    ) -> Self {
        self.options.push(FieldOption::new(value, label));
        self
    }

    pub fn first_value(&self) -> Option<&str> {
        self.values.first().map(|s| s.as_str())
    }

    pub fn as_boolean(&self) -> Option<bool> {
        match self.first_value()? {
            "1" | "true" => Some(true),
            "0" | "false" => Some(false),
            _ => None,
        }
    }

    pub fn to_node(&self) -> IksNode {
        let mut node = IksNode::new_tag("field");
        if !self.var.is_empty() {
            node.add_attribute("var", &self.var);
        }
        if let Some(ft) = &self.field_type {
            node.add_attribute("type", ft.as_str());
        }
        if let Some(lbl) = &self.label {
            node.add_attribute("label", lbl);
        }
        if let Some(desc) = &self.desc {
            let mut desc_node = IksNode::new_tag("desc");
            desc_node.insert_cdata(desc);
            node.add_child(desc_node);
        }
        if self.required {
            node.add_child(IksNode::new_tag("required"));
        }
        for val in &self.values {
            let mut val_node = IksNode::new_tag("value");
            val_node.insert_cdata(val);
            node.add_child(val_node);
        }
        for opt in &self.options {
            let mut opt_node = IksNode::new_tag("option");
            if let Some(lbl) = &opt.label {
                opt_node.add_attribute("label", lbl);
            }
            let mut val_node = IksNode::new_tag("value");
            val_node.insert_cdata(&opt.value);
            opt_node.add_child(val_node);
            node.add_child(opt_node);
        }
        node
    }

    pub fn from_node(node: &IksNode) -> Result<Self> {
        if node.name() != Some("field") {
            return Err(IksError::BadXml);
        }
        let var = node.find_attrib("var").unwrap_or_default().to_string();
        let field_type = node
            .find_attrib("type")
            .and_then(|t| FieldType::from_str(t).ok());
        let label = node.find_attrib("label").map(|s| s.to_string());
        let mut desc = None;
        let mut required = false;
        let mut values = Vec::new();
        let mut options = Vec::new();

        for child in node.children() {
            let c = child.borrow();
            match c.name() {
                Some("desc") => {
                    desc = Some(c.text());
                }
                Some("required") => {
                    required = true;
                }
                Some("value") => {
                    values.push(c.text());
                }
                Some("option") => {
                    let opt_label = c.find_attrib("label").map(|s| s.to_string());
                    let opt_val = c
                        .find("value")
                        .map(|v| v.borrow().text())
                        .unwrap_or_default();
                    options.push(FieldOption {
                        label: opt_label,
                        value: opt_val,
                    });
                }
                _ => {}
            }
        }

        Ok(FormField {
            var,
            field_type,
            label,
            desc,
            required,
            values,
            options,
        })
    }
}

/// An XEP-0004 Data Form (`<x xmlns='jabber:x:data'>`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DataForm {
    pub form_type: Option<DataFormType>,
    pub title: Option<String>,
    pub instructions: Vec<String>,
    pub fields: Vec<FormField>,
    pub reported: Vec<FormField>,
    pub items: Vec<Vec<FormField>>,
}

impl DataForm {
    pub fn new(form_type: DataFormType) -> Self {
        Self {
            form_type: Some(form_type),
            title: None,
            instructions: Vec::new(),
            fields: Vec::new(),
            reported: Vec::new(),
            items: Vec::new(),
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instructions.push(instruction.into());
        self
    }

    pub fn add_field(&mut self, field: FormField) {
        self.fields.push(field);
    }

    pub fn get_field(&self, var: &str) -> Option<&FormField> {
        self.fields.iter().find(|f| f.var == var)
    }

    pub fn get_field_mut(&mut self, var: &str) -> Option<&mut FormField> {
        self.fields.iter_mut().find(|f| f.var == var)
    }

    pub fn get_value(&self, var: &str) -> Option<&str> {
        self.get_field(var).and_then(|f| f.first_value())
    }

    pub fn to_node(&self) -> IksNode {
        let mut node = IksNode::new_tag("x");
        node.add_attribute("xmlns", XMLNS_DATA_FORMS);
        if let Some(ft) = &self.form_type {
            node.add_attribute("type", ft.as_str());
        }
        if let Some(title) = &self.title {
            let mut title_node = IksNode::new_tag("title");
            title_node.insert_cdata(title);
            node.add_child(title_node);
        }
        for inst in &self.instructions {
            let mut inst_node = IksNode::new_tag("instructions");
            inst_node.insert_cdata(inst);
            node.add_child(inst_node);
        }
        for field in &self.fields {
            node.add_child(field.to_node());
        }
        if !self.reported.is_empty() {
            let mut rep_node = IksNode::new_tag("reported");
            for field in &self.reported {
                rep_node.add_child(field.to_node());
            }
            node.add_child(rep_node);
        }
        for item in &self.items {
            let mut item_node = IksNode::new_tag("item");
            for field in item {
                item_node.add_child(field.to_node());
            }
            node.add_child(item_node);
        }
        node
    }

    pub fn from_node(node: &IksNode) -> Result<Self> {
        if node.name() != Some("x") || node.find_attrib("xmlns") != Some(XMLNS_DATA_FORMS) {
            return Err(IksError::BadXml);
        }
        let form_type = node
            .find_attrib("type")
            .and_then(|t| DataFormType::from_str(t).ok());
        let mut title = None;
        let mut instructions = Vec::new();
        let mut fields = Vec::new();
        let mut reported = Vec::new();
        let mut items = Vec::new();

        for child in node.children() {
            let c = child.borrow();
            match c.name() {
                Some("title") => {
                    title = Some(c.text());
                }
                Some("instructions") => {
                    instructions.push(c.text());
                }
                Some("field") => {
                    fields.push(FormField::from_node(&c)?);
                }
                Some("reported") => {
                    for rep_child in c.children() {
                        let rc = rep_child.borrow();
                        if rc.name() == Some("field") {
                            reported.push(FormField::from_node(&rc)?);
                        }
                    }
                }
                Some("item") => {
                    let mut item_fields = Vec::new();
                    for item_child in c.children() {
                        let ic = item_child.borrow();
                        if ic.name() == Some("field") {
                            item_fields.push(FormField::from_node(&ic)?);
                        }
                    }
                    items.push(item_fields);
                }
                _ => {}
            }
        }

        Ok(DataForm {
            form_type,
            title,
            instructions,
            fields,
            reported,
            items,
        })
    }

    /// Attaches this form as a child `<x>` of the given stanza.
    pub fn attach_to(&self, stanza: &mut IksNode) {
        stanza.add_child(self.to_node());
    }

    /// Extracts an XEP-0004 form from a stanza if present.
    pub fn extract_from(stanza: &IksNode) -> Option<Self> {
        for child in stanza.children() {
            let c = child.borrow();
            if c.name() == Some("x") && c.find_attrib("xmlns") == Some(XMLNS_DATA_FORMS) {
                if let Ok(form) = Self::from_node(&c) {
                    return Some(form);
                }
            }
        }
        None
    }
}

// ============================================================================
// XEP-0045: Multi-User Chat (MUC)
// ============================================================================

pub const XMLNS_MUC: &str = "http://jabber.org/protocol/muc";
pub const XMLNS_MUC_USER: &str = "http://jabber.org/protocol/muc#user";

/// Builds an XEP-0045 presence stanza to join a MUC room.
pub fn build_muc_join(
    to_room_nick: &str,
    password: Option<&str>,
    max_history_chars: Option<u32>,
) -> IksNode {
    let mut presence = IksNode::new_tag("presence");
    presence.add_attribute("to", to_room_nick);

    let mut x = IksNode::new_tag("x");
    x.add_attribute("xmlns", XMLNS_MUC);

    if let Some(pass) = password {
        let mut pass_node = IksNode::new_tag("password");
        pass_node.insert_cdata(pass);
        x.add_child(pass_node);
    }

    if let Some(max_chars) = max_history_chars {
        let mut history = IksNode::new_tag("history");
        history.add_attribute("maxchars", max_chars.to_string());
        x.add_child(history);
    }

    presence.add_child(x);
    presence
}

/// Builds an XEP-0045 presence stanza to leave a MUC room.
pub fn build_muc_leave(to_room_nick: &str, status: Option<&str>) -> IksNode {
    let mut presence = IksNode::new_tag("presence");
    presence.add_attribute("to", to_room_nick);
    presence.add_attribute("type", "unavailable");

    if let Some(stat) = status {
        let mut stat_node = IksNode::new_tag("status");
        stat_node.insert_cdata(stat);
        presence.add_child(stat_node);
    }

    presence
}

/// Checks whether a presence stanza contains XEP-0045 MUC information.
pub fn is_muc_presence(presence: &IksNode) -> bool {
    for child in presence.children() {
        let c = child.borrow();
        if c.name() == Some("x") {
            let ns = c.find_attrib("xmlns");
            if ns == Some(XMLNS_MUC) || ns == Some(XMLNS_MUC_USER) {
                return true;
            }
        }
    }
    false
}

/// Extracts MUC status codes (e.g. 110, 201, 307) from an incoming MUC presence stanza.
pub fn extract_muc_status_codes(presence: &IksNode) -> Vec<u16> {
    let mut codes = Vec::new();
    for child in presence.children() {
        let c = child.borrow();
        if c.name() == Some("x") && c.find_attrib("xmlns") == Some(XMLNS_MUC_USER) {
            for sub in c.children() {
                let sc = sub.borrow();
                if sc.name() == Some("status") {
                    if let Some(code_str) = sc.find_attrib("code") {
                        if let Ok(num) = code_str.parse::<u16>() {
                            codes.push(num);
                        }
                    }
                }
            }
        }
    }
    codes
}

// ============================================================================
// XEP-0060: Publish-Subscribe (PubSub)
// ============================================================================

pub const XMLNS_PUBSUB: &str = "http://jabber.org/protocol/pubsub";
pub const XMLNS_PUBSUB_EVENT: &str = "http://jabber.org/protocol/pubsub#event";

/// Represents an individual item in a PubSub node.
#[derive(Debug, Clone)]
pub struct PubSubItem {
    pub id: Option<String>,
    pub payload: Option<IksNode>,
}

/// Builds an XEP-0060 publish IQ stanza.
pub fn build_pubsub_publish(
    id: &str,
    to_service: &str,
    node: &str,
    item_id: Option<&str>,
    payload: Option<IksNode>,
) -> IksNode {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", id);
    iq.add_attribute("to", to_service);

    let mut pubsub = IksNode::new_tag("pubsub");
    pubsub.add_attribute("xmlns", XMLNS_PUBSUB);

    let mut publish = IksNode::new_tag("publish");
    publish.add_attribute("node", node);

    let mut item = IksNode::new_tag("item");
    if let Some(iid) = item_id {
        item.add_attribute("id", iid);
    }
    if let Some(pl) = payload {
        item.add_child(pl);
    }
    publish.add_child(item);

    pubsub.add_child(publish);
    iq.add_child(pubsub);
    iq
}

/// Builds an XEP-0060 subscribe IQ stanza.
pub fn build_pubsub_subscribe(id: &str, to_service: &str, node: &str, jid: &str) -> IksNode {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", id);
    iq.add_attribute("to", to_service);

    let mut pubsub = IksNode::new_tag("pubsub");
    pubsub.add_attribute("xmlns", XMLNS_PUBSUB);

    let mut subscribe = IksNode::new_tag("subscribe");
    subscribe.add_attribute("node", node);
    subscribe.add_attribute("jid", jid);

    pubsub.add_child(subscribe);
    iq.add_child(pubsub);
    iq
}

/// Builds an XEP-0060 unsubscribe IQ stanza.
pub fn build_pubsub_unsubscribe(id: &str, to_service: &str, node: &str, jid: &str) -> IksNode {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", id);
    iq.add_attribute("to", to_service);

    let mut pubsub = IksNode::new_tag("pubsub");
    pubsub.add_attribute("xmlns", XMLNS_PUBSUB);

    let mut unsub = IksNode::new_tag("unsubscribe");
    unsub.add_attribute("node", node);
    unsub.add_attribute("jid", jid);

    pubsub.add_child(unsub);
    iq.add_child(pubsub);
    iq
}

/// Extracts the PubSub node name and published items from an incoming `<message>` event notification.
pub fn extract_pubsub_items(message: &IksNode) -> Option<(String, Vec<PubSubItem>)> {
    let event = message.children().iter().find(|c| {
        let ref_c = c.borrow();
        ref_c.name() == Some("event") && ref_c.find_attrib("xmlns") == Some(XMLNS_PUBSUB_EVENT)
    })?;

    let items_node = event.borrow().find("items")?;
    let node_name = items_node.borrow().find_attrib("node")?.to_string();

    let mut items = Vec::new();
    for child in items_node.borrow().children() {
        let c = child.borrow();
        if c.name() == Some("item") {
            let item_id = c.find_attrib("id").map(|s| s.to_string());
            let payload = c.first_child_tag().map(|p| p.borrow().clone());
            items.push(PubSubItem {
                id: item_id,
                payload,
            });
        }
    }

    Some((node_name, items))
}

// ============================================================================
// XEP-0198: Stream Management
// ============================================================================

pub const XMLNS_STREAM_MANAGEMENT: &str = "urn:xmpp:sm:3";

/// Information received in an `<enabled>` stanza (XEP-0198 Section 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmEnabled {
    pub id: Option<String>,
    pub resume: bool,
    pub max: Option<u32>,
    pub location: Option<String>,
}

/// Information received in a `<resumed>` stanza (XEP-0198 Section 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmResumed {
    pub previd: String,
    pub h: u32,
}

/// State machine tracker for XEP-0198 stream management.
#[derive(Debug, Clone, Default)]
pub struct StreamManagementState {
    pub enabled: bool,
    pub sm_id: Option<String>,
    pub inbound_h: u32,
    pub outbound_h: u32,
    pub unacked_queue: std::collections::VecDeque<IksNode>,
}

impl StreamManagementState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Increments inbound handled stanzas count (wraps at 2^32).
    pub fn handle_inbound_stanza(&mut self) {
        self.inbound_h = self.inbound_h.wrapping_add(1);
    }

    /// Queues an outbound stanza and increments outbound count.
    pub fn queue_outbound_stanza(&mut self, stanza: IksNode) {
        self.outbound_h = self.outbound_h.wrapping_add(1);
        self.unacked_queue.push_back(stanza);
    }

    /// Acknowledges all outbound stanzas up to sequence number `h`.
    pub fn process_ack(&mut self, h: u32) {
        let _ = self.try_process_ack(h);
    }
    /// Reject impossible or stale acknowledgments without losing queued stanzas.
    pub fn try_process_ack(&mut self, h: u32) -> crate::Result<()> {
        let acknowledged = self
            .outbound_h
            .wrapping_sub(self.unacked_queue.len() as u32);
        let count = h.wrapping_sub(acknowledged) as usize;
        if count > self.unacked_queue.len() {
            return Err(crate::IksError::BadXml);
        }
        self.unacked_queue.drain(..count);
        Ok(())
    }

    /// Resets state on a fresh connection.
    pub fn reset(&mut self) {
        self.enabled = false;
        self.sm_id = None;
        self.inbound_h = 0;
        self.outbound_h = 0;
        self.unacked_queue.clear();
    }
}

/// Builds an XEP-0198 `<enable>` stanza to negotiate Stream Management.
pub fn build_sm_enable(resume: bool, max_seconds: Option<u32>) -> IksNode {
    let mut enable = IksNode::new_tag("enable");
    enable.add_attribute("xmlns", XMLNS_STREAM_MANAGEMENT);
    if resume {
        enable.add_attribute("resume", "true");
    }
    if let Some(max) = max_seconds {
        enable.add_attribute("max", max.to_string());
    }
    enable
}

/// Builds an XEP-0198 `<r>` stanza to request an acknowledgment.
pub fn build_sm_request_ack() -> IksNode {
    let mut r = IksNode::new_tag("r");
    r.add_attribute("xmlns", XMLNS_STREAM_MANAGEMENT);
    r
}

/// Builds an XEP-0198 `<a>` stanza to answer an acknowledgment request with handled count `h`.
pub fn build_sm_ack(h: u32) -> IksNode {
    let mut a = IksNode::new_tag("a");
    a.add_attribute("xmlns", XMLNS_STREAM_MANAGEMENT);
    a.add_attribute("h", h.to_string());
    a
}

/// Builds an XEP-0198 `<resume>` stanza to resume a previously disconnected session.
pub fn build_sm_resume(previd: &str, h: u32) -> IksNode {
    let mut resume = IksNode::new_tag("resume");
    resume.add_attribute("xmlns", XMLNS_STREAM_MANAGEMENT);
    resume.add_attribute("previd", previd);
    resume.add_attribute("h", h.to_string());
    resume
}

/// Checks whether an XML stanza belongs to XEP-0198 Stream Management.
pub fn is_sm_stanza(node: &IksNode) -> bool {
    node.find_attrib("xmlns") == Some(XMLNS_STREAM_MANAGEMENT)
}

/// Parses an `<a>` stanza's `h` handled count attribute.
pub fn parse_sm_ack(node: &IksNode) -> Option<u32> {
    if node.name() == Some("a") && node.find_attrib("xmlns") == Some(XMLNS_STREAM_MANAGEMENT) {
        node.find_attrib("h").and_then(|h| h.parse::<u32>().ok())
    } else {
        None
    }
}

/// Parses an `<enabled>` response from the server.
pub fn parse_sm_enabled(node: &IksNode) -> Option<SmEnabled> {
    if node.name() == Some("enabled") && node.find_attrib("xmlns") == Some(XMLNS_STREAM_MANAGEMENT)
    {
        let id = node.find_attrib("id").map(|s| s.to_string());
        let resume = node
            .find_attrib("resume")
            .map(|r| r == "true" || r == "1")
            .unwrap_or(false);
        let max = node.find_attrib("max").and_then(|m| m.parse::<u32>().ok());
        let location = node.find_attrib("location").map(|s| s.to_string());
        Some(SmEnabled {
            id,
            resume,
            max,
            location,
        })
    } else {
        None
    }
}

/// Parses an `<enabled>` response from the server (as `NodeRef`).
pub fn parse_sm_enabled_ref(node: &crate::NodeRef) -> Option<SmEnabled> {
    parse_sm_enabled(&node.borrow())
}

/// Parses a `<resumed>` response from the server.
pub fn parse_sm_resumed(node: &IksNode) -> Option<SmResumed> {
    if node.name() == Some("resumed") && node.find_attrib("xmlns") == Some(XMLNS_STREAM_MANAGEMENT)
    {
        let previd = node.find_attrib("previd")?.to_string();
        let h = node.find_attrib("h").and_then(|h| h.parse::<u32>().ok())?;
        Some(SmResumed { previd, h })
    } else {
        None
    }
}

// ============================================================================
// XEP-0280: Message Carbons
// ============================================================================

pub const XMLNS_CARBONS: &str = "urn:xmpp:carbons:2";
pub const XMLNS_FORWARD: &str = "urn:xmpp:forward:0";

/// Represents a received or sent carbon copy of a message.
#[derive(Debug, Clone)]
pub enum CarbonMessage {
    Received(IksNode),
    Sent(IksNode),
}

/// Builds an IQ stanza to enable Message Carbons on the current stream.
pub fn build_carbons_enable(id: &str) -> IksNode {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", id);
    let mut enable = IksNode::new_tag("enable");
    enable.add_attribute("xmlns", XMLNS_CARBONS);
    iq.add_child(enable);
    iq
}

/// Builds an IQ stanza to disable Message Carbons on the current stream.
pub fn build_carbons_disable(id: &str) -> IksNode {
    let mut iq = IksNode::new_tag("iq");
    iq.add_attribute("type", "set");
    iq.add_attribute("id", id);
    let mut disable = IksNode::new_tag("disable");
    disable.add_attribute("xmlns", XMLNS_CARBONS);
    iq.add_child(disable);
    iq
}

/// Marks a message as private so it is not carbon-copied to other bare-JID resources.
pub fn mark_carbon_private(message: &mut IksNode) {
    let mut private = IksNode::new_tag("private");
    private.add_attribute("xmlns", XMLNS_CARBONS);
    message.add_child(private);
}

/// Wraps an outgoing forwarded message in a `<sent>` carbon element.
pub fn wrap_carbon_sent(message: &IksNode) -> IksNode {
    let mut sent = IksNode::new_tag("sent");
    sent.add_attribute("xmlns", XMLNS_CARBONS);
    let mut forward = IksNode::new_tag("forwarded");
    forward.add_attribute("xmlns", XMLNS_FORWARD);
    forward.add_child(message.clone());
    sent.add_child(forward);
    sent
}

/// Wraps an incoming forwarded message in a `<received>` carbon element.
pub fn wrap_carbon_received(message: &IksNode) -> IksNode {
    let mut received = IksNode::new_tag("received");
    received.add_attribute("xmlns", XMLNS_CARBONS);
    let mut forward = IksNode::new_tag("forwarded");
    forward.add_attribute("xmlns", XMLNS_FORWARD);
    forward.add_child(message.clone());
    received.add_child(forward);
    received
}

/// Extracts a carbon copy (`<received>` or `<sent>`) from an incoming message stanza.
pub fn extract_carbon(message: &IksNode) -> Option<CarbonMessage> {
    for child in message.children() {
        let c = child.borrow();
        if c.find_attrib("xmlns") == Some(XMLNS_CARBONS) {
            let is_received = c.name() == Some("received");
            let is_sent = c.name() == Some("sent");
            if is_received || is_sent {
                for fwd in c.children() {
                    let f = fwd.borrow();
                    if f.name() == Some("forwarded")
                        && f.find_attrib("xmlns") == Some(XMLNS_FORWARD)
                    {
                        for inner in f.children() {
                            let in_node = inner.borrow();
                            if in_node.name() == Some("message") {
                                return if is_received {
                                    Some(CarbonMessage::Received(in_node.clone()))
                                } else {
                                    Some(CarbonMessage::Sent(in_node.clone()))
                                };
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

// ============================================================================
// XEP-0313: Message Archive Management (MAM) & XEP-0059: RSM
// ============================================================================

pub const XMLNS_MAM: &str = "urn:xmpp:mam:2";
pub const XMLNS_RSM: &str = "http://jabber.org/protocol/rsm";
pub const XMLNS_DELAY: &str = "urn:xmpp:delay";

/// Parameters for querying the message archive using MAM.
#[derive(Debug, Clone, Default)]
pub struct MamQuery {
    pub query_id: Option<String>,
    pub with: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub max: Option<u32>,
    pub after: Option<String>,
    pub before: Option<String>,
}

impl MamQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_query_id(mut self, qid: impl Into<String>) -> Self {
        self.query_id = Some(qid.into());
        self
    }

    pub fn with_jid(mut self, jid: impl Into<String>) -> Self {
        self.with = Some(jid.into());
        self
    }

    pub fn with_start(mut self, start: impl Into<String>) -> Self {
        self.start = Some(start.into());
        self
    }

    pub fn with_end(mut self, end: impl Into<String>) -> Self {
        self.end = Some(end.into());
        self
    }

    pub fn with_rsm_max(mut self, max: u32) -> Self {
        self.max = Some(max);
        self
    }

    pub fn with_rsm_after(mut self, after: impl Into<String>) -> Self {
        self.after = Some(after.into());
        self
    }

    pub fn with_rsm_before(mut self, before: impl Into<String>) -> Self {
        self.before = Some(before.into());
        self
    }

    /// Builds the `<iq type='set'>` query stanza for this MAM request.
    pub fn to_iq(&self, iq_id: &str) -> IksNode {
        let mut iq = IksNode::new_tag("iq");
        iq.add_attribute("type", "set");
        iq.add_attribute("id", iq_id);

        let mut query = IksNode::new_tag("query");
        query.add_attribute("xmlns", XMLNS_MAM);
        if let Some(qid) = &self.query_id {
            query.add_attribute("queryid", qid);
        }

        // Data Form (XEP-0004) for query parameters
        if self.with.is_some() || self.start.is_some() || self.end.is_some() {
            let mut form = DataForm::new(DataFormType::Submit);
            let mut form_type_field = FormField::new("FORM_TYPE");
            form_type_field.add_value(XMLNS_MAM);
            form.add_field(form_type_field);

            if let Some(w) = &self.with {
                let mut field = FormField::new("with");
                field.add_value(w);
                form.add_field(field);
            }
            if let Some(s) = &self.start {
                let mut field = FormField::new("start");
                field.add_value(s);
                form.add_field(field);
            }
            if let Some(e) = &self.end {
                let mut field = FormField::new("end");
                field.add_value(e);
                form.add_field(field);
            }
            query.add_child(form.to_node());
        }

        // RSM (XEP-0059) element for pagination
        if self.max.is_some() || self.after.is_some() || self.before.is_some() {
            let mut rsm = IksNode::new_tag("set");
            rsm.add_attribute("xmlns", XMLNS_RSM);
            if let Some(m) = self.max {
                let mut max_node = IksNode::new_tag("max");
                max_node.insert_cdata(m.to_string());
                rsm.add_child(max_node);
            }
            if let Some(after) = &self.after {
                let mut after_node = IksNode::new_tag("after");
                after_node.insert_cdata(after);
                rsm.add_child(after_node);
            }
            if let Some(before) = &self.before {
                let mut before_node = IksNode::new_tag("before");
                if !before.is_empty() {
                    before_node.insert_cdata(before);
                }
                rsm.add_child(before_node);
            }
            query.add_child(rsm);
        }

        iq.add_child(query);
        iq
    }
}

/// Represents an archived message item returned by MAM.
#[derive(Debug, Clone)]
pub struct MamResult {
    pub query_id: Option<String>,
    pub id: String,
    pub timestamp: Option<String>,
    pub message: IksNode,
}

/// Extracts a MAM archive result from an incoming `<message>` stanza.
pub fn extract_mam_result(message: &IksNode) -> Option<MamResult> {
    for child in message.children() {
        let c = child.borrow();
        if c.name() == Some("result") && c.find_attrib("xmlns") == Some(XMLNS_MAM) {
            let query_id = c.find_attrib("queryid").map(|s| s.to_string());
            let id = c.find_attrib("id")?.to_string();

            for fwd in c.children() {
                let f = fwd.borrow();
                if f.name() == Some("forwarded") && f.find_attrib("xmlns") == Some(XMLNS_FORWARD) {
                    let mut timestamp = None;
                    let mut inner_message = None;

                    for inner in f.children() {
                        let in_node = inner.borrow();
                        if in_node.name() == Some("delay")
                            && in_node.find_attrib("xmlns") == Some(XMLNS_DELAY)
                        {
                            timestamp = in_node.find_attrib("stamp").map(|s| s.to_string());
                        } else if in_node.name() == Some("message") {
                            inner_message = Some(in_node.clone());
                        }
                    }

                    if let Some(msg) = inner_message {
                        return Some(MamResult {
                            query_id,
                            id,
                            timestamp,
                            message: msg,
                        });
                    }
                }
            }
        }
    }
    None
}

/// Result of completing a MAM query (`<fin>` stanza).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MamFin {
    pub complete: bool,
    pub first: Option<String>,
    pub last: Option<String>,
    pub count: Option<u32>,
}

/// Parses a MAM `<fin>` completion IQ stanza.
pub fn parse_mam_fin(iq: &IksNode) -> Option<MamFin> {
    let fin_rc = iq.children().iter().find(|c| {
        let b = c.borrow();
        b.name() == Some("fin") && b.find_attrib("xmlns") == Some(XMLNS_MAM)
    })?;
    let fin = fin_rc.borrow();

    let complete = fin
        .find_attrib("complete")
        .map(|c| c == "true" || c == "1")
        .unwrap_or(false);

    let mut first = None;
    let mut last = None;
    let mut count = None;

    if let Some(rsm_rc) = fin.children().iter().find(|c| {
        let b = c.borrow();
        b.name() == Some("set") && b.find_attrib("xmlns") == Some(XMLNS_RSM)
    }) {
        let rsm = rsm_rc.borrow();
        for child in rsm.children() {
            let b = child.borrow();
            match b.name() {
                Some("first") => first = Some(b.text()),
                Some("last") => last = Some(b.text()),
                Some("count") => count = b.text().parse::<u32>().ok(),
                _ => {}
            }
        }
    }

    Some(MamFin {
        complete,
        first,
        last,
        count,
    })
}
