use std::str;
use crate::{IksError, Result, TagType};


/// SAX parser callback trait
pub trait SaxHandler {
    /// Called when a tag is encountered
    fn on_tag(&mut self, name: &str, attributes: &[(String, String)], tag_type: TagType) -> Result<()>;
    
    /// Called when character data is encountered
    fn on_cdata(&mut self, data: &str) -> Result<()>;
}

/// XML Parser state
#[derive(Debug, PartialEq)]
enum State {
    CData,
    TagStart,
    Tag,
    TagEnd,
    Attribute,
    AttributeName,
    AttributeValue,
    ValueApos,
    ValueQuot,
    Whitespace,
    Entity,
    Comment,
    CommentEnd,
    Markup,
    MarkupEnd,
    CDataSection,
    CDataSectionEnd,
    Comment1,
    Comment2,
    Comment3,
    Sect,
    SectCData,
    SectCData1,
    SectCData2,
    SectCData3,
    SectCData4,
    SectCDataC,
    SectCDataE,
    SectCDataE2,
    Pi,
}

/// SAX-style XML parser
pub struct Parser<H: SaxHandler> {
    handler: H,
    state: State,
    buffer: String,
    tag_name: String,
    attr_name: String,
    attr_value: String,
    attributes: Vec<(String, String)>,
    tag_type: TagType,
    entity: String,
}
