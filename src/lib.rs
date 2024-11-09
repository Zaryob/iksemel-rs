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
        }
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
        self.children.iter()
            .find(|child| {
                let child = child.borrow();
                child.node_type == IksType::Tag && 
                child.name.as_ref().map_or(false, |n| n == name)
            })
            .cloned()
    }

    /// Finds the first child's CDATA content with the specified tag name.
    /// 
    /// # Arguments
    /// 
    /// * `name` - The name of the tag to find
    /// 
    /// # Returns
    /// 
    /// An `Option` containing the CDATA content if found
    pub fn find_cdata(&self, name: &str) -> Option<String> {
        self.find(name).and_then(|node| {
            node.borrow().children.iter()
                .find(|child| child.borrow().node_type == IksType::CData)
                .and_then(|cdata| cdata.borrow().content.clone())
        })
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

    /// Inserts a new tag node as a sibling.
    /// 
    /// # Arguments
    /// 
    /// * `name` - The name of the new tag
    /// 
    /// # Returns
    /// 
    /// The newly created tag node
    pub fn insert_sibling<S: Into<String>>(&mut self, name: S) -> IksNode {
        let mut node = IksNode::new_tag(name);
        if let Some(parent) = &self.parent {
            if let Some(parent_rc) = parent.upgrade() {
                node.parent = Some(Rc::downgrade(&parent_rc));
            }
        }
        node
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

    /// Adds an attribute to this node.
    /// 
    /// # Arguments
    /// 
    /// * `name` - The name of the attribute
    /// * `value` - The value of the attribute
    pub fn add_attribute<S: Into<String>>(&mut self, name: S, value: S) {
        self.attributes.push((name.into(), value.into()));
    }

    /// Sets the content of this node.
    /// 
    /// # Arguments
    /// 
    /// * `content` - The content to set
    pub fn set_content<S: Into<String>>(&mut self, content: S) {
        self.content = Some(content.into());
    }

    /// Inserts a new tag node before this node.
    /// 
    /// # Arguments
    /// 
    /// * `name` - The name of the new tag
    /// 
    /// # Returns
    /// 
    /// The newly created tag node
    pub fn insert_before<S: Into<String>>(&mut self, name: S) -> IksNode {
        let mut node = IksNode::new_tag(name);
        if let Some(parent) = &self.parent {
            node.parent = Some(parent.clone());
        }
        node
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
        self.attributes.iter()
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

    /// Gets the first child tag node.
    /// 
    /// # Returns
    /// 
    /// An `Option` containing the first child tag node if it exists
    pub fn first_tag(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.children.iter()
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

    /// Gets this node as an Rc if it's part of a tree.
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