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

use crate::{IksError, Result, TagType};
use std::str;

/// Helper function to calculate the size needed for escaping a string.
///
/// # Arguments
///
/// * `s` - The string to calculate escape size for
///
/// # Returns
///
/// The number of characters needed to escape the string
fn escape_size(s: &str) -> usize {
    s.chars()
        .map(|c| match c {
            '&' => 5,  // &amp;
            '<' => 4,  // &lt;
            '>' => 4,  // &gt;
            '"' => 6,  // &quot;
            '\'' => 6, // &apos;
            _ => 1,
        })
        .sum()
}

/// Helper function to escape XML special characters.
///
/// # Arguments
///
/// * `s` - The string to escape
///
/// # Returns
///
/// The escaped string
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

/// Trait for handling SAX-style XML parsing events.
///
/// This trait defines the callbacks that will be invoked during XML parsing.
/// Implement this trait to handle XML parsing events in a streaming fashion.
pub trait SaxHandler {
    /// Called when a tag is encountered during parsing.
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
    ) -> Result<()>;

    /// Called when character data is encountered during parsing.
    ///
    /// # Arguments
    ///
    /// * `data` - The character data encountered
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure
    fn on_cdata(&mut self, data: &str) -> Result<()>;
}

/// Resolves a single XML entity (named or numeric) into a character.
fn resolve_entity(entity: &str) -> Result<char> {
    match entity {
        "amp" => Ok('&'),
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "apos" => Ok('\''),
        "quot" => Ok('"'),
        _ if entity.starts_with("#x") || entity.starts_with("#X") => {
            let hex_str = &entity[2..];
            let code = u32::from_str_radix(hex_str, 16).map_err(|_| IksError::BadXml)?;
            char::from_u32(code).ok_or(IksError::BadXml)
        }
        _ if entity.starts_with('#') => {
            let dec_str = &entity[1..];
            let code = dec_str.parse::<u32>().map_err(|_| IksError::BadXml)?;
            char::from_u32(code).ok_or(IksError::BadXml)
        }
        _ => Err(IksError::BadXml),
    }
}

/// Unescapes all entities in an attribute value string, respecting the max expansions limit.
fn unescape_value(s: &str, entity_count: &mut usize, max_entities: usize) -> Result<String> {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::new();
            let mut closed = false;
            for next in chars.by_ref() {
                if next == ';' {
                    closed = true;
                    break;
                }
                if entity.len() >= 10 {
                    return Err(IksError::BadXml);
                }
                entity.push(next);
            }
            if !closed {
                return Err(IksError::BadXml);
            }
            *entity_count += 1;
            if *entity_count > max_entities {
                return Err(IksError::MaxEntityExpansionsExceeded);
            }
            let resolved = resolve_entity(&entity)?;
            result.push(resolved);
        } else {
            result.push(c);
        }
    }
    Ok(result)
}

/// Represents the current state of the XML parser.
#[derive(Debug, PartialEq, Eq)]
enum State {
    /// Parsing character data
    CData,
    /// At the start of a tag
    TagStart,
    /// Inside a tag
    Tag,
    /// At the end of a tag
    TagEnd,
    /// Parsing an attribute
    Attribute,
    /// Parsing an attribute name
    AttributeName,
    /// Parsing an attribute value
    AttributeValue,
    /// Parsing a single-quoted attribute value
    ValueApos,
    /// Parsing a double-quoted attribute value
    ValueQuot,
    /// Parsing an entity in text content
    Entity,
    /// After first dash of opening comment
    Comment,
    /// Inside comment content
    CommentBody,
    /// First dash inside comment
    CommentDash1,
    /// Second dash inside comment
    CommentDash2,
    /// Parsing markup
    Markup,
    /// At the end of markup
    MarkupEnd,
    /// Parsing a section
    Sect,
    /// Parsing a CDATA section declaration
    SectCData,
    /// Parsing CDATA section declaration (D)
    SectCData1,
    /// Parsing CDATA section declaration (A)
    SectCData2,
    /// Parsing CDATA section declaration (T)
    SectCData3,
    /// Parsing CDATA section declaration (A)
    SectCData4,
    /// Parsing CDATA section content
    SectCDataC,
    /// First closing bracket of CDATA section
    SectCDataE,
    /// Second closing bracket of CDATA section
    SectCDataE2,
    /// Parsing a processing instruction
    Pi,
    /// Found question mark inside processing instruction
    PiEnd,
}

