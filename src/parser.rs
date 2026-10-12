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

use crate::escape::{escape_to_string, escaped_len};
use crate::{IksError, Result, TagType};
use std::str;

/// XML 1.0 Fifth Edition character range.
pub fn is_xml_char(c: char) -> bool {
    matches!(c as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
}

/// XML 1.0 Fifth Edition NameStartChar.
pub fn is_xml_name_start(c: char) -> bool {
    matches!(c, ':' | '_' | 'A'..='Z' | 'a'..='z')
        || matches!(c as u32, 0xC0..=0xD6 | 0xD8..=0xF6 | 0xF8..=0x2FF | 0x370..=0x37D | 0x37F..=0x1FFF | 0x200C..=0x200D | 0x2070..=0x218F | 0x2C00..=0x2FEF | 0x3001..=0xD7FF | 0xF900..=0xFDCF | 0xFDF0..=0xFFFD | 0x10000..=0xEFFFF)
}

/// Validate an XML name (namespace QName rules are application-level).
pub fn valid_xml_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(is_xml_name_start)
        && chars.all(|c| {
            is_xml_name_start(c)
                || matches!(c, '-' | '.' | '0'..='9' | '\u{B7}')
                || matches!(c as u32, 0x300..=0x36F | 0x203F..=0x2040)
        })
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

    /// Preserve a complete CDATA section; defaults to the legacy text callback.
    fn on_cdata_section(&mut self, data: &str) -> Result<()> {
        self.on_cdata(data)
    }

    /// Called for comments in strict document mode, without delimiters.
    fn on_comment(&mut self, _data: &str) -> Result<()> {
        Ok(())
    }

    /// Called for processing instructions in strict document mode.
    fn on_processing_instruction(&mut self, _target: &str, _data: &str) -> Result<()> {
        Ok(())
    }

    /// A validated XML 1.0 UTF-8 declaration in strict document mode.
    fn on_xml_declaration(&mut self, _attributes: &[(String, String)]) -> Result<()> {
        Ok(())
    }

    /// Belge bittiğinde çağrılır.
    ///
    /// Varsayılan gövde hiçbir şey yapmaz; bu, trait'i uygulayan mevcut
    /// tiplerin (DOM dışındaki tüm handler'lar) değişmeden derlenmesini
    /// sağlar.
    fn on_finish(&mut self) -> Result<()> {
        Ok(())
    }
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
fn unescape_value(
    s: &str,
    entity_count: &mut usize,
    max_entities: usize,
    strict: bool,
    max_token: usize,
) -> Result<String> {
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
                if entity.len() >= if strict { max_token } else { 10 } {
                    return Err(if strict {
                        IksError::MaxTokenSizeExceeded
                    } else {
                        IksError::BadXml
                    });
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
            if strict && (!is_xml_char(resolved) || !valid_reference(&entity)) {
                return Err(IksError::BadXml);
            }
            result.push(resolved);
        } else {
            result.push(if strict && is_xml_whitespace(c) {
                ' '
            } else {
                c
            });
        }
    }
    Ok(result)
}

