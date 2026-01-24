# Changelog

All notable changes to `iksemel-rs` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