/// SAX-style XML parser that processes XML data and calls appropriate handler methods.
///
/// This parser implements a state machine to process XML data character by character,
/// calling the appropriate methods on the provided handler as it encounters XML elements.
///
/// # Examples
///
/// ```
/// use iksemel::{Parser, SaxHandler, TagType, Result};
///
/// struct MyHandler;
///
/// impl SaxHandler for MyHandler {
///     fn on_tag(&mut self, name: &str, attributes: &[(String, String)], tag_type: TagType) -> Result<()> {
///         println!("Found tag: {} ({:?})", name, tag_type);
///         Ok(())
///     }
///     
///     fn on_cdata(&mut self, data: &str) -> Result<()> {
///         println!("Found text: {}", data);
///         Ok(())
///     }
/// }
///
/// let handler = MyHandler;
/// let mut parser = Parser::new(handler);
/// parser.parse("<root>Hello World</root>").unwrap();
/// ```
/// Security and resource limits for the XML parser to protect against DoS attacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParserLimits {
    /// Maximum allowed XML element nesting depth. Default: 1024.
    pub max_depth: usize,
    /// Maximum allowed entity expansions per parse session. Default: 100,000.
    pub max_entity_expansions: usize,
    /// Maximum allowed attributes on a single element. Default: 2048.
    pub max_attributes: usize,
    /// Maximum allowed size for a single token, attribute value, or CDATA buffer in bytes. Default: 10 MB.
    pub max_token_size: usize,
}

impl Default for ParserLimits {
    fn default() -> Self {
        ParserLimits {
            max_depth: 1024,
            max_entity_expansions: 100_000,
            max_attributes: 2048,
            max_token_size: 10 * 1024 * 1024,
        }
    }
}

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
    line: usize,
    column: usize,
    limits: ParserLimits,
    current_depth: usize,
    entity_expansions: usize,
}

/// Fast lookup table for XML name characters: [a-zA-Z0-9_.:-]
pub(crate) const TABLE_IS_NAME_CHAR: [bool; 256] = {
    let mut t = [false; 256];
    let mut i = b'a';
    while i <= b'z' {
        t[i as usize] = true;
        i += 1;
    }
    let mut i = b'A';
    while i <= b'Z' {
        t[i as usize] = true;
        i += 1;
    }
    let mut i = b'0';
    while i <= b'9' {
        t[i as usize] = true;
        i += 1;
    }
    t[b'_' as usize] = true;
    t[b':' as usize] = true;
    t[b'-' as usize] = true;
    t[b'.' as usize] = true;
    t
};

/// Fast lookup table for XML whitespace characters: [ \t\r\n]
pub(crate) const TABLE_IS_WHITESPACE: [bool; 256] = {
    let mut t = [false; 256];
    t[b' ' as usize] = true;
    t[b'\t' as usize] = true;
    t[b'\r' as usize] = true;
    t[b'\n' as usize] = true;
    t
};

/// Returns true if character is valid in XML Name.
pub fn is_xml_name_char(c: char) -> bool {
    let u = c as usize;
    u < 256 && TABLE_IS_NAME_CHAR[u]
}

/// Returns true if character is XML whitespace (space, tab, cr, lf).
pub fn is_xml_whitespace(c: char) -> bool {
    let u = c as usize;
    u < 256 && TABLE_IS_WHITESPACE[u]
}

impl<H: SaxHandler> Parser<H> {
    /// Creates a new parser with default security limits.
    pub fn new(handler: H) -> Self {
        Self::with_limits(handler, ParserLimits::default())
    }

