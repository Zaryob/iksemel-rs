# iksemel-rs

[![License](https://img.shields.io/badge/license-LGPL--2.1-blue.svg)](LICENSE)
[![Safety](https://img.shields.io/badge/unsafe-forbidden-success.svg)](src/lib.rs)
[![CI](https://github.com/Zaryob/iksemel-rs/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/Zaryob/iksemel-rs/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.4.0-orange.svg)](Cargo.toml)

A Rust implementation inspired by [Gürer Özen's iksemel](https://github.com/meduketto/iksemel), providing XML parsing and XMPP (Jabber) protocol helpers. This repository is separate from [meduketto/iksemel-rust](https://github.com/meduketto/iksemel-rust).

## Overview

`iksemel-rs` implements the following APIs. See [validation scope](docs/VALIDATION.md), [security limitations](SECURITY.md) and [bounded fuzzing](fuzz/README.md) before making production or interoperability assumptions.
- **Library forbids unsafe code**: `#![forbid(unsafe_code)]` applies to this library, not to all transitive dependencies or platform TLS code.
- **XML engine**: SAX streaming events and a mutable DOM tree parser.
- **Resource limits**: `ParserLimits` bounds nesting, entity processing, attributes and token sizes. Applications still need input-size, time and memory budgets; tests do not prove complete DoS resistance.
- **Path & query selectors**: Fluent hierarchical queries (`find_path(&["query", "item"])`) and pre-parsed CSS/XPath-like selectors (`select("entry[status=published]/title")`).
- **W3C XML Namespace Resolution**: Automatic prefix splitting (`prefix()`, `local_name()`) and zero-allocation iterative scoped namespace URI inheritance (`namespace_uri()`).
- **XML streaming writer**: `XmlWriter` writes to an existing output buffer with entity escaping and pretty-printing.
- **Borrowing string escaping**: `escape_cow` and `unescape_cow` return borrowed `Cow` where transformation is unnecessary.
- **Non-blocking Tokio Transport (`AsyncConnection`)**: Fully asynchronous XMPP network client over Tokio `TcpStream` and `tokio-native-tls`.
- **Async Stream Split (`split`)**: Decouple connections into concurrent thread-safe `AsyncSender` and `AsyncReceiver` halves.
- **SASL SCRAM (RFC 5802 / RFC 7677)**: Modern `SCRAM-SHA-1` and `SCRAM-SHA-256` client authentication with PBKDF2 key derivation.
- **XEP-0198 (Stream Management)**: Stream reliability, sequence counting (`inbound_h` / `outbound_h`), unacked stanza queue, and session resumption.
- **XMPP Extension Protocols (XEPs)**:
  - **XEP-0004 (Data Forms)**: Forms, submits, cancels, results, options, and tabular reporting (`DataForm`, `FormField`).
  - **XEP-0030 (Service Discovery)**: Queries and response parsers for `disco#info` and `disco#items`.
  - **XEP-0045 (Multi-User Chat)**: Room join/leave, presence detection, and status codes (`build_muc_join`, `build_muc_leave`).
  - **XEP-0060 (Publish-Subscribe)**: Stanza builders for publishing items, subscriptions, and event payload parsing.
  - **XEP-0085 (Chat State Notifications)**: Chat states (`Active`, `Composing`, `Paused`, `Inactive`, `Gone`).
  - **XEP-0199 (XMPP Ping)**: Ping builder (`build_ping`), detector (`is_ping`), and Pong responder (`build_pong`).
  - **XEP-0280 (Message Carbons)**: Multi-device sync (`build_carbons_enable`, `wrap_carbon_sent`, `extract_carbon`).
  - **XEP-0313 (Message Archive Management)**: Historical chat queries (`MamQuery`) with RSM pagination (`max`, `after`, `before`) and `<fin>` tracking.
- **RFC 6122 / RFC 7622 JID Engine**: Parse, validate, compare, and manipulate Jabber Identifiers (bare JID, full JID, domain-only).
- **Incremental XMPP Stream Parser**: Ingest continuous `<stream:stream>` XML data chunk-by-chunk without losing state across network packet boundaries.
- **Stanza Filtering & Dispatch**: Flexible rule builder matching stanzas by type (`iq`, `message`, `presence`), ID, sender JID, or XML namespace.
- **Network Transport & StartTLS**: Synchronous and asynchronous TCP transport with RFC 6120 StartTLS upgrade.
- **Roster Management (RFC 6121)**: Query, modify, backup, and restore XMPP contact rosters to/from server or local XML storage.
- **Optional Serde Integration**: First-class `Serialize` and `Deserialize` support for `Jid`, `IksNode`, and `Roster` via `features = ["serde"]`.
- **Command-Line Tools & Profiling**: High-performance CLI utilities (`ikslint`, `iksperf`, `iksroster`) with JSON telemetry profiling.
- **Production Examples**: Comprehensive examples in `examples/` (`async_xmpp_bot.rs`, `dom_selectors_and_streaming.rs`).

---

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
iksemel = "0.4.0"

# Or enable optional Serde support:
# iksemel = { version = "0.4.0", features = ["serde"] }
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

### 4. SASL SCRAM Authentication, Stream Management (XEP-0198), and Stream Split

```rust
use iksemel::{
    authenticate_scram_sha256_async, bind_resource_async, AsyncConnection, IksNode,
};

#[tokio::main]
async fn main() -> iksemel::Result<()> {
    let mut conn = AsyncConnection::connect("chat.example.com", 5222, "example.com", None).await?;
    conn.start_stream().await?;
    let _features = conn.recv_stanza().await?;

    // Authenticate with modern SCRAM-SHA-256 (RFC 7677 / RFC 5802)
    authenticate_scram_sha256_async(&mut conn, "user", "secretpassword", None).await?;
    let _post_auth = conn.recv_stanza().await?;
    let _jid = bind_resource_async(&mut conn, Some("scram-bot")).await?;

    // Enable XEP-0198 Stream Management
    let sm_res = conn.enable_stream_management(true, Some(300)).await?;
    println!("Stream management enabled, session id: {:?}", sm_res.id);

    // Decouple connection into concurrent lock-free sender and receiver halves
    let (sender, mut receiver) = conn.split()?;

    tokio::spawn(async move {
        let presence = IksNode::new_tag("presence");
        sender.send_stanza(&presence).await.unwrap();
    });

    let incoming = receiver.recv_stanza().await?;
    println!("Received incoming stanza: {:?}", incoming.name());
    Ok(())
}
```

### 5. XEP-0004 Data Forms and XEP-0045 Multi-User Chat

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

### 6. Security Limits & DoS Protection (Hardened Parser)

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

### 7. High-Performance XML Streaming Serialization

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

Run the existing local harnesses from a checkout:

```sh
cargo test --all-features --locked
cargo run --release --locked --bin iksperf -- --synthetic-kb 1024 --iterations 5
cargo bench --locked --bench comparison
```

On the first checkout, run `cargo generate-lockfile` and retain that file with the result. Record the source commit, dirty diff, compiler, OS, CPU, payload bytes, parameters and raw output. [The validation record](docs/VALIDATION.md) contains an actual local run and its limits.

The comparison harness is a small custom executable, not Criterion. Its SAX path now fails on parse errors. It compares **different work**: push SAX callbacks versus pull events, mutable DOM versus borrowed read-only arena, and writing an existing DOM versus parsing and writing a stream. Its byte count uses 1024² (MiB), including where the legacy `iksperf` output labels it MB. Best-of-five timings are exploratory, not a statistical performance guarantee.

Selectors traverse an already-parsed tree; input-size-normalized selector throughput is not XML parsing throughput. The previous fixed throughput table and “3x faster” serialization claim have been removed because their workloads and retained environment evidence do not establish equivalent-work comparisons.

XMPP tests exercise implemented helpers and mocked/local transports. Full server interoperability, XEP conformance and C ABI/behavior compatibility have not been established. There is no claim that every XML/XMPP feature is complete.

---

## Command-Line Tools

The crate builds three binaries:

### `ikslint` - XML Syntax and Structure Validator
Fast linting and statistics reporting over XML documents:
```bash
cargo run --bin ikslint -- --stats --histogram document.xml
```

### `iksperf` - Performance Benchmark & Telemetry Suite
Benchmarks SAX parsing, DOM building, XML serialization, SCRAM crypto, and escaping throughput:
```bash
# Run benchmark on synthetic 1MB payload with latency percentiles (P50, P95)
cargo run --release --bin iksperf -- --synthetic-kb 1024 --iterations 5

# Export machine-readable JSON metrics for CI regression tracking
cargo run --release --bin iksperf -- --synthetic-kb 1024 --test all --json

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

## References

- **[`iks`](https://crates.io/crates/iks) by Gürer Özen**: A related Rust implementation of iksemel for XML parsing and Jabber/XMPP. See the [source repository](https://github.com/meduketto/iksemel-rust) and [API documentation](https://docs.rs/iks).

---

## Authors & License

- **Original C Implementation**: Gurer Ozen ([iksemel](https://github.com/meduketto/iksemel))
- **Rust Implementation**: Süleyman Poyraz <zaryob.dev@gmail.com>

Licensed under the [GNU Lesser General Public License v2.1](LICENSE).
