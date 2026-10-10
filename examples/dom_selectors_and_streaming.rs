//! Demonstrates advanced XML processing with `iksemel-rs`:
//! - Hardened parsing with `ParserLimits`
//! - XPath/CSS-like query selectors (`select`, `select_first`)
//! - Namespace URI scoping and prefix resolution
//! - Zero-allocation escaping (`escape_cow`, `unescape_cow`)
//! - Pretty-printed streaming directly to std::io::stdout via `XmlWriter`

use iksemel::{DomParser, IksNode, ParserLimits, Result, XmlWriter, escape_cow};
use std::io::stdout;

fn main() -> Result<()> {
    let xml_feed = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:dc="http://purl.org/dc/elements/1.1/">
    <title>Rust Engineering Blog</title>
    <updated>2026-02-11T10:00:00Z</updated>
    <entry status="published" type="article">
        <id>urn:uuid:12345</id>
        <title>Safe and Concurrent XMPP in Rust</title>
        <summary>A deep dive into zero-copy parsing, RFC 5802 SCRAM, and Tokio channels.</summary>
        <dc:creator>Süleyman Poyraz</dc:creator>
        <category term="rust"/>
        <category term="xmpp"/>
    </entry>
    <entry status="draft" type="notes">
        <id>urn:uuid:67890</id>
        <title>Draft: Microsecond XML DOM Selectors</title>
        <summary>Benchmark notes on parser limits and allocation strategies.</summary>
        <dc:creator>Rust Team</dc:creator>
        <category term="performance"/>
    </entry>
</feed>"#;

    println!("=== 1. Hardened DOM Parsing with Security Limits ===");
    let limits = ParserLimits {
        max_depth: 32,
        max_entity_expansions: 100,
        max_attributes: 64,
        max_token_size: 1024 * 1024,
    };
    let doc = DomParser::parse_str_with_limits(xml_feed, limits)?;
    let root = doc.borrow();

    println!("Feed Root Tag: <{}>", root.name().unwrap());
    println!("Feed Title: {}", root.find_path_text(&["title"]).unwrap());

    println!("\n=== 2. XPath/CSS-Style Query Selectors ===");
    // Find all published entries
    let published_entries = root.select("entry[status=published]");
    println!("Found {} published entry:", published_entries.len());
    for entry_rc in &published_entries {
        let entry = entry_rc.borrow();
        let title = entry.find_path_text(&["title"]).unwrap_or_default();
        let summary = entry.find_path_text(&["summary"]).unwrap_or_default();
        println!(" - Title:   {}", title);
        println!("   Summary: {}", summary);
    }

    // Select specific nested field directly with query
    if let Some(creator_rc) = root.select_first("entry[status=published]/dc:creator") {
        let creator = creator_rc.borrow();
        println!(" - Creator: {}", creator.text());
        println!("   Prefix:  {:?}", creator.prefix());
        println!("   Local:   {:?}", creator.local_name());
        println!("   NS URI:  {:?}", creator.namespace_uri());
    }

    println!("\n=== 3. Zero-Allocation Escape Demonstration ===");
    let raw_clean = "Alphanumeric string without special characters";
    let cow_clean = escape_cow(raw_clean);
    println!(
        "Clean text is borrowed: {}",
        matches!(cow_clean, std::borrow::Cow::Borrowed(_))
    );

    let raw_special = "<record priority=\"high\" & active='true'>";
    let cow_special = escape_cow(raw_special);
    println!("Special text escaped:   {}", cow_special);

    println!("\n=== 4. Dynamic Node Creation and Streaming Serialization ===");
    let mut stats_node = IksNode::new_tag("analysis");
    stats_node.add_attribute("generated", "2026-02-11");
    stats_node.add_attribute("engine", "iksemel-rs");

    let mut metric1 = IksNode::new_tag("metric");
    metric1.add_attribute("name", "entries_parsed");
    metric1.insert_cdata("2");
    stats_node.add_child(metric1);

    let mut metric2 = IksNode::new_tag("metric");
    metric2.add_attribute("name", "unsafe_code_blocks");
    metric2.insert_cdata("0");
    stats_node.add_child(metric2);

    println!("Streaming output formatted with XmlWriter (2-space indent):");
    let mut writer = XmlWriter::new(stdout());
    writer.set_pretty(true, 2);
    writer.write_node(&stats_node)?;

    Ok(())
}