    /// Creates a new parser with custom security limits.
    pub fn with_limits(handler: H, limits: ParserLimits) -> Self {
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
            line: 1,
            column: 0,
            limits,
            current_depth: 0,
            entity_expansions: 0,
        }
    }

    /// Returns the security limits configured for this parser.
    pub fn limits(&self) -> &ParserLimits {
        &self.limits
    }

    /// Returns the current nesting depth of the parser.
    pub fn current_depth(&self) -> usize {
        self.current_depth
    }

    /// Returns total number of entities expanded during parsing.
    pub fn entity_expansions(&self) -> usize {
        self.entity_expansions
    }

    /// Gets a reference to the handler.
    pub fn handler(&self) -> &H {
        &self.handler
    }

    /// Gets a mutable reference to the handler.
    pub fn handler_mut(&mut self) -> &mut H {
        &mut self.handler
    }

    /// Resets the parser state for reuse.
    pub fn reset(&mut self) {
        self.state = State::CData;
        self.buffer.clear();
        self.tag_name.clear();
        self.attr_name.clear();
        self.attr_value.clear();
        self.attributes.clear();
        self.tag_type = TagType::Open;
        self.entity.clear();
        self.line = 1;
        self.column = 0;
        self.current_depth = 0;
        self.entity_expansions = 0;
    }

    /// Parses a chunk of XML data.
    ///
    /// This method processes the input string character by character,
    /// updating the parser state and calling appropriate handler methods
    /// as it encounters XML elements.
    ///
    /// # Arguments
    ///
    /// * `data` - The XML data to parse
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure
    pub fn parse(&mut self, data: &str) -> Result<()> {
        let bytes = data.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if self.state == State::CData {
                let start = i;
                while i < bytes.len() && bytes[i] != b'<' && bytes[i] != b'&' {
                    let b = bytes[i];
                    if b == b'\n' {
                        self.line += 1;
                        self.column = 0;
                    } else if (b & 0xC0) != 0x80 {
                        self.column += 1;
                    }
                    i += 1;
                }

                if i > start {
                    self.buffer.push_str(&data[start..i]);
                }

                if i >= bytes.len() {
                    break;
                }

                let b = bytes[i];
                i += 1;
                self.column += 1;

                if b == b'<' {
                    if !self.buffer.is_empty() {
                        self.handler.on_cdata(&self.buffer)?;
                        self.buffer.clear();
                    }
                    self.state = State::TagStart;
                } else if b == b'&' {
                    self.state = State::Entity;
                }
                continue;
            } else if self.state == State::CommentBody {
                while i < bytes.len() && bytes[i] != b'-' {
                    let b = bytes[i];
                    if b == b'\n' {
                        self.line += 1;
                        self.column = 0;
                    } else if (b & 0xC0) != 0x80 {
                        self.column += 1;
                    }
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1;
                    self.column += 1;
                    self.state = State::CommentDash1;
                }
                continue;
            } else if self.state == State::SectCDataC {
                let start = i;
                while i < bytes.len() && bytes[i] != b']' {
                    let b = bytes[i];
                    if b == b'\n' {
                        self.line += 1;
                        self.column = 0;
                    } else if (b & 0xC0) != 0x80 {
                        self.column += 1;
                    }
                    i += 1;
                }
                if i > start {
                    self.buffer.push_str(&data[start..i]);
                }
                if i < bytes.len() {
                    i += 1;
                    self.column += 1;
                    self.state = State::SectCDataE;
                }
                continue;
            }

            let c = data[i..].chars().next().unwrap();
            let c_len = c.len_utf8();
            i += c_len;

            self.column += 1;
            if c == '\n' {
                self.line += 1;
                self.column = 0;
            }

            match self.state {
                State::CData | State::CommentBody | State::SectCDataC => unreachable!(),
                State::TagStart => match c {
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
                },
                State::Markup => match c {
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
                },
                State::Comment => {
                    if c != '-' {
                        return Err(IksError::BadXml);
                    }
                    self.state = State::CommentBody;
                }
                State::CommentDash1 => {
                    if c == '-' {
                        self.state = State::CommentDash2;
                    } else {
                        self.state = State::CommentBody;
                    }
                }
                State::CommentDash2 => {
                    if c == '>' {
                        self.state = State::CData;
                    } else if c != '-' {
                        self.state = State::CommentBody;
                    }
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
                    if c == '?' {
                        self.state = State::PiEnd;
                    }
                }
                State::PiEnd => {
                    if c == '>' {
                        self.state = State::CData;
                    } else if c != '?' {
                        self.state = State::Pi;
                    }
                }
                State::Tag => match c {
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
                    _ => self.tag_name.push(c),
                },
                State::Attribute => match c {
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
                },
                State::AttributeName => match c {
                    '=' => {
                        self.state = State::AttributeValue;
                    }
                    ' ' | '\t' | '\n' | '\r' => {
                        if !self.attr_name.is_empty() {
                            self.state = State::AttributeValue;
                        }
                    }
                    _ => self.attr_name.push(c),
                },
                State::AttributeValue => match c {
                    '\'' => self.state = State::ValueApos,
                    '"' => self.state = State::ValueQuot,
                    ' ' | '\t' | '\n' | '\r' => {}
                    _ => return Err(IksError::BadXml),
                },
                State::ValueApos => match c {
                    '\'' => {
                        let unescaped = unescape_value(
                            &self.attr_value,
                            &mut self.entity_expansions,
                            self.limits.max_entity_expansions,
                        )?;
                        if self.attributes.len() >= self.limits.max_attributes {
                            return Err(IksError::MaxAttributesExceeded);
                        }
                        self.attributes
                            .push((std::mem::take(&mut self.attr_name), unescaped));
                        self.attr_value.clear();
                        self.state = State::Attribute;
                    }
                    _ => {
                        if self.attr_value.len() >= self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                        self.attr_value.push(c);
                    }
                },
                State::ValueQuot => match c {
                    '"' => {
                        let unescaped = unescape_value(
                            &self.attr_value,
                            &mut self.entity_expansions,
                            self.limits.max_entity_expansions,
                        )?;
                        if self.attributes.len() >= self.limits.max_attributes {
                            return Err(IksError::MaxAttributesExceeded);
                        }
                        self.attributes
                            .push((std::mem::take(&mut self.attr_name), unescaped));
                        self.attr_value.clear();
                        self.state = State::Attribute;
                    }
                    _ => {
                        if self.attr_value.len() >= self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                        self.attr_value.push(c);
                    }
                },
                State::Entity => match c {
                    ';' => {
                        self.entity_expansions += 1;
                        if self.entity_expansions > self.limits.max_entity_expansions {
                            return Err(IksError::MaxEntityExpansionsExceeded);
                        }
                        let resolved = resolve_entity(&self.entity)?;
                        self.buffer.push(resolved);
                        self.entity.clear();
                        self.state = State::CData;
                    }
                    '<' | '&' | ' ' | '\t' | '\r' | '\n' => {
                        return Err(IksError::BadXml);
                    }
                    _ => {
                        if self.entity.len() >= 10 {
                            return Err(IksError::BadXml);
                        }
                        self.entity.push(c);
                    }
                },
                State::TagEnd => match c {
                    '>' => {
                        self.handle_tag_end()?;
                        self.tag_name.clear();
                        self.attributes.clear();
                    }
                    _ => return Err(IksError::BadXml),
                },
                State::MarkupEnd => {
                    if c == '>' {
                        self.state = State::CData;
                    }
                }
            }
        }

        // Handle any remaining character data
        if !self.buffer.is_empty() && self.state == State::CData {
            self.handler.on_cdata(&self.buffer)?;
            self.buffer.clear();
        }

        Ok(())
    }

    /// Handles the end of a tag.
    ///
    /// This method is called when a tag is fully parsed and calls the
    /// appropriate handler method.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure
    fn handle_tag_end(&mut self) -> Result<()> {
        match self.tag_type {
            TagType::Open => {
                self.current_depth += 1;
                if self.current_depth > self.limits.max_depth {
                    return Err(IksError::MaxDepthExceeded);
                }
            }
            TagType::Close => {
                self.current_depth = self.current_depth.saturating_sub(1);
            }
            TagType::Single => {
                if self.current_depth + 1 > self.limits.max_depth {
                    return Err(IksError::MaxDepthExceeded);
                }
            }
        }

        let result = self
            .handler
            .on_tag(&self.tag_name, &self.attributes, self.tag_type);

        // Only clear tag_name and attributes if it's not a single tag
        // This allows single tags to be properly handled as children
        if self.tag_type != TagType::Single {
            self.tag_name.clear();
            self.attributes.clear();
        }

        self.state = State::CData;

        result
    }

    /// Serializes the current XML state to a string.
    ///
    /// This method is useful for debugging or when you need to see the
    /// current state of the parser as XML.
    ///
    /// # Returns
    ///
    /// A string representation of the current XML state
    pub fn serialize(&self) -> String {
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

    /// Calculates the size needed for serialization.
    ///
    /// This method is used to pre-allocate buffers for serialization.
    ///
    /// # Returns
    ///
    /// The number of characters needed to serialize the current state
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

    /// Gets the current line number in the input.
    ///
    /// # Returns
    ///
    /// The current line number (1-based)
    pub fn line(&self) -> usize {
        self.line
    }

    /// Gets the current column number in the input.
    ///
    /// # Returns
    ///
    /// The current column number (0-based)
    pub fn column(&self) -> usize {
        self.column
    }
}

