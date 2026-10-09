# iksemel-rs

[![License](https://img.shields.io/badge/license-LGPL--2.1-blue.svg)](LICENSE)
[![Safety](https://img.shields.io/badge/unsafe-forbidden-success.svg)](src/lib.rs)
[![CI](https://img.shields.io/badge/CI-passing-success.svg)](.github/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.3.3-orange.svg)](Cargo.toml)

A fast, 100% safe Rust implementation of the [iksemel](https://github.com/meduketto/iksemel) library, offering comprehensive XML parsing and modern XMPP (Jabber) core protocol capabilities.

## Overview

`iksemel-rs` provides complete XML and XMPP capabilities with high throughput, modern Rust ergonomics, and strict memory safety guarantees:
- **100% Safe Rust**: Built with `#![forbid(unsafe_code)]` — zero `unsafe` blocks, zero raw pointer manipulation.
- **High-Throughput XML Engine**: Fast SAX streaming event parser (~300+ MB/s) and bidirectional linked DOM tree parser (~160+ MB/s).
- **Security & DoS Protection**: Hardened parser guardrails (`ParserLimits`) preventing Billion Laughs entity expansion attacks, quadratic token growth, and stack overflow recursion.
- **Ergonomic Path & Query Selectors**: Fluent hierarchical queries (`find_path(&["query", "item"])`) and pre-parsed CSS/XPath-like selectors (`select("entry[status=published]/title")`, ~5,300 MB/s).
- **W3C XML Namespace Resolution**: Automatic prefix splitting (`prefix()`, `local_name()`) and zero-allocation iterative scoped namespace URI inheritance (`namespace_uri()`).
- **Fast XML Streaming Writer**: Zero-allocation byte writer (`XmlWriter`, ~1.2 GB/s) with direct entity escaping and pretty-printing.
- **Zero-Allocation String Escaping**: `escape_cow` and `unescape_cow` returning borrowed `Cow` for strings without entities (~1.1 GB/s).
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
iksemel = "0.3.3"

# Or enable optional Serde support:
# iksemel = { version = "0.3.3", features = ["serde"] }
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

`iksemel-rs` is engineered for high throughput in both network daemon and embedded environments with zero-copy optimizations, pre-allocated string buffers, and slice borrowing.

### Benchmark Results (Apple Silicon / Release Mode)

Measured using `cargo run --release --bin iksperf -- --synthetic-kb 1024 --iterations 5`:

```text
================================================================================
 iksemel-rs High-Performance XML & XMPP Profiling Suite
 Payload Size: 1.00 MB (1048671 bytes) | Iterations: 5 | Chunk Size: 4096 bytes
================================================================================
  Benchmark Operation        |       Best |        Avg |  P50 (Med) |        P95 | Throughput
  ---------------------------+------------+------------+------------+------------+-----------
  SAX Parser (Streaming)     |     3.04ms |     3.18ms |     3.19ms |     3.36ms |  314.38 MB/s
  DOM Parser (Tree Build)    |     5.27ms |     5.73ms |     5.56ms |     6.68ms |  174.67 MB/s
  XmlWriter (Stream Buffer)  |   682.50µs |   738.58µs |   689.29µs |   852.83µs | 1354.08 MB/s
  DOM to_string() (Alloc)    |     3.82ms |     3.95ms |     3.91ms |     4.11ms |  252.94 MB/s
  DOM Path & Selectors       |   127.75µs |   162.47µs |   134.58µs |   284.38µs | 6155.69 MB/s
  Zero-Alloc escape_cow()    |   168.58µs |   170.23µs |   168.79µs |   175.96µs | 1259.87 MB/s
  Standard escape() (Alloc)  |   221.58µs |   243.68µs |   248.83µs |   249.96µs |  880.13 MB/s
  SCRAM-SHA-256 Handshake    |   194.24ms |   195.80ms |   195.92ms |   197.59ms |    0.50 MB/s
  PBKDF2-SHA-256 (4096 iter) |    98.21ms |    98.64ms |    98.36ms |    99.76ms |    0.03 MB/s
  XEP Stanza Build/Gen (2k)  |    13.87ms |    14.07ms |    14.05ms |    14.24ms |   47.43 MB/s
  SHA-1 Digest               |   746.42µs |   804.38µs |   771.71µs |   911.38µs | 1243.30 MB/s
================================================================================
```

### Key Performance Characteristics

| Operation | Best Latency | Avg Latency | P50 (Median) | P95 Latency | Throughput | Description |
|:---|:---:|:---:|:---:|:---:|:---:|:---|
| **DOM Path & Selectors** | 127.75 µs | 162.47 µs | 134.58 µs | 284.38 µs | **~6,150 MB/s** | Indexed path traversal (`/root/item/name`) & attribute lookup |
| **XmlWriter (Stream Buffer)** | 682.50 µs | 738.58 µs | 689.29 µs | 852.83 µs | **~1,350 MB/s** | Direct non-allocating streaming serializer with XML entity escaping |
| **Zero-Alloc `escape_cow()`** | 168.58 µs | 170.23 µs | 168.79 µs | 175.96 µs | **~1,260 MB/s** | Slice-based string escaping borrowing untouched string slices |
| **SHA-1 Digest** | 746.42 µs | 804.38 µs | 771.71 µs | 911.38 µs | **~1,240 MB/s** | RFC 3174 cryptographic hashing |
| **Standard `escape()` (Alloc)** | 221.58 µs | 243.68 µs | 248.83 µs | 249.96 µs | **~880 MB/s** | Standard heap-allocating XML character escaping |
| **SAX Parser (Streaming)** | 3.04 ms | 3.18 ms | 3.19 ms | 3.36 ms | **~315 MB/s** | Chunked event-driven streaming parser (push/pull tokenization) |
| **DOM `to_string()` (Alloc)** | 3.82 ms | 3.95 ms | 3.91 ms | 4.11 ms | **~253 MB/s** | Direct memory serialization of DOM tree to owned `String` |
| **DOM Parser (Tree Build)** | 5.27 ms | 5.73 ms | 5.56 ms | 6.68 ms | **~175 MB/s** | Full tree build with bidirectional node links and tag validation |
| **XEP Stanza Generation** | 13.87 ms | 14.07 ms | 14.05 ms | 14.24 ms | **~47 MB/s** | High-volume batch generation of 2,000 XEP stanzas (MUC, Carbons, MAM, Forms) |
| **SCRAM-SHA-256 Handshake** | 194.24 ms | 195.80 ms | 195.92 ms | 197.59 ms | **~0.5 MB/s** | Complete SASL SCRAM handshake simulation including PBKDF2 iterations |

### Automated Telemetry & CI Profiling

The `iksperf` suite supports machine-readable JSON output for automated CI regression checks (`--json`):

```bash
cargo run --release --bin iksperf -- --synthetic-kb 1024 --test all --json
```

Example JSON schema:
```json
{
  "suite": "iksemel-rs",
  "payload_bytes": 1048671,
  "iterations": 5,
  "metrics": [
    {
      "name": "SAX Parser (Streaming)",
      "bytes": 1048671,
      "best_micros": 2954,
      "avg_micros": 3160,
      "p50_micros": 3131,
      "p95_micros": 3501,
      "max_micros": 3501,
      "best_mb_s": 338.48,
      "avg_mb_s": 316.48
    }
  ]
}
```

### Comparative Benchmark (vs. Other Rust XML Crates)

Benchmark executed across standard 1.00 MB synthetic XML payload via `cargo bench`:

```text
================================================================================
 Rust XML Libraries Comparative Benchmark (Payload: 1.00 MB, Iterations: 5)
================================================================================
  Operation / Library              |  Best Time |   Avg Time |   Throughput
  ---------------------------------+------------+------------+-------------
  iksemel SAX (Streaming)          |    3.59ms |    4.05ms |    278.27 MB/s
  quick-xml Reader (Pull)          |    1.38ms |    1.41ms |    724.59 MB/s
  xml-rs EventReader (Pull)        |   17.63ms |   17.96ms |     56.71 MB/s
  ---------------------------------+------------+------------+-------------
  iksemel DOM (Mutable Tree)       |    4.52ms |    4.88ms |    221.19 MB/s
  roxmltree (Read-Only Arena)      |    2.73ms |    2.80ms |    366.95 MB/s
  ---------------------------------+------------+------------+-------------
  iksemel XmlWriter (Stream)       |  662.46µs |  755.42µs |   1509.66 MB/s
  quick-xml Writer (Roundtrip)     |    1.85ms |    1.87ms |    541.86 MB/s
================================================================================
```

#### Feature & Architecture Comparison

| Feature / Capability | `iksemel-rs` | `quick-xml` | `roxmltree` | `xml-rs` |
|:---|:---:|:---:|:---:|:---:|
| **Memory Safety Guarantee** | **100% Safe** (`#![forbid(unsafe_code)]`) | Mixed (unsafe opts) | 100% Safe | 100% Safe |
| **Parsing Model** | **Chunked Push/Streaming SAX & DOM** | Pull / Iterator | In-Memory Arena | Pull / Iterator |
| **DOM Tree Mutation** | **Yes (Full Bidirectional Nodes)** | No (Event-only) | No (Read-only arena) | No (Event-only) |
| **Serialization Throughput** | **~1,510 MB/s (`XmlWriter`)** | ~542 MB/s (`Writer`) | N/A (Read-only) | ~30 MB/s (`Emitter`) |
| **Path Traversal & Selectors** | **Yes (`find_path`, `select`)** | No | Limited | No |
| **DoS Security Limits** | **Yes (Depth, Token, Entities)** | Limited | Limited | Limited |
| **RFC 6120/6121 XMPP Protocols**| **Built-in Native Stack** | No | No | No |
| **SASL SCRAM-SHA-1 / 256** | **Built-in PBKDF2 & HMAC** | No | No | No |
| **XEP Protocol Extensions** | **MUC, PubSub, Carbons, MAM, Forms** | No | No | No |

**Key Takeaways:**
1. **Streaming Serialization (`XmlWriter`):** `iksemel-rs` reaches **~1,510 MB/s**, outpacing `quick-xml` by nearly **3x** due to its zero-copy escaping and direct non-allocating byte buffer serialization.
2. **Streaming SAX vs. Pull Parsers:** `iksemel-rs` parses at **~278 MB/s**, being nearly **5x faster** than `xml-rs` (~57 MB/s). While `quick-xml` achieves high throughput on raw tokenization, `iksemel` balances parsing with full entity validation, bidirectional namespace tracking, and DoS security limit enforcement.
3. **DOM Capabilities:** While `roxmltree` is a read-only arena, `iksemel-rs` constructs a fully mutable, bidirectional linked DOM tree with parent, child, and sibling relationships, plus path querying at **~221 MB/s**.
4. **Complete Network & Protocol Stack:** Beyond raw XML parsing, `iksemel-rs` provides an asynchronous Tokio network stack, modern SASL SCRAM authentication, and XMPP extensions out-of-the-box.


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
