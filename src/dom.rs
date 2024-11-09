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

use std::rc::Rc;
use std::cell::RefCell;
use crate::{IksError, IksNode, Result, TagType, SaxHandler};
use crate::constants::memory;

/// DOM parser that builds a tree structure from SAX events.
/// 
/// This parser implements the `SaxHandler` trait to build a complete DOM tree
/// from XML parsing events. It maintains parent-child relationships and
/// handles all XML node types.
/// 
/// # Examples
/// 
/// ```
/// use iksemel::{DomParser, IksNode};
/// 
/// // Parse XML string into DOM
/// let xml = r#"<root><child>Hello World</child></root>"#;
/// let dom = DomParser::parse_str(xml).unwrap();
/// 
/// // Access the DOM tree
/// let root = dom.borrow();
/// if let Some(content) = root.find_cdata("child") {
///     assert_eq!(content, "Hello World");
/// }
/// ```
pub struct DomParser {
    root: Option<Rc<RefCell<IksNode>>>,
    node_stack: Vec<Rc<RefCell<IksNode>>>,
    chunk_size: usize,
}

impl DomParser {
    /// Creates a new DOM parser.
    /// 
    /// # Returns
    /// 
    /// A new `DomParser` instance
    pub fn new() -> Result<Self> {
        Ok(DomParser {
            root: None,
            node_stack: Vec::new(),
            chunk_size: memory::DEFAULT_IKS_CHUNK_SIZE,
        })
    }

    /// Sets a size hint for better memory allocation.
    /// 
    /// This method can be used to optimize memory allocation based on
    /// the expected size of the XML document.
    /// 
    /// # Arguments
    /// 
    /// * `approx_size` - Approximate size of the XML document in bytes
    pub fn set_size_hint(&mut self, approx_size: usize) {
        let cs = approx_size / 10;
        self.chunk_size = cs.max(memory::DEFAULT_IKS_CHUNK_SIZE);
    }

    /// Gets the parsed document root node.
    /// 
    /// # Returns
    /// 
    /// An `Option` containing the root node if the document has been parsed
    pub fn document(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.root.clone()
    }

    /// Parses an XML string into a DOM tree.
    /// 
    /// This is a convenience method that creates a new parser, parses the
    /// input string, and returns the root node of the resulting DOM tree.
    /// 
    /// # Arguments
    /// 
    /// * `xml` - The XML string to parse
    /// 
    /// # Returns
    /// 
    /// A `Result` containing the root node of the DOM tree
    pub fn parse_str(xml: &str) -> Result<Rc<RefCell<IksNode>>> {
        let mut parser = DomParser::new()?;
        let mut sax_parser = crate::Parser::new(parser);
        sax_parser.parse(xml)?;
        
        // Get the root node from the parser's handler
        sax_parser.handler().document().ok_or(IksError::BadXml)
    }

    /// Loads and parses an XML file into a DOM tree.
    /// 
    /// This is a convenience method that reads a file and parses its contents
    /// into a DOM tree.
    /// 
    /// # Arguments
    /// 
    /// * `path` - Path to the XML file to parse
    /// 
    /// # Returns
    /// 
    /// A `Result` containing the root node of the DOM tree
    pub fn load_file(path: &str) -> Result<Rc<RefCell<IksNode>>> {
        let xml = std::fs::read_to_string(path)?;
        Self::parse_str(&xml)
    }

    /// Saves a DOM tree to an XML file.
    /// 
    /// This method serializes the DOM tree to XML and writes it to a file.
    /// 
    /// # Arguments
    /// 
    /// * `node` - The root node of the DOM tree to save
    /// * `path` - Path where the XML file should be written
    /// 
    /// # Returns
    /// 
    /// A `Result` indicating success or failure
    pub fn save_file(node: &Rc<RefCell<IksNode>>, path: &str) -> Result<()> {
        let xml = node.borrow().to_string();
        std::fs::write(path, xml)?;
        Ok(())
    }
}