impl<H: SaxHandler> std::fmt::Display for Parser<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.serialize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type ParsedTag = (String, Vec<(String, String)>, TagType);

    struct TestHandler {
        tags: Vec<ParsedTag>,
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
        fn on_tag(
            &mut self,
            name: &str,
            attributes: &[(String, String)],
            tag_type: TagType,
        ) -> Result<()> {
            self.tags
                .push((name.to_string(), attributes.to_vec(), tag_type));
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
        assert_eq!(
            parser.handler.tags[0].1[0],
            ("attr".to_string(), "value".to_string())
        );
        assert_eq!(parser.handler.tags[0].2, TagType::Open);

        assert_eq!(parser.handler.cdata[0], "text");

        assert_eq!(parser.handler.tags[1].0, "root");
        assert_eq!(parser.handler.tags[1].2, TagType::Close);
    }

    #[test]
    fn test_numeric_and_special_entities() {
        let handler = TestHandler::new();
        let mut parser = Parser::new(handler);
        parser
            .parse("<msg>A &#65; &#x42; &amp; &lt; &gt; &apos; &quot;</msg>")
            .unwrap();
        assert_eq!(parser.handler.cdata[0], "A A B & < > ' \"");
    }

    #[test]
    fn test_attribute_entities() {
        let handler = TestHandler::new();
        let mut parser = Parser::new(handler);
        parser
            .parse("<tag title=\"&quot;Hello &amp; World&quot;\" num=\"&#x31;&#x32;\"/>")
            .unwrap();
        assert_eq!(parser.handler.tags.len(), 1);
        let attrs = &parser.handler.tags[0].1;
        assert_eq!(
            attrs[0],
            ("title".to_string(), "\"Hello & World\"".to_string())
        );
        assert_eq!(attrs[1], ("num".to_string(), "12".to_string()));
    }

    #[test]
    fn test_comments_with_dashes() {
        let handler = TestHandler::new();
        let mut parser = Parser::new(handler);
        parser
            .parse("<!-- a - b -- c ---><root><!-- another comment -->text</root>")
            .unwrap();
        assert_eq!(parser.handler.tags.len(), 2);
        assert_eq!(parser.handler.cdata[0], "text");
    }

    #[test]
    fn test_processing_instruction() {
        let handler = TestHandler::new();
        let mut parser = Parser::new(handler);
        parser
            .parse("<?xml version=\"1.0\" encoding=\"UTF-8\"?><root/>")
            .unwrap();
        assert_eq!(parser.handler.tags.len(), 1);
        assert_eq!(parser.handler.tags[0].0, "root");
    }

    #[test]
    fn test_cdata_section() {
        let handler = TestHandler::new();
        let mut parser = Parser::new(handler);
        parser
            .parse("<root><![CDATA[<unescaped> &amp; text]]></root>")
            .unwrap();
        assert_eq!(parser.handler.cdata[0], "<unescaped> &amp; text");
    }

    #[test]
    fn test_malformed_xml() {
        // Unknown entity
        let mut parser = Parser::new(TestHandler::new());
        assert!(parser.parse("<root>&unknown;</root>").is_err());

        // Unclosed entity
        let mut parser = Parser::new(TestHandler::new());
        assert!(parser.parse("<root>&amp</root>").is_err());

        // Malformed comment
        let mut parser = Parser::new(TestHandler::new());
        assert!(parser.parse("<!- wrong -->").is_err());
    }

    #[test]
    fn test_lookup_tables_and_fast_scanner() {
        assert!(is_xml_name_char('a'));
        assert!(is_xml_name_char('Z'));
        assert!(is_xml_name_char('0'));
        assert!(is_xml_name_char('_'));
        assert!(is_xml_name_char(':'));
        assert!(is_xml_name_char('-'));
        assert!(is_xml_name_char('.'));
        assert!(!is_xml_name_char('<'));
        assert!(!is_xml_name_char('>'));
        assert!(!is_xml_name_char('&'));

        assert!(is_xml_whitespace(' '));
        assert!(is_xml_whitespace('\t'));
        assert!(is_xml_whitespace('\n'));
        assert!(is_xml_whitespace('\r'));
        assert!(!is_xml_whitespace('a'));

        // Fast scanner over large text
        let long_text = "The quick brown fox jumps over the lazy dog. ".repeat(100);
        let xml = format!("<doc><text>{}</text></doc>", long_text);
        let handler = TestHandler::new();
        let mut parser = Parser::new(handler);
        parser.parse(&xml).unwrap();
        assert_eq!(parser.handler.cdata[0], long_text);
    }
}
