# iksemel-rs

A fast, memory-safe Rust implementation of the [iksemel](https://github.com/meduketto/iksemel) library, offering comprehensive XML parsing and modern XMPP (Jabber) core protocol capabilities.

## Overview

`iksemel-rs` faithfully provides the complete capabilities of original C `iksemel` with modern Rust safety and ergonomics:
- **XML Engine**: Both SAX streaming events and DOM tree manipulation, with numeric/character entity resolution and CDATA handling.
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

### 2. JID Parsing and Comparison

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

### 3. Incremental XMPP Stream Parsing & Stanza Routing

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

### 4. Client Connection, StartTLS, and Roster Fetch

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
cargo run --bin iksperf -- --input document.xml --block-size 8192 --test all
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