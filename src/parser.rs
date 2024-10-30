use std::str;
use crate::{IksError, Result, TagType};

/// Helper function to calculate the size needed for escaping a string
fn escape_size(s: &str) -> usize {
    s.chars().map(|c| match c {
        '&' => 5,  // &amp;
        '<' => 4,  // &lt;
        '>' => 4,  // &gt;
        '"' => 6,  // &quot;
        '\'' => 6, // &apos;
        _ => 1,
    }).sum()
}

/// Helper function to escape XML special characters
fn escape(s: &str) -> String {
    let mut result = String::with_capacity(escape_size(s));
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(c),
        }
    }
    result
}

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

impl<H: SaxHandler> Parser<H> {
    /// Create a new parser with the given handler
    pub fn new(handler: H) -> Self {
        Parser {
            handler,
            state: State::CData,
            buffer: String::new(),
            tag_name: String::new(),
            attr_name: String::new(),
            attr_value: String::new(),
            attributes: Vec::new(),
            tag_type: TagType::Open,
            entity: String::new(),
        }
    }

    /// Get the handler
    pub fn handler(&self) -> &H {
        &self.handler
    }

    /// Get the handler mutably
    pub fn handler_mut(&mut self) -> &mut H {
        &mut self.handler
    }

    /// Parse a chunk of XML data
    pub fn parse(&mut self, data: &str) -> Result<()> {
        for c in data.chars() {
            match self.state {
                State::CData => {
                    match c {
                        '<' => {
                            if !self.buffer.is_empty() {
                                self.handler.on_cdata(&self.buffer)?;
                                self.buffer.clear();
                            }
                            self.state = State::TagStart;
                        }
                        '&' => {
                            if !self.buffer.is_empty() {
                                self.handler.on_cdata(&self.buffer)?;
                                self.buffer.clear();
                            }
                            self.state = State::Entity;
                        }
                        _ => self.buffer.push(c)
                    }
                }
                State::TagStart => {
                    match c {
                        '/' => {
                            self.tag_type = TagType::Close;
                            self.state = State::Tag;
                        }
                        '?' => {
                            self.state = State::Pi;
                        }
                        '!' => {
                            self.state = State::Markup;
                        }
                        _ => {
                            self.tag_type = TagType::Open;
                            self.tag_name.push(c);
                            self.state = State::Tag;
                        }
                    }
                }
                State::Markup => {
                    match c {
                        '[' => {
                            self.state = State::Sect;
                        }
                        '-' => {
                            self.state = State::Comment;
                        }
                        '>' => {
                            self.state = State::CData;
                        }
                        _ => {
                            self.state = State::MarkupEnd;
                        }
                    }
                }
                State::Comment => {
                    if c != '-' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::Comment1;
                }
                State::Comment1 => {
                    if c == '-' {
                        self.state = State::Comment2;
                    }
                }
                State::Comment2 => {
                    if c == '-' {
                        self.state = State::Comment3;
                    } else {
                        self.state = State::Comment1;
                    }
                }
                State::Comment3 => {
                    if c != '>' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::CData;
                }
                State::Sect => {
                    if c != 'C' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::SectCData;
                }
                State::SectCData => {
                    if c != 'D' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::SectCData1;
                }
                State::SectCData1 => {
                    if c != 'A' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::SectCData2;
                }
                State::SectCData2 => {
                    if c != 'T' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::SectCData3;
                }
                State::SectCData3 => {
                    if c != 'A' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::SectCData4;
                }
                State::SectCData4 => {
                    if c != '[' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::SectCDataC;
                }
                State::SectCDataC => {
                    if c == ']' {
                        self.state = State::SectCDataE;
                    } else {
                        self.buffer.push(c);
                    }
                }
                State::SectCDataE => {
                    if c == ']' {
                        self.state = State::SectCDataE2;
                    } else {
                        self.buffer.push(']');
                        self.buffer.push(c);
                        self.state = State::SectCDataC;
                    }
                }
                State::SectCDataE2 => {
                    if c == '>' {
                        self.state = State::CData;
                    } else if c == ']' {
                        self.buffer.push(']');
                    } else {
                        self.buffer.push(']');
                        self.buffer.push(']');
                        self.buffer.push(c);
                        self.state = State::SectCDataC;
                    }
                }
                State::Pi => {
                    if c == '>' {
                        self.state = State::CData;
                    }
                }
                State::Tag => {
                    match c {
                        '>' => {
                            self.handle_tag_end()?;
                        }
                        '/' => {
                            self.tag_type = TagType::Single;
                            self.state = State::TagEnd;
                        }
                        ' ' | '\t' | '\n' | '\r' => {
                            if !self.tag_name.is_empty() {
                                self.state = State::Attribute;
                            }
                        }
                        _ => self.tag_name.push(c)
                    }
                }
                State::Attribute => {
                    match c {
                        '>' => {
                            self.handle_tag_end()?;
                        }
                        '/' => {
                            self.tag_type = TagType::Single;
                            self.state = State::TagEnd;
                        }
                        ' ' | '\t' | '\n' | '\r' => {}
                        _ => {
                            self.attr_name.push(c);
                            self.state = State::AttributeName;
                        }
                    }
                }
                State::AttributeName => {
                    match c {
                        '=' => {
                            self.state = State::AttributeValue;
                        }
                        ' ' | '\t' | '\n' | '\r' => {
                            if !self.attr_name.is_empty() {
                                self.state = State::AttributeValue;
                            }
                        }
                        _ => self.attr_name.push(c)
                    }
                }
                State::AttributeValue => {
                    match c {
                        '\'' => self.state = State::ValueApos,
                        '"' => self.state = State::ValueQuot,
                        ' ' | '\t' | '\n' | '\r' => {}
                        _ => return Err(IksError::BadXml)
                    }
                }
                State::ValueApos => {
                    match c {
                        '\'' => {
                            self.attributes.push((
                                std::mem::take(&mut self.attr_name),
                                std::mem::take(&mut self.attr_value)
                            ));
                            self.state = State::Attribute;
                        }
                        _ => self.attr_value.push(c)
                    }
                }
                State::ValueQuot => {
                    match c {
                        '"' => {
                            self.attributes.push((
                                std::mem::take(&mut self.attr_name),
                                std::mem::take(&mut self.attr_value)
                            ));
                            self.state = State::Attribute;
                        }
                        _ => self.attr_value.push(c)
                    }
                }
                State::Entity => {
                    match c {
                        ';' => {
                            let entity = match self.entity.as_str() {
                                "amp" => "&",
                                "lt" => "<",
                                "gt" => ">",
                                "apos" => "'",
                                "quot" => "\"",
                                _ => return Err(IksError::BadXml)
                            };
                            self.buffer.push_str(entity);
                            self.entity.clear();
                            self.state = State::CData;
                        }
                        _ => {
                            if self.entity.len() >= 8 {
                                return Err(IksError::BadXml);
                            }
                            self.entity.push(c);
                        }
                    }
                }
                State::TagEnd => {
                    match c {
                        '>' => {
                            self.handle_tag_end()?;
                            self.tag_name.clear();
                            self.attributes.clear();
                        }
                        _ => return Err(IksError::BadXml)
                    }
                }
                // Other states handling omitted for brevity
                _ => {}
            }
        }

        // Handle any remaining character data
        if !self.buffer.is_empty() && self.state == State::CData {
            self.handler.on_cdata(&self.buffer)?;
            self.buffer.clear();
        }

        Ok(())
    }

