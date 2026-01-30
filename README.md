# iksemel-rs

[![License](https://img.shields.io/badge/license-LGPL--2.1-blue.svg)](LICENSE)
[![Safety](https://img.shields.io/badge/unsafe-forbidden-success.svg)](src/lib.rs)
[![CI](https://img.shields.io/badge/CI-passing-success.svg)](.github/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.3.1-orange.svg)](Cargo.toml)

A fast, 100% safe Rust implementation of the [iksemel](https://github.com/meduketto/iksemel) library, offering comprehensive XML parsing and modern XMPP (Jabber) core protocol capabilities.

## Overview

`iksemel-rs` provides complete XML and XMPP capabilities with high throughput, modern Rust ergonomics, and strict memory safety guarantees:
- **100% Safe Rust**: Built with `#![forbid(unsafe_code)]` — zero `unsafe` blocks, zero raw pointer manipulation.
- **High-Throughput XML Engine**: Fast SAX streaming event parser (~230+ MB/s) and bidirectional linked DOM tree parser (~160+ MB/s).
- **Security & DoS Protection**: Hardened parser guardrails (`ParserLimits`) preventing Billion Laughs entity expansion attacks, quadratic token growth, and stack overflow recursion.
- **Ergonomic Path & Query Selectors**: Fluent hierarchical queries (`find_path(&["query", "item"])`) and CSS/XPath-like selectors (`select("entry[status=published]/title")`).
- **W3C XML Namespace Resolution**: Automatic prefix splitting (`prefix()`, `local_name()`) and scoped namespace URI inheritance (`namespace_uri()`).
- **Fast XML Streaming Writer**: Zero-allocation byte writer (`XmlWriter`, ~1.4 GB/s) with direct entity escaping and pretty-printing.
- **Non-blocking Tokio Transport (`AsyncConnection`)**: Fully asynchronous XMPP network client over Tokio `TcpStream` and `tokio-native-tls`.
- **XMPP Extension Protocols (XEPs)**:
  - **XEP-0004 (Data Forms)**: Forms, submits, cancels, results, options, and tabular reporting (`DataForm`, `FormField`).
  - **XEP-0030 (Service Discovery)**: Queries and response parsers for `disco#info` and `disco#items`.
  - **XEP-0045 (Multi-User Chat)**: Room join/leave, presence detection, and status codes (`build_muc_join`, `build_muc_leave`).
  - **XEP-0060 (Publish-Subscribe)**: Stanza builders for publishing items, subscriptions, and event payload parsing.
  - **XEP-0085 (Chat State Notifications)**: Chat states (`Active`, `Composing`, `Paused`, `Inactive`, `Gone`).
  - **XEP-0199 (XMPP Ping)**: Ping builder (`build_ping`), detector (`is_ping`), and Pong responder (`build_pong`).
- **RFC 6122 / RFC 7622 JID Engine**: Parse, validate, compare, and manipulate Jabber Identifiers (bare JID, full JID, domain-only).
- **Incremental XMPP Stream Parser**: Ingest continuous `<stream:stream>` XML data chunk-by-chunk without losing state across network packet boundaries.
- **Stanza Filtering & Dispatch**: Flexible rule builder matching stanzas by type (`iq`, `message`, `presence`), ID, sender JID, or XML namespace.
- **Network Transport & StartTLS**: Synchronous and asynchronous TCP transport with RFC 6120 StartTLS upgrade.
- **SASL & Non-SASL Authentication**: Support for SASL PLAIN, resource binding, session establishment, and legacy Non-SASL SHA-1 digest authentication.
- **Roster Management (RFC 6121)**: Query, modify, backup, and restore XMPP contact rosters to/from server or local XML storage.
- **Optional Serde Integration**: First-class `Serialize` and `Deserialize` support for `Jid`, `IksNode`, and `Roster` via `features = ["serde"]`.
- **Command-Line Tools**: High-performance CLI utilities (`ikslint`, `iksperf`, `iksroster`).

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
iksemel = "0.3.1"

# Or enable optional Serde support:
# iksemel = { version = "0.3.1", features = ["serde"] }
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

### 2. Asynchronous Tokio XMPP Client

```rust
use iksemel::{AsyncConnection, authenticate_plain_async, bind_resource_async, IksNode};
use std::time::Duration;

#[tokio::main]
async fn main() -> iksemel::Result<()> {
    let mut conn = AsyncConnection::connect("example.com", 5222, "example.com", Some(Duration::from_secs(10))).await?;

    conn.start_stream().await?;
    let _features = conn.recv_stanza().await?;

    // Asynchronous StartTLS upgrade
    conn.start_tls().await?;
    let _tls_features = conn.recv_stanza().await?;

    // Asynchronous SASL PLAIN authentication
    authenticate_plain_async(&mut conn, "user", "secretpassword", None).await?;
    let _auth_features = conn.recv_stanza().await?;

    // Resource binding
    let bound_jid = bind_resource_async(&mut conn, Some("tokio-bot")).await?;
    println!("Connected as: {}", bound_jid);

    // Send chat message
    let mut msg = IksNode::new_tag("message");
    msg.add_attribute("to", "friend@example.com");
    msg.add_attribute("type", "chat");
    let mut body = IksNode::new_tag("body");
    body.insert_cdata("Hello from async Tokio!");
    msg.add_child(body);

    conn.send_stanza(&msg).await?;
    conn.close().await?;
    Ok(())
}
```

### 3. XEP-0199 Ping and XEP-0030 Service Discovery

```rust
use iksemel::{build_ping, is_ping, build_pong, build_disco_info_query, parse_disco_info_response, DomParser};

fn main() -> iksemel::Result<()> {
    // 1. Ping / Pong
    let ping_stanza = build_ping("ping_1", Some("server.example.com"));
    assert!(is_ping(&ping_stanza));

    let pong = build_pong(&ping_stanza)?;
    assert_eq!(pong.find_attrib("type"), Some("result"));

    // 2. Service Discovery
    let disco_query = build_disco_info_query("disco_1", "conference.example.org", None);
    assert_eq!(disco_query.find_attrib("type"), Some("get"));

    Ok(())
}
```

### 4. XEP-0004 Data Forms and XEP-0045 Multi-User Chat

```rust
use iksemel::{DataForm, DataFormType, FormField, FieldType, build_muc_join, is_muc_presence};

fn main() -> iksemel::Result<()> {
    // 1. Build an XEP-0004 Configuration Form
    let mut form = DataForm::new(DataFormType::Form)
        .with_title("Bot Settings");
    form.add_field(
        FormField::new("bot_name")
            .with_type(FieldType::TextSingle)
            .with_label("Bot Handle")
            .with_value("FerrisBot")
    );
    form.add_field(
        FormField::new("public")
            .with_type(FieldType::Boolean)
            .with_value("1")
    );

    let form_node = form.to_node();
    let parsed_form = DataForm::from_node(&form_node)?;
    assert_eq!(parsed_form.get_value("bot_name"), Some("FerrisBot"));

    // 2. Build an XEP-0045 MUC Join Presence
    let join_presence = build_muc_join("rust-room@conference.example.org/ferris", Some("secret"), Some(50));
    assert!(is_muc_presence(&join_presence));

    Ok(())
}
```

### 5. Security Limits & DoS Protection (Hardened Parser)

```rust
use iksemel::{DomParser, ParserLimits, IksError};

fn main() {
    let limits = ParserLimits {
        max_depth: 32,                 // Maximum nesting depth
        max_entity_expansions: 100,    // Protection against Billion Laughs
        max_attributes: 128,           // Maximum attributes per tag
        max_token_size: 1024 * 1024,   // 1 MB max token size
    };

    let xml = "<root><child>Secure data</child></root>";
    let result = DomParser::parse_str_with_limits(xml, limits);
    assert!(result.is_ok());
}
```

### 6. High-Performance XML Streaming Serialization

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

---

## Performance & Benchmarks

`iksemel-rs` is engineered for high throughput in both network daemon and embedded environments.

### Benchmark Results (Apple Silicon / Release Mode)

Measured using `cargo run --release --bin iksperf -- --synthetic-kb 1024 --iterations 5`:

| Operation | Throughput (MB/s) | Latency (1 MB payload) | Description |
|:---|:---:|:---:|:---|
| **XmlWriter (Stream Buffer)** | **~1,400 MB/s** | **0.71 ms** | Direct byte writing with escaping |
| **SHA-1 Digest** | **~1,300 MB/s** | **0.76 ms** | Fast RFC 3174 cryptographic hash |
| **DOM Path & Selectors** | **~620 MB/s** | **1.61 ms** | Hierarchical path traversal & CSS/XPath filtering |
| **DOM to_string() (Alloc)** | **~260 MB/s** | **3.82 ms** | In-memory String DOM serialization |
| **SAX Parser (Streaming)** | **~233 MB/s** | **4.29 ms** | Chunked streaming event callback parser |
| **DOM Parser (Tree Build)** | **~162 MB/s** | **6.16 ms** | Full bidirectional linked DOM tree construction |

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