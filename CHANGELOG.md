# Changelog

All notable changes to `iksemel-rs` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2026-10-10

### Changed (Breaking)
- Minimum supported Rust version is now **1.88** (`rust-version` declared); the crate uses Rust 2024 edition and let chains.

### Changed
- Migrated `iksemel` and `iksemel-ffi` to Rust 2024 edition (`#[unsafe(no_mangle)]`, `unsafe extern` in the C ABI adapter).
- Dependencies upgraded: `thiserror` 2, `sha1`/`sha2`/`md-5` 0.11, `hmac` 0.13, `getrandom` 0.4; other minimums raised to current releases. Public API unchanged.

### CI
- New MSRV job (Rust 1.88); `actions/checkout` v5.

---

## [0.4.0] - 2026-10-10

### Changed (Breaking)
- `IksPacket::from` is now `Option<PacketJid>`, a raw C `iksid` view (no normalization; a leading `jabber:` is stripped; `/` is split before `@`), matching C `iks_id_new`. Matching is byte-exact like C `iks_strcmp`.
- `RuleBuilder::with_from` / `with_from_partial` accept `impl IntoRuleJid` (`Jid`, `&str` or `String`) and store the rule text as a `String`; the `RuleBuilder::from` / `from_partial` fields are now `Option<String>`.
- `Jid::new` splits the resource on the first `/` before the node on `@`, so `example.com/resource@device` parses with resource `resource@device`.
- `Jid` normalization now uses nodeprep/resourceprep (stringprep) and IDNA for the domain instead of ASCII lowercasing.
- `IksError` gains a `StreamError(String)` variant; `recv_stanza` returns it for `<stream:error/>`.

### Added
- C packet classification (`IksPacket`, `ikspak` parity), weighted `PacketFilter` with `FilterStatus::Eat`, `remove_rule`, and id-matched `recv_iq_response` used by roster, bind, session and non-SASL auth flows.
- `FilterHook`, `PacketFilter::add_rule_with_hook` and `PacketFilter::remove_hook` (C `iks_filter_remove_hook`).
- Pluggable transports and TLS backends (`Transport`, `AsyncTransport`, `TlsBackend`), traffic log hooks and byte counters.
- `ActorConnection`: a `Send`-compatible async facade over `AsyncConnection`.
- XEP-0198 stream management tracking on `AsyncConnection`, with `resume_stream_management` and `reconnect_and_resume`.
- Legacy DIGEST-MD5 SASL (RFC 2831) and C-ordered stanza builders.
- `iksemel-ffi`: C ABI adapter (`ffi/`), Python binding (`python/`) and `scripts/build-c-library.py`.
- Automated release workflow: bumping the version on `master` tags, creates the GitHub release and publishes to crates.io.

### Fixed
- `AsyncConnection::recv_iq_response` is cancellation-safe: stanzas received while waiting are no longer lost when the future is dropped by an outer `timeout`/`select!`.

---

## [0.3.4] - 2026-10-08

### Added
- **Comparative Benchmark Suite (`benches/comparison.rs`)**:
  - Comparative benchmark against `quick-xml`, `roxmltree`, and `xml-rs` across streaming SAX, DOM parsing, and XML serialization.
  - Demonstrated ~1,510 MB/s streaming serialization throughput with `XmlWriter` (~3x faster than `quick-xml` and ~50x faster than `xml-rs`).
  - Added comprehensive feature and architectural comparison matrix in README.
- **Cross-Platform Hardening & Windows Compatibility**:
  - Fixed Windows SChannel TLS stream size clippy warnings with `#[allow(clippy::large_enum_variant)]` on `ConnectionStream` and `AsyncConnectionStream`.
  - Added `.gitattributes` enforcing consistent LF line endings across all operating systems.
  - Migrated GitHub Actions CI caching to `Swatinem/rust-cache@v2`.
  - Isolated temporary files in DOM file operations tests using process IDs.
  - Hardened asynchronous transport tests against slow virtual machine scheduling in CI.

---

## [0.3.3] - 2026-02-11

### Added
- **XEP-0280 (Message Carbons)**: Multi-device synchronization (`build_carbons_enable`, `build_carbons_disable`, `mark_carbon_private`, `wrap_carbon_sent`, `wrap_carbon_received`, and `extract_carbon`).
- **XEP-0313 (Message Archive Management - MAM v2) & XEP-0059 (RSM)**: Query historical chats (`MamQuery`) with time/JID filters, RSM result set pagination (`max`, `after`, `before`), `extract_mam_result`, and completion indicator (`parse_mam_fin`).
- **Examples**:
  - `examples/async_xmpp_bot.rs`: Complete async echo bot with StartTLS, SCRAM-SHA-256, Stream Management, and connection splitting.
  - `examples/dom_selectors_and_streaming.rs`: Hardened DOM parsing, CSS/XPath query selectors, zero-allocation escaping, and pretty `XmlWriter`.
- **Advanced Performance Profiling Suite (`iksperf`)**:
  - Added latency percentiles (Min, Avg, P50/Median, P95, Max) and throughput metrics.
  - Added zero-allocation `escape_cow` vs `escape` benchmarks.
  - Added SCRAM-SHA-256 handshake and PBKDF2 throughput benchmarks.
  - Added `--json` telemetry output flag for automated CI regression tracking.

### Performance
- **Zero-Allocation String Escaping**: Added `escape_cow` and `unescape_cow` returning `Cow::Borrowed` when no special XML entities exist.
- **Iterative Ancestor Namespace Scoping**: Eliminated intermediate `format!` string allocations during DOM parent namespace resolution.
- **Pre-Parsed Query Selectors**: Pre-parsed selector segments once at query initiation using `SelectorSegment`, providing ~5,300 MB/s query throughput.

---

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