impl SaxHandler for DomParser {
    /// Handles tag events during parsing.
    /// 
    /// This method creates new nodes for tags and maintains the parent-child
    /// relationships in the DOM tree.
    /// 
    /// # Arguments
    /// 
    /// * `name` - The name of the tag
    /// * `attributes` - Vector of (name, value) pairs for the tag's attributes
    /// * `tag_type` - The type of tag (open, close, or single)
    /// 
    /// # Returns
    /// 
    /// A `Result` indicating success or failure
    fn on_tag(&mut self, name: &str, attributes: &[(String, String)], tag_type: TagType) -> Result<()> {
        match tag_type {
            TagType::Open | TagType::Single => {
                let mut node = IksNode::new_tag(name);
                
                // Pre-allocate attributes vector with capacity
                node.attributes.reserve(attributes.len());
                
                // Add attributes efficiently
                for (attr, value) in attributes {
                    node.add_attribute(attr, value);
                }
                
                let node_rc = Rc::new(RefCell::new(node));

                if let Some(parent_rc) = self.node_stack.last() {
                    node_rc.borrow_mut().parent = Some(Rc::downgrade(parent_rc));
                    parent_rc.borrow_mut().children.push(node_rc.clone());
                    if tag_type == TagType::Open {
                        self.node_stack.push(node_rc);
                    }
                } else {
                    self.root = Some(node_rc.clone());
                    if tag_type == TagType::Open {
                        self.node_stack.push(node_rc);
                    }
                }
            },
            TagType::Close => {
                if let Some(current) = self.node_stack.last() {
                    if current.borrow().name.as_ref().map_or(false, |n| n == name) {
                        self.node_stack.pop();
                    } else {
                        // Only return error if we're not at the root level
                        if !self.node_stack.is_empty() {
                            return Err(IksError::BadXml);
                        }
                    }
                }
            },
        }
        Ok(())
    }
    
    /// Handles character data events during parsing.
    /// 
    /// This method creates text nodes for character data and adds them to
    /// the current parent node.
    /// 
    /// # Arguments
    /// 
    /// * `data` - The character data encountered
    /// 
    /// # Returns
    /// 
    /// A `Result` indicating success or failure
    fn on_cdata(&mut self, data: &str) -> Result<()> {
        if let Some(parent) = self.node_stack.last() {
            if !data.trim().is_empty() {
                let mut cdata = IksNode::new(crate::IksType::CData);
                cdata.set_content(data);
                parent.borrow_mut().add_child(cdata);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dom_child(){
        let xml = r#"
            <root>
                <child id="3"/>
            </root>"#;

        let dom = DomParser::parse_str(xml).unwrap();
        let root = dom.borrow();
        
        assert_eq!(root.name.as_ref().unwrap(), "root");
        assert_eq!(root.children.len(), 1);
        
        let child = root.children[0].borrow();
        assert_eq!(child.name.as_ref().unwrap(), "child");
        assert_eq!(child.attributes[0], ("id".to_string(), "3".to_string()));
        assert!(child.children.is_empty());
    }    

    #[test]
    fn test_dom_parsing() {
        let xml = r#"
            <root version="1.0">
                <child id="1">Text1</child>
                <child id="2">Text2</child>
                <child id="3"/>
            </root>"#;
            
        let dom = DomParser::parse_str(xml).unwrap();
        let root = dom.borrow();
        
        assert_eq!(root.name.as_ref().unwrap(), "root");
        assert_eq!(root.attributes[0], ("version".to_string(), "1.0".to_string()));
        assert_eq!(root.children.len(), 3);
        
        let child1 = root.children[0].borrow();
        assert_eq!(child1.name.as_ref().unwrap(), "child");
        assert_eq!(child1.attributes[0], ("id".to_string(), "1".to_string()));
        
        // Check CDATA content
        let text = child1.children.first().unwrap();
        assert_eq!(text.borrow().content.as_ref().unwrap(), "Text1");
        
        let child2 = root.children[1].borrow();
        assert_eq!(child2.name.as_ref().unwrap(), "child");
        assert_eq!(child2.attributes[0], ("id".to_string(), "2".to_string()));
        assert_eq!(child2.children.first().unwrap().borrow().content.as_ref().unwrap(), "Text2");
        
        let child3 = root.children[2].borrow();
        assert_eq!(child3.name.as_ref().unwrap(), "child");
        assert_eq!(child3.attributes[0], ("id".to_string(), "3".to_string()));
        assert!(child3.children.is_empty());
    }
    
    #[test]
    fn test_file_operations() -> Result<()> {
        let root = Rc::new(RefCell::new(IksNode::new_tag("root")));
        root.borrow_mut().add_attribute("version", "1.0");
        
        let mut child = IksNode::new_tag("child");
        let mut cdata = IksNode::new(crate::IksType::CData);
        cdata.set_content("Hello World");
        child.add_child(cdata);
        root.borrow_mut().add_child(child);
        
        // Save to file
        let temp_path = std::env::temp_dir().join("test.xml");
        DomParser::save_file(&root, temp_path.to_str().unwrap())?;
        
        // Load from file
        let loaded = DomParser::load_file(temp_path.to_str().unwrap())?;
        
        // Compare the XML strings
        let root_xml = root.borrow().to_string();
        let loaded_xml = loaded.borrow().to_string();
        assert_eq!(root_xml, loaded_xml);
        
        // Clean up the temporary file
        std::fs::remove_file(temp_path)?;
        
        Ok(())
    }
} 