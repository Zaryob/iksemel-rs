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

use crate::{IksError, IksNode, IksType, Parser, Result, SaxHandler, TagType};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// Represents events emitted by the XMPP stream parser.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// The root stream header was opened (e.g. `<stream:stream ...>`).
    StreamStart(IksNode),
    /// A complete top-level XMPP stanza (`<iq>`, `<message>`, `<presence>`, `<features>`, etc.).
    Stanza(IksNode),
    /// The root stream was closed (`</stream:stream>`).
    StreamEnd,
}

struct StreamDispatcher {
    events: VecDeque<StreamEvent>,
    depth: usize,
    stream_root_name: Option<String>,
    stanza_stack: Vec<Rc<RefCell<IksNode>>>,
    current_stanza_root: Option<Rc<RefCell<IksNode>>>,
}

impl StreamDispatcher {
    fn new() -> Self {
        StreamDispatcher {
            events: VecDeque::new(),
            depth: 0,
            stream_root_name: None,
            stanza_stack: Vec::new(),
            current_stanza_root: None,
        }
    }

    fn reset(&mut self) {
        self.events.clear();
        self.depth = 0;
        self.stream_root_name = None;
        self.stanza_stack.clear();
        self.current_stanza_root = None;
    }
}

impl SaxHandler for StreamDispatcher {
    fn on_tag(
        &mut self,
        name: &str,
        attributes: &[(String, String)],
        tag_type: TagType,
    ) -> Result<()> {
        match tag_type {
            TagType::Open => {
                if self.depth == 0 {
                    // Root stream element (e.g. <stream:stream>)
                    let mut node = IksNode::new_tag(name);
                    for (k, v) in attributes {
                        node.add_attribute(k, v);
                    }
                    self.stream_root_name = Some(name.to_string());
                    self.depth = 1;
                    self.events.push_back(StreamEvent::StreamStart(node));
                } else if self.depth == 1 {
                    // Start of a top-level stanza (e.g. <message>, <iq>, <presence>)
                    let mut node = IksNode::new_tag(name);
                    for (k, v) in attributes {
                        node.add_attribute(k, v);
                    }
                    let node_rc = node.into_rc();
                    self.current_stanza_root = Some(node_rc.clone());
                    self.stanza_stack.push(node_rc);
                    self.depth += 1;
                } else {
                    // Child element inside a stanza
                    let mut node = IksNode::new_tag(name);
                    for (k, v) in attributes {
                        node.add_attribute(k, v);
                    }
                    let node_rc = if let Some(parent) = self.stanza_stack.last() {
                        parent.borrow_mut().add_child(node)
                    } else {
                        node.into_rc()
                    };
                    self.stanza_stack.push(node_rc);
                    self.depth += 1;
                }
            }
            TagType::Single => {
                if self.depth == 0 {
                    // Self-closing root stream tag (unusual, but valid XML)
                    let mut node = IksNode::new_tag(name);
                    for (k, v) in attributes {
                        node.add_attribute(k, v);
                    }
                    self.events
                        .push_back(StreamEvent::StreamStart(node.clone()));
                    self.events.push_back(StreamEvent::StreamEnd);
                } else if self.depth == 1 {
                    // Self-closing top-level stanza (e.g. <presence/>)
                    let mut node = IksNode::new_tag(name);
                    for (k, v) in attributes {
                        node.add_attribute(k, v);
                    }
                    self.events.push_back(StreamEvent::Stanza(node));
                } else {
                    // Self-closing child inside stanza
                    let mut node = IksNode::new_tag(name);
                    for (k, v) in attributes {
                        node.add_attribute(k, v);
                    }
                    if let Some(parent) = self.stanza_stack.last() {
                        let _ = parent.borrow_mut().add_child(node);
                    }
                }
            }
            TagType::Close => {
                if self.depth == 1 {
                    // Closing of root stream element (e.g. </stream:stream>)
                    if self.stream_root_name.as_deref() == Some(name) {
                        self.depth = 0;
                        self.stream_root_name = None;
                        self.events.push_back(StreamEvent::StreamEnd);
                    } else {
                        return Err(IksError::BadXml);
                    }
                } else if self.depth > 1 {
                    // Closing a stanza element or sub-element
                    if let Some(popped) = self.stanza_stack.pop() {
                        if popped.borrow().name() != Some(name) {
                            return Err(IksError::BadXml);
                        }
                    }
                    self.depth -= 1;

                    // If we just popped the top-level stanza, emit it!
                    if self.depth == 1 {
                        if let Some(root_rc) = self.current_stanza_root.take() {
                            let stanza = root_rc.borrow().clone();
                            self.events.push_back(StreamEvent::Stanza(stanza));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn on_cdata(&mut self, data: &str) -> Result<()> {
        if self.depth > 1 {
            if let Some(parent) = self.stanza_stack.last() {
                let mut p = parent.borrow_mut();
                if let Some(last_child) = p.children().last() {
                    if last_child.borrow().node_type() == IksType::CData {
                        let mut lc = last_child.borrow_mut();
                        let mut combined = lc.content().unwrap_or("").to_string();
                        combined.push_str(data);
                        lc.set_content(combined);
                        return Ok(());
                    }
                }
                if !data.trim().is_empty() {
                    let mut cdata = IksNode::new(IksType::CData);
                    cdata.set_content(data);
                    p.add_child(cdata);
                }
            }
        }
        Ok(())
    }
}

/// An incremental XMPP stream parser.
///
/// XMPP streams consist of an unclosed root `<stream:stream>` element
/// containing sequential top-level XML stanzas.
pub struct StreamParser {
    parser: Parser<StreamDispatcher>,
}

impl StreamParser {
    /// Creates a new `StreamParser`.
    pub fn new() -> Self {
        StreamParser {
            parser: Parser::new(StreamDispatcher::new()),
        }
    }

    /// Resets the stream parser state (useful after TLS negotiation or SASL restart).
    pub fn reset(&mut self) {
        self.parser.reset();
        self.parser.handler_mut().reset();
    }

    /// Parses a chunk of incoming streaming XML data and returns any completed events.
    pub fn parse_chunk(&mut self, data: &str) -> Result<Vec<StreamEvent>> {
        self.parser.parse(data)?;
        let collected: Vec<StreamEvent> = self.parser.handler_mut().events.drain(..).collect();
        Ok(collected)
    }

    /// Checks if there are any pending parsed events.
    pub fn has_events(&self) -> bool {
        !self.parser.handler().events.is_empty()
    }

    /// Pops the next completed event from the parser's queue.
    pub fn pop_event(&mut self) -> Option<StreamEvent> {
        self.parser.handler_mut().events.pop_front()
    }
}

impl Default for StreamParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_parsing_basic() {
        let mut parser = StreamParser::new();

        // 1. Open stream
        let events = parser.parse_chunk("<stream:stream to='example.com' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams'>").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::StreamStart(node) => {
                assert_eq!(node.name(), Some("stream:stream"));
                assert_eq!(node.find_attrib("to"), Some("example.com"));
            }
            _ => panic!("Expected StreamStart"),
        }

        // 2. Feed a stanza in chunks
        let events = parser
            .parse_chunk("<message to='bob@example.com'>")
            .unwrap();
        assert_eq!(events.len(), 0);

        let events = parser
            .parse_chunk("<body>Hello Bob!</body></message>")
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Stanza(node) => {
                assert_eq!(node.name(), Some("message"));
                assert_eq!(node.find_attrib("to"), Some("bob@example.com"));
                assert_eq!(node.find_cdata("body"), Some("Hello Bob!".to_string()));
            }
            _ => panic!("Expected Stanza"),
        }

        // 3. Feed a single self-closing presence
        let events = parser.parse_chunk("<presence/>").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Stanza(node) => {
                assert_eq!(node.name(), Some("presence"));
            }
            _ => panic!("Expected Stanza"),
        }

        // 4. Close stream
        let events = parser.parse_chunk("</stream:stream>").unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::StreamEnd => {}
            _ => panic!("Expected StreamEnd"),
        }
    }

    #[test]
    fn test_stream_reset() {
        let mut parser = StreamParser::new();
        let _ = parser
            .parse_chunk("<stream:stream to='example.com'>")
            .unwrap();
        parser.reset();

        let events = parser
            .parse_chunk("<stream:stream to='new.example.com'>")
            .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::StreamStart(node) => {
                assert_eq!(node.find_attrib("to"), Some("new.example.com"));
            }
            _ => panic!("Expected StreamStart"),
        }
    }
}
