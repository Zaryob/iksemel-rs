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

use crate::{IksNode, IksType};
use std::io::{self, Write};

/// A streaming, zero-allocation XML writer.
pub struct XmlWriter<W: Write> {
    writer: W,
    pretty: bool,
    indent_size: usize,
    depth: usize,
}

impl<W: Write> XmlWriter<W> {
    /// Creates a new `XmlWriter` wrapping the given write stream.
    pub fn new(writer: W) -> Self {
        XmlWriter {
            writer,
            pretty: false,
            indent_size: 2,
            depth: 0,
        }
    }

    /// Enables or disables pretty printing with indentation.
    pub fn set_pretty(&mut self, pretty: bool, indent_size: usize) {
        self.pretty = pretty;
        self.indent_size = indent_size;
    }

    /// Writes an entire DOM `IksNode` tree to the underlying stream.
    pub fn write_node(&mut self, node: &IksNode) -> io::Result<()> {
        match node.node_type() {
            IksType::Tag => {
                let name = node.name().unwrap_or("tag");
                let has_children = node.has_children();
                let has_tag_children = self.pretty
                    && node
                        .children()
                        .iter()
                        .any(|c| c.borrow().node_type() == IksType::Tag);
                let content = node.content();

                if self.pretty && self.depth > 0 {
                    self.write_indent()?;
                }

                self.writer.write_all(b"<")?;
                self.writer.write_all(name.as_bytes())?;

                for (attr_name, attr_val) in node.attributes() {
                    self.writer.write_all(b" ")?;
                    self.writer.write_all(attr_name.as_bytes())?;
                    self.writer.write_all(b"=\"")?;
                    crate::escape::write_escaped(&mut self.writer, attr_val)?;
                    self.writer.write_all(b"\"")?;
                }

                if !has_children && content.is_none() {
                    self.writer.write_all(b"/>")?;
                    if self.pretty {
                        self.writer.write_all(b"\n")?;
                    }
                    return Ok(());
                }

                self.writer.write_all(b">")?;

                if let Some(text) = content {
                    crate::escape::write_escaped(&mut self.writer, text)?;
                }

                if has_children {
                    if has_tag_children {
                        self.writer.write_all(b"\n")?;
                        self.depth += 1;
                        for child in node.children() {
                            self.write_node(&child.borrow())?;
                        }
                        self.depth -= 1;
                        if self.pretty {
                            self.write_indent()?;
                        }
                    } else {
                        for child in node.children() {
                            self.write_node(&child.borrow())?;
                        }
                    }
                }

                self.writer.write_all(b"</")?;
                self.writer.write_all(name.as_bytes())?;
                self.writer.write_all(b">")?;

                if self.pretty {
                    self.writer.write_all(b"\n")?;
                }
            }
            IksType::CData => {
                if let Some(text) = node.content() {
                    if self.pretty && self.depth > 0 {
                        // Pretty modda sırf biçimlendirme boşluğu olan düğümler
                        // atlanır; bu, çıktının idempotent (ve dolayısıyla
                        // round-trip'te kararlı) kalmasını sağlar. `trim()`
                        // burada **yalnızca bir yüklem** olarak kullanılır:
                        // yazılan baytlar kırpılmaz.
                        if !text.trim().is_empty() {
                            crate::escape::write_escaped(&mut self.writer, text)?;
                        }
                    } else {
                        // Yoğun mod hiçbir şeyi atlamaz ve hiçbir şeyi kırpmaz.
                        crate::escape::write_escaped(&mut self.writer, text)?;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Flushes the underlying stream.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn write_indent(&mut self) -> io::Result<()> {
        let total_spaces = self.depth * self.indent_size;
        for _ in 0..total_spaces {
            self.writer.write_all(b" ")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::DomParser;

    #[test]
    fn test_xml_writer_compact() {
        let mut root = IksNode::new_tag("root");
        root.add_attribute("id", "1 & 2");
        let mut child = IksNode::new_tag("child");
        child.insert_cdata("Hello <world>");
        root.add_child(child);

        let mut buf = Vec::new();
        let mut writer = XmlWriter::new(&mut buf);
        writer.write_node(&root).unwrap();

        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            "<root id=\"1 &amp; 2\"><child>Hello &lt;world&gt;</child></root>"
        );
    }

    #[test]
    fn test_xml_writer_pretty() {
        let mut root = IksNode::new_tag("message");
        root.add_attribute("type", "chat");
        let mut body = IksNode::new_tag("body");
        body.set_content("Hello!");
        root.add_child(body);

        let mut buf = Vec::new();
        let mut writer = XmlWriter::new(&mut buf);
        writer.set_pretty(true, 2);
        writer.write_node(&root).unwrap();

        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("<message type=\"chat\">\n"));
        assert!(output.contains("  <body>Hello!</body>\n"));
    }

    /// Pretty mod düğümler *arasına* boşluk ekleyebilir ama bir düğümün kendi
    /// içeriğini asla değiştiremez. `" ab "` kırpılmamalıdır.
    #[test]
    fn pretty_mode_does_not_trim_cdata_content() {
        let mut root = IksNode::new_tag("r");
        let mut cdata = IksNode::new(IksType::CData);
        cdata.set_content(" ab ");
        root.add_child(cdata);

        let mut buf = Vec::new();
        let mut writer = XmlWriter::new(&mut buf);
        writer.set_pretty(true, 2);
        writer.write_node(&root).unwrap();

        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains(" ab "), "içerik kırpılmamalı: {:?}", out);
    }

    /// Sırf boşluk olan CDATA düğümleri pretty modda atlanır — böylece
    /// `parse → pretty → parse` döngüsü her turda boşluk biriktirmez. Yoğun
    /// modda hiçbir şey atlanmaz.
    #[test]
    fn pretty_mode_skips_whitespace_only_cdata_but_compact_does_not() {
        let mut root = IksNode::new_tag("r");
        let mut ws = IksNode::new(IksType::CData);
        ws.set_content("\n  ");
        root.add_child(ws);

        let mut compact = Vec::new();
        let mut w1 = XmlWriter::new(&mut compact);
        w1.write_node(&root).unwrap();
        assert_eq!(String::from_utf8(compact).unwrap(), "<r>\n  </r>");

        let mut pretty = Vec::new();
        let mut w2 = XmlWriter::new(&mut pretty);
        w2.set_pretty(true, 2);
        w2.write_node(&root).unwrap();
        let pretty_out = String::from_utf8(pretty).unwrap();
        assert!(
            !pretty_out.contains("\n  \n"),
            "boşluk-only düğüm atlanmalı: {:?}",
            pretty_out
        );
    }

    /// Pretty çıktı yeniden ayrıştırıldığında boşluk her turda büyümemeli.
    #[test]
    fn pretty_output_is_stable_across_round_trips() {
        let xml = "<r><a>bir</a><b>iki</b></r>";
        let node = DomParser::parse_str(xml).unwrap();
        let once = node.borrow().to_pretty_string(2);
        let re = DomParser::parse_str(&once).unwrap();
        let twice = re.borrow().to_pretty_string(2);
        assert_eq!(once, twice, "pretty çıktı idempotent olmalı");
    }
}
