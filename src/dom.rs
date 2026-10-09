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

use crate::constants::memory;
use crate::{IksError, IksNode, NodeRef, Result, SaxHandler, TagType};
use std::cell::RefCell;
use std::rc::Rc;

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
    /// C: `*iksptr` — yalnızca kök element **kapandığında** yazılır.
    root: Option<Rc<RefCell<IksNode>>>,
    /// C: `data->current` — hâlen açık olan en içteki düğüm.
    current: Option<Rc<RefCell<IksNode>>>,
    /// Açık olan kökü güçlü referansla tutar (çocuklar parent'a Weak tuttuğu için).
    open_root: Option<Rc<RefCell<IksNode>>>,
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
            current: None,
            open_root: None,
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
    pub fn document(&self) -> Option<NodeRef> {
        self.root.clone().map(NodeRef)
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
    pub fn parse_str(xml: &str) -> Result<NodeRef> {
        let parser = DomParser::new()?;
        let mut sax_parser = crate::Parser::new(parser);
        sax_parser.parse(xml)?;
        sax_parser.finish()?;
        sax_parser.handler().document().ok_or(IksError::BadXml)
    }

    /// Parses an XML string into a DOM tree with custom security limits.
    pub fn parse_str_with_limits(xml: &str, limits: crate::ParserLimits) -> Result<NodeRef> {
        let parser = DomParser::new()?;
        let mut sax_parser = crate::Parser::with_limits(parser, limits);
        sax_parser.parse(xml)?;
        sax_parser.finish()?;
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
    pub fn load_file(path: &str) -> Result<NodeRef> {
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
    pub fn save_file(node: &NodeRef, path: &str) -> Result<()> {
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
    fn on_tag(
        &mut self,
        name: &str,
        attributes: &[(String, String)],
        tag_type: TagType,
    ) -> Result<()> {
        match tag_type {
            TagType::Open | TagType::Single => {
                let mut node = IksNode::new_tag(name);
                node.attributes.extend(attributes.iter().cloned());

                match self.current.clone() {
                    Some(parent) => {
                        // `add_child` parent/prev/next bağlarını kurar ve
                        // yerleştirilmiş `Rc`'yi döndürür.
                        let node_rc = parent.borrow_mut().add_child(node);
                        if tag_type == TagType::Open {
                            self.current = Some(node_rc);
                        }
                    }
                    None => {
                        // Üst düzey element: C `*iksptr`'yi burada yazar.
                        // Kök zaten varsa **ezilir** — son kök kazanır.
                        let node_rc = node.into_rc();
                        if tag_type == TagType::Open {
                            self.current = Some(node_rc.clone());
                            self.open_root = Some(node_rc);
                        } else {
                            self.root = Some(node_rc);
                        }
                    }
                }
            }
            TagType::Close => {
                // C `iks_strcmp(NULL, name)` = -1 → IKS_BADXML.
                let Some(current) = self.current.clone() else {
                    return Err(IksError::BadXml);
                };
                if current.borrow().name.as_deref() != Some(name) {
                    return Err(IksError::BadXml);
                }
                let parent = current.borrow().parent();
                match parent {
                    Some(parent) => self.current = Some(parent),
                    None => {
                        self.root = Some(current);
                        self.current = None;
                        self.open_root = None;
                    }
                }
            }
        }
        Ok(())
    }

    /// Handles character data events during parsing.
    fn on_cdata(&mut self, data: &str) -> Result<()> {
        // C `cdataHook` koşulsuz `iks_insert_cdata` çağırır. `current` NULL
        // iken gelen metin (kökten önceki üst düzey metin) atılır.
        if let Some(parent) = self.current.as_ref() {
            crate::append_text(parent, data);
        }
        Ok(())
    }

    /// Handles document completion events during parsing.
    fn on_finish(&mut self) -> Result<()> {
        // Kapanmamış bir element varsa belge eksiktir.
        if self.current.is_some() {
            return Err(IksError::BadXml);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dom_child() {
        let xml = r#"
            <root>
                <child id="3"/>
            </root>"#;

        let dom = DomParser::parse_str(xml).unwrap();
        let root = dom.borrow();

        assert_eq!(root.name.as_ref().unwrap(), "root");
        // C semantiği: kökün içindeki boşluk metni düğüm olarak korunur.
        // Oracle: 3 çocuk — CDATA("\n                "), TAG child,
        // CDATA("\n            ").
        assert_eq!(root.children.len(), 3);
        assert_eq!(
            root.children[0].borrow().content.as_deref(),
            Some("\n                ")
        );

        let child = root.children[1].borrow();
        assert_eq!(child.name.as_ref().unwrap(), "child");
        assert_eq!(child.attributes[0], ("id".to_string(), "3".to_string()));
        assert!(child.children.is_empty());

        assert_eq!(
            root.children[2].borrow().content.as_deref(),
            Some("\n            ")
        );
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
        assert_eq!(
            root.attributes[0],
            ("version".to_string(), "1.0".to_string())
        );
        // C semantiği: 3 element + 4 boşluk CDATA düğümü = 7 çocuk.
        assert_eq!(root.children.len(), 7);

        let child1 = root.children[1].borrow();
        assert_eq!(child1.name.as_ref().unwrap(), "child");
        assert_eq!(child1.attributes[0], ("id".to_string(), "1".to_string()));
        assert_eq!(
            child1.children.first().unwrap().borrow().content.as_deref(),
            Some("Text1")
        );

        let child2 = root.children[3].borrow();
        assert_eq!(child2.name.as_ref().unwrap(), "child");
        assert_eq!(child2.attributes[0], ("id".to_string(), "2".to_string()));
        assert_eq!(
            child2.children.first().unwrap().borrow().content.as_deref(),
            Some("Text2")
        );

        let child3 = root.children[5].borrow();
        assert_eq!(child3.name.as_ref().unwrap(), "child");
        assert_eq!(child3.attributes[0], ("id".to_string(), "3".to_string()));
        assert!(child3.children.is_empty());
    }

    #[test]
    fn test_file_operations() -> Result<()> {
        let root = NodeRef::new_tag("root");
        root.borrow_mut().add_attribute("version", "1.0");

        let mut child = IksNode::new_tag("child");
        let mut cdata = IksNode::new(crate::IksType::CData);
        cdata.set_content("Hello World");
        child.add_child(cdata);
        root.borrow_mut().add_child(child);

        // Save to file
        let temp_path =
            std::env::temp_dir().join(format!("test_iksemel_dom_{}.xml", std::process::id()));
        DomParser::save_file(&root, temp_path.to_str().unwrap())?;

        // Load from file
        let loaded = DomParser::load_file(temp_path.to_str().unwrap())?;

        // Compare the XML strings
        let root_xml = root.borrow().to_string();
        let loaded_xml = loaded.borrow().to_string();
        assert_eq!(root_xml, loaded_xml);

        // Clean up the temporary file
        let _ = std::fs::remove_file(temp_path);

        Ok(())
    }

    #[test]
    fn test_dom_sibling_navigation() {
        let xml = r#"<list><item id="1"/><item id="2"/><item id="3"/></list>"#;
        let dom = DomParser::parse_str(xml).unwrap();
        let root = dom.borrow();
        assert_eq!(root.children.len(), 3);

        let item1 = root.children[0].borrow();
        let item2 = root.children[1].borrow();
        let item3 = root.children[2].borrow();

        assert_eq!(item1.next().unwrap().borrow().find_attrib("id"), Some("2"));
        assert_eq!(item2.next().unwrap().borrow().find_attrib("id"), Some("3"));
        assert!(item3.next().is_none());

        assert_eq!(item3.prev().unwrap().borrow().find_attrib("id"), Some("2"));
        assert_eq!(item2.prev().unwrap().borrow().find_attrib("id"), Some("1"));
        assert!(item1.prev().is_none());

        assert_eq!(
            item1.parent().unwrap().borrow().name.as_deref(),
            Some("list")
        );
    }

    #[test]
    fn test_dom_attribute_entities() {
        let xml = r#"<node msg="&quot;Hello &amp; World&quot;" hex="&#x41;"/>"#;
        let dom = DomParser::parse_str(xml).unwrap();
        let root = dom.borrow();
        assert_eq!(root.find_attrib("msg"), Some("\"Hello & World\""));
        assert_eq!(root.find_attrib("hex"), Some("A"));
    }

    /// C yalnızca kök element kapandığında `*iksptr`'yi yazar; kapanmamış
    /// belge için kök vermez. Rust `finish()` ile bunu hataya çevirir
    /// (spec D1: C'nin ölü `finish` parametresine anlam kazandırma kararı).
    #[test]
    fn unfinished_document_is_an_error() {
        assert!(matches!(
            DomParser::parse_str("<r><c/>"),
            Err(IksError::BadXml)
        ));
        assert!(matches!(DomParser::parse_str("<r>"), Err(IksError::BadXml)));
    }

    /// C: kapanış etiketi `current` NULL iken gelirse IKS_BADXML
    /// (`iks_strcmp(NULL, name)` = -1). Oracle: `</x>` → err=2,
    /// `<a/></b>` → err=2, `<a></a></b>` → err=2.
    #[test]
    fn stray_closing_tag_is_an_error() {
        assert!(matches!(
            DomParser::parse_str("</x>"),
            Err(IksError::BadXml)
        ));
        assert!(matches!(
            DomParser::parse_str("<a/></b>"),
            Err(IksError::BadXml)
        ));
        assert!(matches!(
            DomParser::parse_str("<a></a></b>"),
            Err(IksError::BadXml)
        ));
    }

    /// Hata verilse bile, kök kapandıysa ağaç handler'da kalır
    /// (C: `<a/></b>` → err=2 ama `*iksptr` dolu).
    #[test]
    fn completed_root_survives_a_later_error() {
        let parser = DomParser::new().unwrap();
        let mut sax = crate::Parser::new(parser);
        assert!(sax.parse("<a/></b>").is_err());
        let doc = sax.handler().document().expect("kök teslim edilmiş olmalı");
        assert_eq!(doc.borrow().name(), Some("a"));
    }

    /// C: kök kapanmadan sonra gelen yeni bir üst düzey element kökü **ezer**
    /// (son kök kazanır). Oracle: `<a/><b/>` → `iks_string` = `<b/>`.
    #[test]
    fn last_top_level_element_wins() {
        let dom = DomParser::parse_str("<a/><b/>").unwrap();
        assert_eq!(dom.borrow().name(), Some("b"));
    }

    /// C: kök kapanmadan önceki üst düzey metin atılır.
    #[test]
    fn text_before_root_is_discarded() {
        let dom = DomParser::parse_str("text<r/>").unwrap();
        assert_eq!(dom.borrow().name(), Some("r"));
        assert!(dom.borrow().children.is_empty());
    }

    /// Kapanış etiketi adı `current`'ın adıyla uyuşmazsa hata.
    #[test]
    fn mismatched_closing_tag_is_an_error() {
        assert!(matches!(
            DomParser::parse_str("<a><b></c></a>"),
            Err(IksError::BadXml)
        ));
    }
}
