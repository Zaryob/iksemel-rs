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

    
}
