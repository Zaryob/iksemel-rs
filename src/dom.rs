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

/// DOM parser that builds a tree structure from SAX events
pub struct DomParser {
    root: Option<Rc<RefCell<IksNode>>>,
    node_stack: Vec<Rc<RefCell<IksNode>>>,
    chunk_size: usize,
}

impl DomParser {
    /// Create a new DOM parser
    pub fn new() -> Self {
        DomParser {
            root: None,
            node_stack: Vec::new(),
            chunk_size: memory::DEFAULT_IKS_CHUNK_SIZE,
        }
    }

    /// Set size hint for better memory allocation
    pub fn set_size_hint(&mut self, approx_size: usize) {
        let cs = approx_size / 10;
        self.chunk_size = cs.max(memory::DEFAULT_IKS_CHUNK_SIZE);
    }

    /// Get the parsed document root node
    pub fn document(&self) -> Option<Rc<RefCell<IksNode>>> {
        self.root.clone()
    }

    /// Parse an XML string into a DOM tree
    pub fn parse_str(xml: &str) -> Result<Rc<RefCell<IksNode>>> {
        let mut parser = DomParser::new();
        let mut sax_parser = crate::Parser::new(parser);
        sax_parser.parse(xml)?;
        
        // Get the root node from the parser's handler
        sax_parser.handler().document().ok_or(IksError::BadXml)
    }

    /// Load and parse an XML file into a DOM tree
    pub fn load_file(path: &str) -> Result<Rc<RefCell<IksNode>>> {
        let xml = std::fs::read_to_string(path)?;
        Self::parse_str(&xml)
    }

    /// Save a DOM tree to an XML file
    pub fn save_file(node: &Rc<RefCell<IksNode>>, path: &str) -> Result<()> {
        let xml = node.borrow().to_string();
        std::fs::write(path, xml)?;
        Ok(())
    }
}

impl SaxHandler for DomParser {
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
                        return Err(IksError::BadXml);
                    }
                }
            },
        }
        Ok(())
    }
    
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
        
        assert_eq!(
            root.borrow().to_string(),
            loaded.borrow().to_string()
        );
        
        Ok(())
    }

    #[test]
    fn test_size_hint() {
        let mut parser = DomParser::new();
        parser.set_size_hint(10000);
        assert!(parser.chunk_size >= memory::DEFAULT_IKS_CHUNK_SIZE);
    }
} 