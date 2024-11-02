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

mod parser;
mod dom;
mod ikstack;
mod utility;
mod constants;
mod helper;

use std::fmt;
use thiserror::Error;
use std::rc::{Rc, Weak};
use std::cell::RefCell;

pub use parser::{Parser, SaxHandler};
pub use dom::DomParser;
pub use utility::{str_dup, str_cat, str_casecmp, str_len, escape, unescape, set_mem_funcs};
pub use constants::{memory, xml};
pub use helper::{align_size, calculate_chunk_growth, escape_size, unescape_size};

/// XML node types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IksType {
    None,
    Tag,
    Attribute,
    CData,
}

/// XML tag types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    Open,
    Close,
    Single,
}

/// Parser error types
#[derive(Error, Debug)]
pub enum IksError {
    #[error("Out of memory")]
    NoMem,
    #[error("Invalid XML")]
    BadXml,
    #[error("Hook returned error")]
    Hook,
    #[error("Network DNS error")]
    NetNoDns,
    #[error("Network socket error")]
    NetNoSock,
    #[error("Network connection error")]
    NetNoConn,
    #[error("Network read/write error")]
    NetRwErr,
    #[error("Network operation not supported")]
    NetNotSupp,
    #[error("TLS operation failed")]
    NetTlsFail,
    #[error("Network connection dropped")]
    NetDropped,
    #[error("Unknown network error")]
    NetUnknown,
    #[error("File not found")]
    FileNoFile,
    #[error("File access denied")]
    FileNoAccess,
    #[error("File read/write error")]
    FileRwErr,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for iksemel operations
pub type Result<T> = std::result::Result<T, IksError>;

/// XML Node structure
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
}

