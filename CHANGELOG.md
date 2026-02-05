# Changelog

All notable changes to `iksemel-rs` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.2] - 2026-02-05

### Added
- **SASL SCRAM Authentication (RFC 5802 & RFC 7677)**:
  - Full client-side implementation of `SCRAM-SHA-1` and `SCRAM-SHA-256` state machine (`ScramClient`, `ScramHash`).
  - Cryptographic key derivation primitives: HMAC-SHA-1, HMAC-SHA-256, and RFC 6070 PBKDF2 (`pbkdf2_sha1`, `pbkdf2_sha256`).
  - Asynchronous authentication helpers: `authenticate_scram_sha1_async` and `authenticate_scram_sha256_async`.
  - Comprehensive test suite validating RFC 5802 Section 5 test vectors and end-to-end mock server handshakes.
- **XEP-0198 (Stream Management)**:
  - Sequence counting and reliability state tracking (`StreamManagementState`): tracking `inbound_h`, `outbound_h`, unacknowledged stanza queue (`unacked_queue`), and acknowledgment processing.
  - Protocol stanza builders and parsers: `<enable>`, `<enabled>`, `<resume>`, `<resumed>`, `<r>` (ack request), and `<a>` (ack response).
  - High-level async connection methods: `enable_stream_management`, `send_sm_ack`, and `request_sm_ack`.
- **Asynchronous Transport Stream Split**:
  - `split()` method on `AsyncConnection` decoupling into `AsyncSender` and `AsyncReceiver`.
  - Thread-safe concurrent stanza dispatch across Tokio tasks with non-blocking stream parsing.

---

## [0.3.1] - 2026-01-30

### Added
- **XEP-0004 (Data Forms)**: Full implementation of `<x xmlns='jabber:x:data'>` forms (`DataForm`, `FormField`, `FieldType`, `DataFormType`), supporting form builders, parsers, multi-value fields, tabular reporting (`<reported>` / `<item>`), options, and stanza attachment/extraction.
- **XEP-0045 (Multi-User Chat)**: Helpers for joining rooms with history limits and passwords (`build_muc_join`), leaving rooms (`build_muc_leave`), detecting MUC presence (`is_muc_presence`), and extracting MUC user status codes (`extract_muc_status_codes`).
- **XEP-0060 (Publish-Subscribe)**: Stanza builders for publishing items (`build_pubsub_publish`), subscribing (`build_pubsub_subscribe`), unsubscribing (`build_pubsub_unsubscribe`), and parsing incoming PubSub event notification items (`extract_pubsub_items`).
- **Ergonomic DOM Helpers**: Added `new_cdata` constructor on `IksNode` for creating text leaf nodes directly.
- **Benchmark Suite Enhancements (`iksperf`)**: Added DOM query selector traversal benchmarks and high-frequency XEP stanza construction throughput metrics.

---

## [0.3.0] - 2026-01-24

### Added
- **100% Safe Rust Guarantee**: Enforced `#![forbid(unsafe_code)]` across the entire crate. Completely removed legacy C raw pointer allocator pool (`IksStack`) and `static mut ALLOCATOR`.
- **Security & DoS Protection**: Introduced `ParserLimits` protecting against Billion Laughs exponential entity expansion attacks, quadratic token growth, and runaway nesting depth recursion.
- **Ergonomic Path & Query Selectors**: Added `find_path`, `find_path_text`, `first_child_tag`, `select`, and `select_first` to `IksNode` for fluent CSS/XPath-like element queries and attribute filtering.
- **W3C XML Namespace Resolution**: Added `prefix()`, `local_name()`, `namespace_uri()`, and scoped namespace URI inheritance climbing up the parent DOM tree.
- **Non-blocking Tokio Transport (`AsyncConnection`)**: Built asynchronous XMPP network client over Tokio `TcpStream` and `tokio-native-tls`, with asynchronous StartTLS, SASL PLAIN, and resource binding.
- **XMPP Extension Protocols (XEPs)**:
  - **XEP-0199 (XMPP Ping)**: Ping builder (`build_ping`), detector (`is_ping`), and Pong responder (`build_pong`).
  - **XEP-0030 (Service Discovery)**: Queries and response parsers for `disco#info` and `disco#items`.
  - **XEP-0085 (Chat State Notifications)**: Chat states (`Active`, `Composing`, `Paused`, `Inactive`, `Gone`), attaching and extracting from messages.
- **Optional Serde Integration**: First-class `Serialize` and `Deserialize` support for `Jid`, `IksNode`, `RosterItem`, and `Roster` via `features = ["serde"]`.
- **Automated CI/CD**: Added GitHub Actions workflow running cross-platform tests (Linux, macOS, Windows), Clippy (`-D warnings`), and `rustfmt`.

### Changed
- Replaced raw string manipulation with fast zero-alloc slice scanning and table-driven character classification.
- Enhanced streaming writer `XmlWriter` with pretty-printing and direct stream escaping.

---

## [0.2.0] - 2025-07-24

### Added
- **XMPP Core Capabilities**:
  - RFC 6122 / RFC 7622 JID engine (bare, full, domain-only, stringprep-ready).
  - Incremental XMPP stream parser (`StreamParser`) retaining parser state across TCP packet chunks.
  - Stanza filtering & routing engine (`PacketFilter`, `RuleBuilder`).
  - Synchronous TCP transport with RFC 6120 StartTLS upgrade via `native-tls`.
  - SASL PLAIN and legacy Non-SASL SHA-1 digest authentication.
  - RFC 6121 Roster management protocol (`fetch_roster`, `sync_roster`).
- **CLI Tools**:
  - `ikslint`: Fast XML validation, entity checking, and tag histogram reporting.
  - `iksperf`: Benchmark suite with throughput measurement in MB/s and synthetic fixture generation.
  - `iksroster`: Contact roster synchronization and backup CLI.
- **High-Performance XML Streaming Writer**: `XmlWriter` providing buffered byte serialization with zero heap allocation overhead.

---

## [0.1.0] - 2024-05-10

### Added
- Initial Rust port of the original C `iksemel` library.
- Basic SAX streaming event parser and DOM tree representation.
- Base64 encoding/decoding and SHA-1 hashing helpers.