    fn handle_tag_end(&mut self) -> Result<()> {
        let result = self.handler.on_tag(
            &self.tag_name,
            &self.attributes,
            self.tag_type
        );
        
        // Only clear tag_name and attributes if it's not a single tag
        // This allows single tags to be properly handled as children
        if self.tag_type != TagType::Single {
            self.tag_name.clear();
            self.attributes.clear();
        }
        
        self.state = State::CData;
        
        result
    }

    /// Serialize the current XML state to a string
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        
        // Handle CDATA
        if !self.buffer.is_empty() {
            result.push_str(&escape(&self.buffer));
        }

        // Handle tag
        if !self.tag_name.is_empty() {
            result.push('<');
            if self.tag_type == TagType::Close {
                result.push('/');
            }
            result.push_str(&escape(&self.tag_name));

            // Handle attributes
            for (name, value) in &self.attributes {
                result.push(' ');
                result.push_str(&escape(name));
                result.push('=');
                result.push('"');
                result.push_str(&escape(value));
                result.push('"');
            }

            if self.tag_type == TagType::Single {
                result.push('/');
            }
            result.push('>');
        }

        result
    }

    /// Calculate the size needed for serialization
    pub fn serialized_size(&self) -> usize {
        let mut size = 0;

        // Add size for CDATA
        if !self.buffer.is_empty() {
            size += escape_size(&self.buffer);
        }

        // Add size for tag
        if !self.tag_name.is_empty() {
            size += 1; // <
            if self.tag_type == TagType::Close {
                size += 1; // /
            }
            size += escape_size(&self.tag_name);

            // Add size for attributes
            for (name, value) in &self.attributes {
                size += 1; // space
                size += escape_size(name);
                size += 1; // =
                size += 1; // "
                size += escape_size(value);
                size += 1; // "
            }

            if self.tag_type == TagType::Single {
                size += 1; // /
            }
            size += 1; // >
        }

        size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestHandler {
        tags: Vec<(String, Vec<(String, String)>, TagType)>,
        cdata: Vec<String>,
    }
    
    impl TestHandler {
        fn new() -> Self {
            TestHandler {
                tags: Vec::new(),
                cdata: Vec::new(),
            }
        }
    }
    
    impl SaxHandler for TestHandler {
        fn on_tag(&mut self, name: &str, attributes: &[(String, String)], tag_type: TagType) -> Result<()> {
            self.tags.push((
                name.to_string(),
                attributes.to_vec(),
                tag_type
            ));
            Ok(())
        }
        
        fn on_cdata(&mut self, data: &str) -> Result<()> {
            self.cdata.push(data.to_string());
            Ok(())
        }
    }
    
    #[test]
    fn test_basic_parsing() {
        let handler = TestHandler::new();
        let mut parser = Parser::new(handler);
        
        parser.parse("<root attr=\"value\">text</root>").unwrap();
        
        assert_eq!(parser.handler.tags.len(), 2);
        assert_eq!(parser.handler.tags[0].0, "root");
        assert_eq!(parser.handler.tags[0].1[0], ("attr".to_string(), "value".to_string()));
        assert_eq!(parser.handler.tags[0].2, TagType::Open);
        
        assert_eq!(parser.handler.cdata[0], "text");
        
        assert_eq!(parser.handler.tags[1].0, "root");
        assert_eq!(parser.handler.tags[1].2, TagType::Close);
    }
} 