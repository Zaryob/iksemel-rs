# iksemel-rs

A fast, 100% safe Rust implementation of the [iksemel](https://github.com/meduketto/iksemel) library, offering comprehensive XML parsing and modern XMPP (Jabber) core protocol capabilities.

## Overview

`iksemel-rs` provides complete XML and XMPP capabilities with high throughput, modern Rust ergonomics, and strict memory safety guarantees:
- **100% Safe Rust**: Built with `#![forbid(unsafe_code)]` — zero `unsafe` blocks, zero raw pointer manipulation.
- **High-Throughput XML Engine**: Fast SAX streaming event parser (~230+ MB/s) and bidirectional linked DOM tree parser (~160+ MB/s).
- **Security & DoS Protection**: Hardened parser guardrails (`ParserLimits`) preventing Billion Laughs entity expansion attacks, quadratic token growth, and stack overflow recursion.
- **Ergonomic Path & Query Selectors**: Fluent hierarchical queries (`find_path(&["query", "item"])`) and CSS/XPath-like selectors (`select("entry[status=published]/title")`).
- **W3C XML Namespace Resolution**: Automatic prefix splitting (`prefix()`, `local_name()`) and scoped namespace URI inheritance (`namespace_uri()`).
- **Fast XML Streaming Writer**: Zero-allocation byte writer (`XmlWriter`, ~1.4 GB/s) with direct entity escaping and pretty-printing.
- **RFC 6122 / RFC 7622 JID Engine**: Parse, validate, compare, and manipulate Jabber Identifiers (bare JID, full JID, domain-only).
- **Incremental XMPP Stream Parser**: Ingest continuous `<stream:stream>` XML data chunk-by-chunk without losing state across network packet boundaries.
- **Stanza Filtering & Dispatch**: Flexible rule builder matching stanzas by type (`iq`, `message`, `presence`), ID, sender JID, or XML namespace.
- **Network Transport & StartTLS**: Synchronous TCP transport with configurable timeouts and RFC 6120 StartTLS upgrade via `native-tls`.
- **SASL & Non-SASL Authentication**: Support for SASL PLAIN, resource binding, session establishment, and legacy Non-SASL SHA-1 digest authentication.
- **Roster Management (RFC 6121)**: Query, modify, backup, and restore XMPP contact rosters to/from server or local XML storage.
- **Optional Serde Integration**: First-class `Serialize` and `Deserialize` support for `Jid`, `IksNode`, and `Roster` via `features = ["serde"]`.
- **Command-Line Tools**: High-performance CLI utilities (`ikslint`, `iksperf`, `iksroster`).

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
iksemel = "0.2.0"

# Or enable optional Serde support:
# iksemel = { version = "0.2.0", features = ["serde"] }
```

---

## Usage Examples

### 1. DOM Parsing, Path Traversal, and Query Selectors

```rust
use iksemel::{DomParser, IksNode};

fn main() -> iksemel::Result<()> {
    let xml = r#"<feed>
        <entry id="1" status="published">
            <title>Rust 2026</title>
            <author><name>Alice</name></author>
        </entry>
        <entry id="2" status="draft">
            <title>Draft Notes</title>
        </entry>
    </feed>"#;

    let dom = DomParser::parse_str(xml)?;
    let root = dom.borrow();

    // 1. Direct path traversal
    if let Some(author_name) = root.find_path_text(&["entry", "author", "name"]) {
        println!("First author: {}", author_name);
    }

    // 2. Query selector with attribute filter
    let published_entries = root.select("entry[status=published]");
    assert_eq!(published_entries.len(), 1);

    // 3. Child tag selector
    let titles = root.select("entry/title");
    for title in titles {
        println!("Title: {}", title.borrow().text());
    }

    Ok(())
}
```

### 2. XML Namespace (XMLNS) Resolution

```rust
use iksemel::DomParser;

fn main() -> iksemel::Result<()> {
    let xml = r#"<stream:stream xmlns="jabber:client" xmlns:stream="http://etherx.jabber.org/streams">
        <message to="alice@example.com">
            <body>Hello!</body>
        </message>
    </stream:stream>"#;

    let doc = DomParser::parse_str(xml)?;
    let root = doc.borrow();

    assert_eq!(root.prefix(), Some("stream"));
    assert_eq!(root.local_name(), Some("stream"));
    assert_eq!(root.namespace_uri(), Some("http://etherx.jabber.org/streams".to_string()));

    let msg = root.find("message").unwrap();
    // Inherited default namespace from parent
    assert_eq!(msg.borrow().namespace_uri(), Some("jabber:client".to_string()));

    Ok(())
}
```

### 3. Security Limits & DoS Protection (Hardened Parser)

```rust
use iksemel::{DomParser, ParserLimits, IksError};

fn main() {
    let limits = ParserLimits {
        max_depth: 32,                 // Maximum nesting depth
        max_entity_expansions: 100,    // Protection against Billion Laughs
        max_attributes: 128,           // Maximum attributes per tag
        max_token_size: 1024 * 1024,   // 1 MB max token size
    };

    let malicious_xml = "<root>&amp;&amp;&amp;&amp;&amp;&amp;</root>";
    let result = DomParser::parse_str_with_limits(malicious_xml, limits);
    // Securely parsed or rejected if limits exceeded
}
```

### 4. High-Performance XML Streaming Serialization

```rust
use iksemel::{IksNode, XmlWriter};
use std::io::stdout;

fn main() -> iksemel::Result<()> {
    let mut root = IksNode::new_tag("iq");
    root.add_attribute("type", "result");
    root.add_attribute("id", "auth_2");

    let mut bind = IksNode::new_tag("bind");
    bind.add_attribute("xmlns", "urn:ietf:params:xml:ns:xmpp-bind");
    root.add_child(bind);

    // Fast compact streaming directly to any std::io::Write target
    root.write_to(&mut stdout())?;

    // Or pretty-printed with configurable indentation
    println!("\nPretty-printed:\n{}", root.to_pretty_string(2));

    Ok(())
}
```

### 5. JID Parsing, Comparison, and Serde Support

```rust
use iksemel::Jid;

fn main() -> iksemel::Result<()> {
    let jid = Jid::new("alice@example.com/mobile")?;

    assert_eq!(jid.node(), Some("alice"));
    assert_eq!(jid.domain(), "example.com");
    assert_eq!(jid.resource(), Some("mobile"));
    assert_eq!(jid.bare(), "alice@example.com");

    let server_jid = Jid::new("example.com")?;
    assert!(jid.matches(&server_jid)); // Domain matching

    #[cfg(feature = "serde")]
    {
        let json = serde_json::to_string(&jid).unwrap();
        assert_eq!(json, "\"alice@example.com/mobile\"");
    }

    Ok(())
}
```

### 6. Incremental XMPP Stream Parsing & Stanza Routing

```rust
use iksemel::{StreamParser, StreamEvent, PacketFilter, RuleBuilder, StanzaType};

fn main() -> iksemel::Result<()> {
    let mut parser = StreamParser::new();
    let mut filter = PacketFilter::new();

    filter.add_rule(
        RuleBuilder::new().with_type(StanzaType::Message).with_subtype("chat"),
        |stanza| {
            println!("Received chat message: {}", stanza.to_string());
            true
        },
    );

    let incoming_chunks = [
        "<stream:stream to='example.com' xmlns='jabber:client' xmlns:stream='http://etherx.jabber.org/streams'>",
        "<message type='chat' from='alice@example.com'>",
        "<body>How are you?</body></message>",
        "</stream:stream>",
    ];

    for chunk in &incoming_chunks {
        let events = parser.parse_chunk(chunk)?;
        for event in events {
            if let StreamEvent::Stanza(stanza) = event {
                filter.dispatch(&stanza);
            }
        }
    }

    Ok(())
}
```

### 7. Client Connection, StartTLS, and Roster Fetch

```rust
use iksemel::{Connection, authenticate_plain, bind_resource, fetch_roster, Jid};
use std::time::Duration;

fn main() -> iksemel::Result<()> {
    let jid = Jid::new("user@example.com/desktop")?;
    let mut conn = Connection::connect("example.com", 5222, "example.com", Some(Duration::from_secs(10)))?;

    // Initial stream header
    conn.start_stream()?;
    let _features = conn.recv_stanza()?;

    // Upgrade to TLS
    conn.start_tls()?;
    let _tls_features = conn.recv_stanza()?;

    // Authenticate with SASL PLAIN
    authenticate_plain(&mut conn, "user", "secretpassword", None)?;
    let _auth_features = conn.recv_stanza()?;

    // Resource binding
    let _bound = bind_resource(&mut conn, Some("desktop"))?;

    // Fetch roster
    let roster = fetch_roster(&mut conn, "roster_1")?;
    for item in &roster.items {
        println!("Contact: {} ({:?})", item.jid.bare(), item.subscription);
    }

    conn.close()?;
    Ok(())
}
```

---

## Performance & Benchmarks

`iksemel-rs` is engineered for high throughput in both network daemon and embedded environments.

### Benchmark Results (Apple Silicon / Release Mode)

Measured using `cargo run --release --bin iksperf -- --synthetic-kb 1024 --iterations 5`:

| Operation | Throughput (MB/s) | Latency (1 MB payload) | Description |
|:---|:---:|:---:|:---|
| **XmlWriter (Stream Buffer)** | **~1,400 MB/s** | **0.71 ms** | Direct byte writing with escaping |
| **SHA-1 Digest** | **~1,300 MB/s** | **0.76 ms** | Fast RFC 3174 cryptographic hash |
| **DOM to_string() (Alloc)** | **~260 MB/s** | **3.82 ms** | In-memory String DOM serialization |
| **SAX Parser (Streaming)** | **~233 MB/s** | **4.29 ms** | Chunked streaming event callback parser |
| **DOM Parser (Tree Build)** | **~162 MB/s** | **6.16 ms** | Full bidirectional linked DOM tree construction |

### Key Architectural Optimizations
1. **Zero Unsafe Code**: Complete memory safety verified by `#![forbid(unsafe_code)]`.
2. **Branch-Free Lookup Tables**: 256-byte static lookup tables classify XML element and attribute name characters (`NAME_CHAR_TABLE`, `WHITESPACE_TABLE`) without branch mispredictions.
3. **Fast-Path Character Scanning**: CDATA and comment scanner bypasses UTF-8 decoding loops for contiguous ASCII slices up to special delimiters (`<`, `&`, `-->`).
4. **Optimized Memory Footprint**: Small, pre-allocated vectors for node attributes and children reduce heap fragmentation during deep tree construction.
5. **Buffered Streaming Writer**: `XmlWriter` writes directly to any `std::io::Write` buffer with zero intermediate `String` allocations.

---

## Command-Line Tools

The crate builds three binaries:

### `ikslint` - XML Syntax and Structure Validator
Fast linting and statistics reporting over XML documents:
```bash
cargo run --bin ikslint -- --stats --histogram document.xml
```

### `iksperf` - Performance Benchmark Suite
Benchmarks SAX parsing, DOM building, XML serialization, and SHA-1 hashing throughput:
```bash
# Run benchmark on synthetic 1MB payload
cargo run --release --bin iksperf -- --synthetic-kb 1024 --iterations 5

# Run benchmark on custom XML document
cargo run --release --bin iksperf -- --input document.xml --block-size 8192 --test all
```

### `iksroster` - XMPP Roster Backup & Restore
Backup or synchronize contact rosters with an XMPP server:
```bash
# Backup roster to file
cargo run --bin iksroster -- --backup user@example.com --secure --file contacts.xml

# Restore / upload roster to server
cargo run --bin iksroster -- --restore user@example.com --secure --file contacts.xml
```

---

## Building & Testing

Run all unit, integration, and doc tests:
```bash
cargo test --all-targets --all-features
```

Check code quality with Clippy:
```bash
cargo clippy --all-targets --all-features
```

---

## Authors & License

- **Original C Implementation**: Gurer Ozen ([iksemel](https://github.com/meduketto/iksemel))
- **Rust Implementation**: Süleyman Poyraz <zaryob.dev@gmail.com>

Licensed under the [GNU Lesser General Public License v2.1](LICENSE).