impl IksNode {
    /// Create a new XML node
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
        }
    }

    /// Create a new tag node with name
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
        }
    }

    /// Get parent node
    pub fn parent(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.parent.as_ref().and_then(|w| w.upgrade())
    }

    /// Get next sibling
    pub fn next(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.next.clone()
    }

    /// Get previous sibling
    pub fn prev(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.prev.as_ref().and_then(|w| w.upgrade())
    }

    /// Get next sibling tag
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

    /// Find first child with given tag name
    pub fn find(&self, name: &str) -> Option<Rc<RefCell<IksNode>>> {
        self.children.iter()
            .find(|child| {
                let child = child.borrow();
                child.node_type == IksType::Tag && 
                child.name.as_ref().map_or(false, |n| n == name)
            })
            .cloned()
    }

    /// Find first child's CDATA content with given tag name
    pub fn find_cdata(&self, name: &str) -> Option<String> {
        self.find(name).and_then(|node| {
            node.borrow().children.iter()
                .find(|child| child.borrow().node_type == IksType::CData)
                .and_then(|cdata| cdata.borrow().content.clone())
        })
    }

    /// Add a child node
    pub fn add_child(&mut self, child: IksNode) -> Rc<RefCell<IksNode>> {
        let child_rc = Rc::new(RefCell::new(child));
        
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

    /// Insert a new tag node as a sibling
    pub fn insert_sibling<S: Into<String>>(&mut self, name: S) -> IksNode {
        let mut node = IksNode::new_tag(name);
        if let Some(parent) = &self.parent {
            if let Some(parent_rc) = parent.upgrade() {
                node.parent = Some(Rc::downgrade(&parent_rc));
            }
        }
        node
    }

    /// Insert CDATA content
    pub fn insert_cdata<S: Into<String>>(&mut self, data: S) -> Rc<RefCell<IksNode>> {
        let mut cdata = IksNode::new(IksType::CData);
        cdata.set_content(data);
        self.add_child(cdata)
    }

    /// Add an attribute
    pub fn add_attribute<S: Into<String>>(&mut self, name: S, value: S) {
        self.attributes.push((name.into(), value.into()));
    }

    /// Set node content
    pub fn set_content<S: Into<String>>(&mut self, content: S) {
        self.content = Some(content.into());
    }

    /// Insert a new tag node before this node
    pub fn insert_before<S: Into<String>>(&mut self, name: S) -> IksNode {
        let mut node = IksNode::new_tag(name);
        if let Some(parent) = &self.parent {
            node.parent = Some(parent.clone());
        }
        node
    }

    /// Find attribute value by name
    pub fn find_attrib(&self, name: &str) -> Option<&str> {
        self.attributes.iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Find first child with given attribute name and value
    pub fn find_with_attrib(&self, tag_name: Option<&str>, attr_name: &str, value: &str) -> Option<Rc<RefCell<IksNode>>> {
        self.children.iter()
            .find(|child| {
                let child = child.borrow();
                if child.node_type != IksType::Tag {
                    return false;
                }
                if let Some(name) = tag_name {
                    if child.name.as_ref().map_or(true, |n| n != name) {
                        return false;
                    }
                }
                child.find_attrib(attr_name) == Some(value)
            })
            .cloned()
    }

    /// Get first child tag
    pub fn first_tag(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.children.iter()
            .find(|child| child.borrow().node_type == IksType::Tag)
            .cloned()
    }

    /// Check if node has children
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Check if node has attributes
    pub fn has_attributes(&self) -> bool {
        !self.attributes.is_empty()
    }

    /// Get this node as an Rc if it's part of a tree
    fn as_rc(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.parent.as_ref()
            .and_then(|w| w.upgrade())
            .map(|p| {
                p.borrow().children.iter()
                    .find(|c| Rc::ptr_eq(c, &p))
                    .cloned()
            })
            .flatten()
    }
}

impl Clone for IksNode {
    fn clone(&self) -> Self {
        IksNode {
            node_type: self.node_type,
            name: self.name.clone(),
            content: self.content.clone(),
            attributes: self.attributes.clone(),
            children: Vec::new(), // Don't clone children to avoid cycles
            parent: None,
            next: None,
            prev: None,
        }
    }
}

impl fmt::Display for IksNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.node_type {
            IksType::Tag => {
                write!(f, "<{}", self.name.as_ref().unwrap())?;
                
                // Write attributes
                for (name, value) in &self.attributes {
                    write!(f, " {}=\"{}\"", name, escape_attr(value))?;
                }

                if self.children.is_empty() && self.content.is_none() {
                    write!(f, "/>")?;
                } else {
                    write!(f, ">")?;
                    
                    // Write content if any
                    if let Some(content) = &self.content {
                        write!(f, "{}", escape_text(content))?;
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
                    write!(f, "{}", escape_text(content))?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Escape special XML characters in attribute values
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape special XML characters in text content
fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
        let root = Rc::new(RefCell::new(IksNode::new_tag("root")));
        
        let mut child1 = IksNode::new_tag("child1");
        child1.add_attribute("id", "1");
        root.borrow_mut().add_child(child1);
        
        let mut child2 = IksNode::new_tag("child2");
        child2.add_attribute("id", "2");
        root.borrow_mut().add_child(child2);
        
        // Test find methods
        let found = root.borrow().find("child1").unwrap();
        assert_eq!(found.borrow().name.as_ref().unwrap(), "child1");
        
        let found = root.borrow().find_with_attrib(None, "id", "2").unwrap();
        assert_eq!(found.borrow().name.as_ref().unwrap(), "child2");
        
        // Test navigation
        {
            let root_ref = root.borrow();
            let children = &root_ref.children;
            
            let first = &children[0];
            assert_eq!(first.borrow().name.as_ref().unwrap(), "child1");
            
            let second = &children[1];
            assert_eq!(second.borrow().name.as_ref().unwrap(), "child2");
        }
    }

    #[test]
    fn test_cdata_handling() {
        let root = Rc::new(RefCell::new(IksNode::new_tag("root")));
        
        let mut child = IksNode::new_tag("child");
        let cdata = child.insert_cdata("Hello World");
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
} 