fn valid_reference(entity: &str) -> bool {
    if let Some(hex) = entity.strip_prefix("#x") {
        !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit())
    } else if let Some(decimal) = entity.strip_prefix('#') {
        !decimal.is_empty() && decimal.bytes().all(|b| b.is_ascii_digit())
    } else {
        true
    }
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
    /// A strict-mode attribute value ended; whitespace is required before another attribute.
    AttributeAfterValue,
    /// Parsing an attribute name
    AttributeName,
    /// Whitespace after an attribute name, before the equals sign.
    AttributeEquals,
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
    /// Recognize a forbidden DOCTYPE without misclassifying arbitrary malformed markup.
    DtdHeader,
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
    strict_document: bool,
    open_names: Vec<String>,
    roots: usize,
    finished: bool,
    seen_any: bool,
    pi_at_start: bool,
    literal_brackets: u8,
    token_bytes: usize,
    first_character: bool,
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
            strict_document: false,
            open_names: Vec::new(),
            roots: 0,
            finished: false,
            seen_any: false,
            pi_at_start: false,
            literal_brackets: 0,
            token_bytes: 0,
            first_character: true,
        }
    }

    /// Opt into complete-document validation and preservation callbacks.
    /// Input must already have XML end-of-line normalization applied. DTDs are forbidden.
    pub fn with_document_limits(handler: H, limits: ParserLimits) -> Self {
        let mut parser = Self::with_limits(handler, limits);
        parser.strict_document = true;
        parser
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

    /// Recover the owned event handler after parsing.
    pub fn into_handler(self) -> H {
        self.handler
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
        self.open_names.clear();
        self.roots = 0;
        self.finished = false;
        self.seen_any = false;
        self.pi_at_start = false;
        self.literal_brackets = 0;
        self.token_bytes = 0;
        self.first_character = true;
    }

    /// Belgenin bittiğini handler'a bildirir.
    ///
    /// C `iks_parse`'ın `finish` parametresi ölü koddur (`sax.c:634-644`
    /// onu hiçbir yere geçirmez). Bu metot o parametrenin amaçlanan anlamını
    /// gerçekler: handler'ın "belge tamamlandı" bilgisine göre karar
    /// vermesini sağlar.
    pub fn finish(&mut self) -> Result<()> {
        if self.strict_document
            && (self.finished
                || self.state != State::CData
                || !self.open_names.is_empty()
                || self.roots != 1)
        {
            return Err(IksError::BadXml);
        }
        self.flush_text()?;
        self.handler.on_finish()?;
        self.finished = true;
        Ok(())
    }

    /// Emits buffered character data at a C SAX chunk boundary. Partial entities
    /// and CDATA delimiters remain buffered until they can be decoded safely.
    pub fn flush_text(&mut self) -> Result<()> {
        if self.state == State::CData && !self.buffer.is_empty() {
            self.handler.on_cdata(&self.buffer)?;
            self.buffer.clear();
        }
        Ok(())
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
        if self.strict_document && self.finished {
            return Err(IksError::BadXml);
        }
        // C `sax_core` bayt döngüsünün en başında 0x00'ı reddeder ve bu
        // kontrol bağlamdan bağımsızdır (sax.c:209). Hızlı bayt tarama
        // yolları (CData, CommentBody, SectCDataC) ayırıcılarına kadar ham
        // baytları geçtiği için döngü tepesindeki bir kontrol NUL'u göremez;
        // bu yüzden tarama burada, durum makinesinden önce yapılır. Üç
        // döngüyü tek tek yamamak, dördüncü bir döngünün deliği sessizce
        // yeniden açmasına yol açardı.
        if data.as_bytes().contains(&0) {
            return Err(IksError::BadXml);
        }

        let bytes = data.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            if !self.strict_document && self.state == State::CData {
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
                    if self.buffer.len() > self.limits.max_token_size {
                        return Err(IksError::MaxTokenSizeExceeded);
                    }
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
            } else if !self.strict_document && self.state == State::CommentBody {
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
            } else if !self.strict_document && self.state == State::SectCDataC {
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
                    if self.buffer.len() > self.limits.max_token_size {
                        return Err(IksError::MaxTokenSizeExceeded);
                    }
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

            if self.strict_document {
                let first = self.first_character;
                self.first_character = false;
                if first && c == '\u{FEFF}' {
                    continue;
                }
                if self.state != State::CData {
                    self.token_bytes += c_len;
                    if self.token_bytes > self.limits.max_token_size {
                        return Err(IksError::MaxTokenSizeExceeded);
                    }
                }
            }

            if self.strict_document && !is_xml_char(c) {
                return Err(IksError::BadXml);
            }

            self.column += 1;
            if c == '\n' {
                self.line += 1;
                self.column = 0;
            }

            match self.state {
                State::CData => match c {
                    '<' => {
                        self.token_bytes = 1;
                        self.pi_at_start = !self.seen_any;
                        self.seen_any = true;
                        self.literal_brackets = 0;
                        self.flush_text()?;
                        self.state = State::TagStart;
                    }
                    '&' => {
                        self.token_bytes = 1;
                        if self.open_names.is_empty() {
                            return Err(IksError::BadXml);
                        }
                        self.seen_any = true;
                        self.literal_brackets = 0;
                        self.state = State::Entity;
                    }
                    _ => {
                        self.seen_any = true;
                        if self.open_names.is_empty() && !is_xml_whitespace(c) {
                            return Err(IksError::BadXml);
                        }
                        if c == '>' && self.literal_brackets == 2 {
                            return Err(IksError::BadXml);
                        }
                        self.literal_brackets = if c == ']' {
                            (self.literal_brackets + 1).min(2)
                        } else {
                            0
                        };
                        self.buffer.push(c);
                    }
                },
                State::CommentBody => {
                    if c == '-' {
                        self.state = State::CommentDash1;
                    } else {
                        self.buffer.push(c);
                    }
                }
                State::SectCDataC => {
                    if c == ']' {
                        self.state = State::SectCDataE;
                    } else {
                        self.buffer.push(c);
                    }
                }
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
                        if self.tag_name.len() > self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                        self.state = State::Tag;
                    }
                },
                State::Markup => match c {
                    '[' => {
                        if self.strict_document && self.open_names.is_empty() {
                            return Err(IksError::BadXml);
                        }
                        self.state = State::Sect;
                    }
                    '-' => {
                        self.state = State::Comment;
                    }
                    '>' => {
                        if self.strict_document {
                            return Err(IksError::BadXml);
                        }
                        self.state = State::CData;
                    }
                    _ => {
                        if self.strict_document {
                            if c == 'D' {
                                self.buffer.push(c);
                                self.state = State::DtdHeader;
                                continue;
                            }
                            return Err(IksError::BadXml);
                        }
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
                        if self.strict_document {
                            self.buffer.push('-');
                            self.buffer.push(c);
                        }
                        self.state = State::CommentBody;
                    }
                }
                State::CommentDash2 => {
                    if c == '>' {
                        if self.strict_document {
                            self.handler.on_comment(&self.buffer)?;
                            self.buffer.clear();
                        }
                        self.state = State::CData;
                    } else if c != '-' {
                        if self.strict_document {
                            return Err(IksError::BadXml);
                        }
                        self.state = State::CommentBody;
                    } else if self.strict_document {
                        return Err(IksError::BadXml);
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
                    if self.buffer.len() > self.limits.max_token_size {
                        return Err(IksError::MaxTokenSizeExceeded);
                    }
                }
                State::SectCDataE2 => {
                    if c == '>' {
                        if self.strict_document {
                            self.handler.on_cdata_section(&self.buffer)?;
                            self.buffer.clear();
                        }
                        self.state = State::CData;
                    } else if c == ']' {
                        self.buffer.push(']');
                    } else {
                        self.buffer.push(']');
                        self.buffer.push(']');
                        self.buffer.push(c);
                        self.state = State::SectCDataC;
                    }
                    if self.buffer.len() > self.limits.max_token_size {
                        return Err(IksError::MaxTokenSizeExceeded);
                    }
                }
                State::Pi => {
                    if c == '?' {
                        self.state = State::PiEnd;
                    } else if self.strict_document {
                        self.buffer.push(c);
                    }
                }
                State::PiEnd => {
                    if c == '>' {
                        if self.strict_document {
                            self.finish_pi()?;
                            self.buffer.clear();
                        }
                        self.state = State::CData;
                    } else if c != '?' {
                        if self.strict_document {
                            self.buffer.push('?');
                            self.buffer.push(c);
                        }
                        self.state = State::Pi;
                    } else if self.strict_document {
                        self.buffer.push('?');
                    }
                }
                State::Tag => match c {
                    '>' => {
                        self.handle_tag_end()?;
                    }
                    '/' => {
                        if self.tag_type == TagType::Close {
                            return Err(IksError::BadXml);
                        }
                        self.tag_type = TagType::Single;
                        self.state = State::TagEnd;
                    }
                    ' ' | '\t' | '\n' | '\r' => {
                        if !self.tag_name.is_empty() {
                            self.state = State::Attribute;
                        }
                    }
                    _ => {
                        self.tag_name.push(c);
                        if self.tag_name.len() > self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                    }
                },
                State::Attribute => match c {
                    '>' => {
                        self.handle_tag_end()?;
                    }
                    '/' => {
                        if self.tag_type == TagType::Close {
                            return Err(IksError::BadXml);
                        }
                        self.tag_type = TagType::Single;
                        self.state = State::TagEnd;
                    }
                    ' ' | '\t' | '\n' | '\r' => {}
                    _ => {
                        if self.strict_document && self.tag_type == TagType::Close {
                            return Err(IksError::BadXml);
                        }
                        self.attr_name.push(c);
                        if self.attr_name.len() > self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                        self.state = State::AttributeName;
                    }
                },
                State::AttributeAfterValue => match c {
                    ' ' | '\t' | '\n' | '\r' => self.state = State::Attribute,
                    '>' => self.handle_tag_end()?,
                    '/' => {
                        self.tag_type = TagType::Single;
                        self.state = State::TagEnd;
                    }
                    _ => return Err(IksError::BadXml),
                },
                State::AttributeName => match c {
                    '=' => {
                        self.state = State::AttributeValue;
                    }
                    ' ' | '\t' | '\n' | '\r' => {
                        if !self.attr_name.is_empty() {
                            self.state = State::AttributeEquals;
                        }
                    }
                    _ => {
                        self.attr_name.push(c);
                        if self.attr_name.len() > self.limits.max_token_size {
                            return Err(IksError::MaxTokenSizeExceeded);
                        }
                    }
                },
                State::AttributeEquals => match c {
                    '=' => self.state = State::AttributeValue,
                    ' ' | '\t' | '\n' | '\r' => {}
                    _ => return Err(IksError::BadXml),
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
                            self.strict_document,
                            self.limits.max_token_size,
                        )?;
                        if self.attributes.len() >= self.limits.max_attributes {
                            return Err(IksError::MaxAttributesExceeded);
                        }
                        self.attributes
                            .push((std::mem::take(&mut self.attr_name), unescaped));
                        self.attr_value.clear();
                        self.state = if self.strict_document {
                            State::AttributeAfterValue
                        } else {
                            State::Attribute
                        };
                    }
                    _ => {
                        if self.strict_document && c == '<' {
                            return Err(IksError::BadXml);
                        }
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
                            self.strict_document,
                            self.limits.max_token_size,
                        )?;
                        if self.attributes.len() >= self.limits.max_attributes {
                            return Err(IksError::MaxAttributesExceeded);
                        }
                        self.attributes
                            .push((std::mem::take(&mut self.attr_name), unescaped));
                        self.attr_value.clear();
                        self.state = if self.strict_document {
                            State::AttributeAfterValue
                        } else {
                            State::Attribute
                        };
                    }
                    _ => {
                        if self.strict_document && c == '<' {
                            return Err(IksError::BadXml);
                        }
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
                        if self.strict_document
                            && (!is_xml_char(resolved) || !valid_reference(&self.entity))
                        {
                            return Err(IksError::BadXml);
                        }
                        self.buffer.push(resolved);
                        self.entity.clear();
                        self.state = State::CData;
                    }
                    '<' | '&' | ' ' | '\t' | '\r' | '\n' => {
                        return Err(IksError::BadXml);
                    }
                    _ => {
                        if self.entity.len()
                            >= if self.strict_document {
                                self.limits.max_token_size
                            } else {
                                10
                            }
                        {
                            return Err(if self.strict_document {
                                IksError::MaxTokenSizeExceeded
                            } else {
                                IksError::BadXml
                            });
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
                State::DtdHeader => {
                    if self.buffer == "DOCTYPE" {
                        return Err(if is_xml_whitespace(c) {
                            IksError::DtdForbidden
                        } else {
                            IksError::BadXml
                        });
                    }
                    self.buffer.push(c);
                    if !"DOCTYPE".starts_with(self.buffer.as_str()) {
                        return Err(IksError::BadXml);
                    }
                }
            }
            if self.strict_document
                && (self.buffer.len() > self.limits.max_token_size
                    || self.attr_value.len() > self.limits.max_token_size
                    || self.tag_name.len() > self.limits.max_token_size
                    || self.attr_name.len() > self.limits.max_token_size)
            {
                return Err(IksError::MaxTokenSizeExceeded);
            }
        }

        // Handle any remaining character data
        if !self.buffer.is_empty() && self.state == State::CData {
            self.handler.on_cdata(&self.buffer)?;
            self.buffer.clear();
        }

        Ok(())
    }

    fn finish_pi(&mut self) -> Result<()> {
        let raw = self.buffer.as_str();
        let split = raw.find(is_xml_whitespace).unwrap_or(raw.len());
        let target = &raw[..split];
        if !valid_xml_name(target) {
            return Err(IksError::BadXml);
        }
        if target.eq_ignore_ascii_case("xml") {
            if target != "xml" || !self.pi_at_start {
                return Err(IksError::BadXml);
            }
            let mut rest = &raw[split..];
            let mut attributes = Vec::new();
            while !rest.is_empty() {
                let trimmed = rest.trim_start_matches(is_xml_whitespace);
                if trimmed.len() == rest.len() {
                    return Err(IksError::BadXml);
                }
                rest = trimmed;
                if rest.is_empty() {
                    break;
                }
                let end = rest
                    .find(|c: char| !c.is_ascii_alphabetic())
                    .unwrap_or(rest.len());
                let name = &rest[..end];
                rest = rest[end..].trim_start_matches(is_xml_whitespace);
                rest = rest
                    .strip_prefix('=')
                    .ok_or(IksError::BadXml)?
                    .trim_start_matches(is_xml_whitespace);
                let quote = rest.chars().next().ok_or(IksError::BadXml)?;
                if !matches!(quote, '\'' | '"') {
                    return Err(IksError::BadXml);
                }
                rest = &rest[1..];
                let end = rest.find(quote).ok_or(IksError::BadXml)?;
                let value = &rest[..end];
                attributes.push((name.to_owned(), value.to_owned()));
                rest = &rest[end + 1..];
            }
            if attributes.first().map(|(name, _)| name.as_str()) != Some("version")
                || attributes.len() > 3
            {
                return Err(IksError::BadXml);
            }
            let version = &attributes[0].1;
            if version != "1.0" {
                if version
                    .strip_prefix("1.")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                {
                    return Err(IksError::UnsupportedXml(
                        "XML version (only 1.0 is supported)",
                    ));
                }
                return Err(IksError::BadXml);
            }
            let mut seen_encoding = false;
            let mut seen_standalone = false;
            for (name, value) in &attributes[1..] {
                match name.as_str() {
                    "encoding" if !seen_encoding && !seen_standalone => {
                        seen_encoding = true;
                        if !value
                            .as_bytes()
                            .first()
                            .is_some_and(u8::is_ascii_alphabetic)
                            || !value.bytes().all(|b| {
                                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')
                            })
                        {
                            return Err(IksError::BadXml);
                        }
                        if !value.eq_ignore_ascii_case("UTF-8") {
                            return Err(IksError::UnsupportedXml(
                                "encoding (only UTF-8 is supported)",
                            ));
                        }
                    }
                    "standalone" if !seen_standalone && matches!(value.as_str(), "yes" | "no") => {
                        seen_standalone = true
                    }
                    _ => return Err(IksError::BadXml),
                }
            }
            self.handler.on_xml_declaration(&attributes)
        } else {
            let data = if split == raw.len() {
                ""
            } else {
                &raw[split + raw[split..].chars().next().unwrap().len_utf8()..]
            };
            self.handler.on_processing_instruction(target, data)
        }
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
        if self.strict_document {
            if matches!(self.tag_type, TagType::Open | TagType::Single)
                && self.current_depth >= self.limits.max_depth
            {
                return Err(IksError::MaxDepthExceeded);
            }
            if !valid_xml_name(&self.tag_name)
                || self
                    .attributes
                    .iter()
                    .any(|(name, _)| !valid_xml_name(name))
            {
                return Err(IksError::BadXml);
            }
            let mut names = std::collections::HashSet::new();
            if self.attributes.iter().any(|(name, _)| !names.insert(name)) {
                return Err(IksError::BadXml);
            }
            match self.tag_type {
                TagType::Open | TagType::Single => {
                    if self.open_names.is_empty() {
                        self.roots += 1;
                        if self.roots > 1 {
                            return Err(IksError::BadXml);
                        }
                    }
                    if self.tag_type == TagType::Open {
                        self.open_names.push(self.tag_name.clone());
                    }
                }
                TagType::Close => {
                    if self.open_names.pop().as_deref() != Some(self.tag_name.as_str())
                        || !self.attributes.is_empty()
                    {
                        return Err(IksError::BadXml);
                    }
                }
            }
        }
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
            result.push_str(&escape_to_string(&self.buffer));
        }

        // Handle tag
        if !self.tag_name.is_empty() {
            result.push('<');
            if self.tag_type == TagType::Close {
                result.push('/');
            }
            result.push_str(&escape_to_string(&self.tag_name));

            // Handle attributes
            for (name, value) in &self.attributes {
                result.push(' ');
                result.push_str(&escape_to_string(name));
                result.push('=');
                result.push('"');
                result.push_str(&escape_to_string(value));
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
            size += escaped_len(&self.buffer);
        }

        // Add size for tag
        if !self.tag_name.is_empty() {
            size += 1; // <
            if self.tag_type == TagType::Close {
                size += 1; // /
            }
            size += escaped_len(&self.tag_name);

            // Add size for attributes
            for (name, value) in &self.attributes {
                size += 1; // space
                size += escaped_len(name);
                size += 1; // =
                size += 1; // "
                size += escaped_len(value);
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

    /// Testler için hiçbir şey yapmayan handler.
    struct NullHandler;

    impl SaxHandler for NullHandler {
        fn on_tag(
            &mut self,
            _name: &str,
            _attributes: &[(String, String)],
            _tag_type: TagType,
        ) -> Result<()> {
            Ok(())
        }
        fn on_cdata(&mut self, _data: &str) -> Result<()> {
            Ok(())
        }
    }

    /// C `sax_core` bayt döngüsünün en başında 0x00'ı reddeder (sax.c:209),
    /// bağlamdan bağımsız olarak.
    #[test]
    fn nul_byte_is_rejected_in_every_context() {
        let cases = [
            "<r>a\0b</r>",      // metin (CData hızlı yolu)
            "<r x=\"a\0b\"/>",  // attribute değeri
            "<r x\0y=\"1\"/>",  // attribute adı
            "<r\0>text</r>",    // tag adı (TagStart dalı)
            "<![CDATA[a\0b]]>", // kesit CDATA (SectCDataC hızlı yolu)
            "<!-- a\0b -->",    // yorum (CommentBody hızlı yolu)
            "<?pi a\0b?>",      // işlem talimatı
        ];
        for xml in cases {
            let mut parser = Parser::new(NullHandler);
            assert!(
                matches!(parser.parse(xml), Err(IksError::BadXml)),
                "reddedilmeliydi: {:?}",
                xml
            );
        }
    }

    /// Kontrol: NUL içermeyen benzer belgeler hata vermez.
    #[test]
    fn documents_without_nul_still_parse() {
        for xml in [
            "<r>ab</r>",
            "<r x=\"ab\"/>",
            "<![CDATA[ab]]>",
            "<!-- ab -->",
        ] {
            let mut parser = Parser::new(NullHandler);
            assert!(parser.parse(xml).is_ok(), "kabul edilmeliydi: {:?}", xml);
        }
    }

    fn limits_with_token_size(n: usize) -> ParserLimits {
        ParserLimits {
            max_token_size: n,
            ..ParserLimits::default()
        }
    }

    /// `max_token_size`, belgelenmiş sözleşmesi gereği *her* biriktirme
    /// yolunu kapsar.
    #[test]
    fn max_token_size_covers_every_accumulation_path() {
        let cases = [
            "<r>aaaaaaaaaaaa</r>",             // bulunan metin
            "<r><![CDATA[aaaaaaaaaaaa]]></r>", // kesit CDATA
            "<aaaaaaaaaaaa>",                  // tag adı (TagStart)
            "<r aaaaaaaaaaaa=\"1\"/>",         // attribute adı (Attribute)
            "<r aaaaaaaaaaaa>",                // attribute adı (AttributeName)
            "<r x=\"aaaaaaaaaaaa\"/>",         // attribute değeri (zaten vardı)
            "<r><![CDATA[aaaaaaaa]]]]></r>",   // SectCDataE2 geri alma yolu
        ];
        for xml in cases {
            let mut parser = Parser::with_limits(NullHandler, limits_with_token_size(4));
            assert!(
                matches!(parser.parse(xml), Err(IksError::MaxTokenSizeExceeded)),
                "sınırı aşmalıydı: {:?}",
                xml
            );
        }
    }

    /// Sınır davranışı: 4 bayt kabul, 5 bayt ret.
    #[test]
    fn max_token_size_boundary_is_exact() {
        let mut ok = Parser::with_limits(NullHandler, limits_with_token_size(4));
        assert!(ok.parse("<r>abcd</r>").is_ok());

        let mut too_long = Parser::with_limits(NullHandler, limits_with_token_size(4));
        assert!(matches!(
            too_long.parse("<r>abcde</r>"),
            Err(IksError::MaxTokenSizeExceeded)
        ));
    }

    /// Varsayılan limitlerle büyük ama meşru bir belge hata vermemeli.
    #[test]
    fn default_limits_do_not_reject_large_legitimate_tokens() {
        let big = "a".repeat(1024 * 1024);

        let mut text = Parser::new(NullHandler);
        assert!(text.parse(&format!("<r>{}</r>", big)).is_ok());

        let mut cdata = Parser::new(NullHandler);
        assert!(cdata.parse(&format!("<r><![CDATA[{}]]></r>", big)).is_ok());
    }
}

#[cfg(test)]
mod document_tests {
    use super::*;

    #[derive(Default)]
    struct Events {
        comments: Vec<String>,
        sections: Vec<String>,
        pis: Vec<(String, String)>,
        declarations: usize,
    }
    impl SaxHandler for Events {
        fn on_tag(&mut self, _: &str, _: &[(String, String)], _: TagType) -> Result<()> {
            Ok(())
        }
        fn on_cdata(&mut self, _: &str) -> Result<()> {
            Ok(())
        }
        fn on_comment(&mut self, text: &str) -> Result<()> {
            self.comments.push(text.to_owned());
            Ok(())
        }
        fn on_cdata_section(&mut self, text: &str) -> Result<()> {
            self.sections.push(text.to_owned());
            Ok(())
        }
        fn on_processing_instruction(&mut self, target: &str, data: &str) -> Result<()> {
            self.pis.push((target.to_owned(), data.to_owned()));
            Ok(())
        }
        fn on_xml_declaration(&mut self, _: &[(String, String)]) -> Result<()> {
            self.declarations += 1;
            Ok(())
        }
    }

    #[test]
    fn document_events_preserve_misc_and_cdata_across_character_chunks() {
        let xml = "\u{feff}<?xml version='1.0'?><!--before--><?p  data?><r><![CDATA[raw < &]]><!--inside--></r>";
        let mut parser = Parser::with_document_limits(Events::default(), ParserLimits::default());
        for c in xml.chars() {
            parser.parse(&c.to_string()).unwrap();
        }
        parser.finish().unwrap();
        let events = parser.into_handler();
        assert_eq!(events.comments, ["before", "inside"]);
        assert_eq!(events.sections, ["raw < &"]);
        assert_eq!(events.pis, [("p".to_owned(), " data".to_owned())]);
        assert_eq!(events.declarations, 1);
    }

    #[test]
    fn strict_document_rejects_incomplete_and_malformed_input() {
        for xml in [
            "",
            "<r>",
            "<r/",
            "<r></x>",
            "<r/><s/>",
            "<r/>tail",
            "<r a='x'b='y'/>",
            "<r a='<'/>",
            "<r>&#0;</r>",
            "<r>&#X41;</r>",
            "<r>&#+65;</r>",
            "<r><!--x--y--></r>",
            "<r>]]></r>",
            "<r a='1' a='2'/>",
            "<r></r a='1'>",
            " <r/><?xml version='1.0'?>",
            "<?xml encoding='UTF-8'?><r/>",
            "<1r/>",
            "<r><![CDATA[x",
        ] {
            let mut parser =
                Parser::with_document_limits(Events::default(), ParserLimits::default());
            assert!(
                parser.parse(xml).and_then(|()| parser.finish()).is_err(),
                "must reject {xml:?}"
            );
        }
    }

    #[test]
    fn document_policy_has_distinct_dtd_and_unsupported_errors() {
        for xml in [
            "<!DOCTYPE r><r/>",
            "<!DOCTYPE r SYSTEM 'file:///never-open'><r/>",
        ] {
            let mut parser =
                Parser::with_document_limits(Events::default(), ParserLimits::default());
            assert!(matches!(parser.parse(xml), Err(IksError::DtdForbidden)));
        }
        for xml in [
            "<?xml version='1.1'?><r/>",
            "<?xml version='1.0' encoding='ISO-8859-1'?><r/>",
        ] {
            let mut parser =
                Parser::with_document_limits(Events::default(), ParserLimits::default());
            assert!(matches!(
                parser.parse(xml),
                Err(IksError::UnsupportedXml(_))
            ));
        }
    }

    #[test]
    fn strict_document_markup_tokens_and_eof_are_bounded() {
        for xml in [
            "<r><!--0123456789--></r>",
            "<r><?p 0123456789?></r>",
            "<r a='0123456789'/>",
        ] {
            let limits = ParserLimits {
                max_token_size: 12,
                ..ParserLimits::default()
            };
            let mut parser = Parser::with_document_limits(Events::default(), limits);
            assert!(matches!(
                parser.parse(xml),
                Err(IksError::MaxTokenSizeExceeded)
            ));
        }
        let mut parser = Parser::with_document_limits(Events::default(), ParserLimits::default());
        parser.parse("<r/>").unwrap();
        parser.finish().unwrap();
        assert!(parser.parse("<s/>").is_err());
        assert!(parser.finish().is_err());
    }

    #[test]
    fn strict_names_include_xml_10_fifth_edition_unicode_ranges() {
        let mut parser = Parser::with_document_limits(Events::default(), ParserLimits::default());
        parser.parse("<İksel 变量='x'><𐀀/></İksel>").unwrap();
        parser.finish().unwrap();
    }
}
