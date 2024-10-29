use std::rc::Rc;
use std::cell::RefCell;
use crate::{IksError, IksNode, Result, TagType, SaxHandler};

/// DOM parser that builds a tree structure from SAX events
pub struct DomParser {
    root: Option<Rc<RefCell<IksNode>>>,
    node_stack: Vec<Rc<RefCell<IksNode>>>,
}

impl DomParser {
    /// Create a new DOM parser
    pub fn new() -> Self {
        DomParser {
            root: None,
            node_stack: Vec::new(),
        }
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
                self.node_stack.pop();
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
