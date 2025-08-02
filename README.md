# iksemel-rs

A fast, memory-safe Rust implementation of the [iksemel](https://github.com/meduketto/iksemel) library, offering comprehensive XML parsing and modern XMPP (Jabber) core protocol capabilities.

## Overview

`iksemel-rs` faithfully provides the complete capabilities of original C `iksemel` with modern Rust safety, high throughput, and zero memory vulnerabilities:
- **XML Engine**: Both high-throughput SAX streaming events and DOM tree manipulation, with numeric/character entity resolution and CDATA handling.
- **Fast XML Streaming Writer**: Zero-allocation byte writer (`XmlWriter`) with direct entity escaping and configurable pretty-printing.
- **RFC 6122 / RFC 7622 JID Engine**: Parse, validate, compare, and manipulate Jabber Identifiers (bare JID, full JID, domain-only).
- **Incremental XMPP Stream Parser**: Ingest continuous `<stream:stream>` XML data chunk-by-chunk without losing state across arbitrary network packet boundaries.
- **Stanza Filtering & Dispatch**: Flexible rule builder matching stanzas by type (`iq`, `message`, `presence`), ID, sender JID, or XML namespace.
- **Network Transport & StartTLS**: Synchronous TCP transport with configurable timeouts and RFC 6120 StartTLS upgrade via `native-tls`.
- **SASL & Non-SASL Authentication**: Support for SASL PLAIN, resource binding, session establishment, and legacy Non-SASL SHA-1 digest authentication.
- **Roster Management (RFC 6121)**: Query, modify, backup, and restore XMPP contact rosters to/from server or local XML storage.
- **Command-Line Tools**: High-performance CLI utilities (`ikslint`, `iksperf`, `iksroster`).

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
iksemel = "0.2.0"
```

---

## Usage Examples

### 1. DOM XML Parsing and Navigation

```rust
use iksemel::{DomParser, IksNode};

fn main() -> iksemel::Result<()> {
    let xml = r#"<message from='alice@example.com' to='bob@example.com'>
        <body>Hello World!</body>
    </message>"#;

    let dom = DomParser::parse_str(xml)?;
    let root = dom.borrow();

    assert_eq!(root.name(), Some("message"));
    assert_eq!(root.find_attrib("from"), Some("alice@example.com"));

    if let Some(body) = root.find_cdata("body") {
        println!("Message text: {}", body);
    }

    Ok(())
}
```

### 2. High-Performance XML Streaming Serialization

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

### 3. JID Parsing and Comparison

```rust
use iksemel::Jid;

fn main() -> iksemel::Result<()> {
    let jid = Jid::new("alice@example.com/mobile")?;

    assert_eq!(jid.node(), Some("alice"));
    assert_eq!(jid.domain(), "example.com");
    assert_eq!(jid.resource(), Some("mobile"));
    assert_eq!(jid.bare(), "alice@example.com");

    let server_jid = Jid::new("example.com")?;
    assert!(jid.matches(&server_jid)); // Same domain match

    Ok(())
}
```

### 4. Incremental XMPP Stream Parsing & Stanza Routing

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

### 5. Client Connection, StartTLS, and Roster Fetch

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
1. **Branch-Free Lookup Tables**: 256-byte static lookup tables classify XML element and attribute name characters (`NAME_CHAR_TABLE`, `WHITESPACE_TABLE`) without branch mispredictions.
2. **Fast-Path Character Scanning**: CDATA and comment scanner bypasses UTF-8 decoding loops for contiguous ASCII slices up to special delimiters (`<`, `&`, `-->`).
3. **Optimized Memory Footprint**: Small, pre-allocated vectors for node attributes and children reduce heap fragmentation during deep tree construction.
4. **Buffered Streaming Writer**: `XmlWriter` writes directly to any `std::io::Write` buffer with zero intermediate `String` allocations.

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
cargo test --all-targets
```

Check code quality with Clippy:
```bash
cargo clippy --all-targets
```

---

## Authors & License

- **Original C Implementation**: Gurer Ozen ([iksemel](https://github.com/meduketto/iksemel))
- **Rust Implementation**: Süleyman Poyraz <zaryob.dev@gmail.com>

Licensed under the [GNU Lesser General Public License v2.1](LICENSE).