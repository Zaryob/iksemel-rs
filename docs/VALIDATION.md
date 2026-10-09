# Local validation and measurement scope

9 October 2026 (Europe/Istanbul); macOS 27.0 (26A428), Apple M4, 16 GiB RAM, Xcode 27.0 (27A266a), Apple Clang 21.0.0. Runs shared a busy development host; timings are observations, not performance guarantees.

Source baseline: [`c1176a5b99f4d503041239f814e1f51a6b521783`](https://github.com/Zaryob/iksemel-rs/commit/c1176a5b99f4d503041239f814e1f51a6b521783). The comparison harness and bounded fuzz target were changed in this portfolio branch; production parser code was unchanged. Stable rustc/cargo 1.91.1, aarch64-apple-darwin. Local checks:

- `cargo test --all-features`: all executed tests and doctests passed (45 library unit tests plus integration/doctest suites).
- `cargo +nightly miri test --lib parser::tests`: 8 passed, 37 filtered out. This does not cover the entire crate or network/TLS dependencies.
- `cargo +nightly fuzz run xml fuzz/seeds -- -max_total_time=45 -max_len=65536`: 314,921 executions in 46 seconds, no crash observed. cargo-fuzz 0.13.2 and rustc 1.101.0-nightly (1d81eb4ad, 2026-10-07). This short run originally used the seed directory as a writable corpus; the documented command now copies seeds into an ignored corpus directory. No generated corpus is committed. [fuzz-Cargo.lock](evidence/fuzz-Cargo.lock) retains the dependency resolution of this smoke run.

The target bounds UTF-8 input to 65,536 bytes and supplies depth/entity/attribute/token limits. It exercises DOM and incremental SAX string APIs. Invalid UTF-8, transport, credentials and full XML/XMPP conformance are outside this target's coverage; those surfaces are not declared safe or exempt from review.

Per-test terminal reports are not tracked in the repository; the commands above produce local output.

## Benchmarks

```sh
# To reproduce the dependency resolution used for this record:
cp docs/evidence/Cargo.lock Cargo.lock
cargo bench --locked --bench comparison
cargo run --locked --release --bin iksperf -- --synthetic-kb 1024 --iterations 5
```

The comparison harness fixes the release payload at 1024 KiB and uses five samples; it does not accept size/iteration flags. [comparison.txt](evidence/comparison.txt) and [iksperf.txt](evidence/iksperf.txt) retain raw output; the resolved [Cargo.lock](evidence/Cargo.lock) pins this measurement, not a new crate release.

The comparison run consumed 1,048,666 input bytes and reports MiB/s (2^20 bytes). SAX, pull readers, mutable DOM, read-only DOM and writers perform different work. These numbers do **not** establish relative library speed. The harness is an `Instant` loop, not Criterion; it has no independent-process statistics or controlled host load. It now checks SAX errors and black-boxes results, but equivalent semantic outputs still need validation.

`iksperf` prints `MB/s` while dividing by 2^20; interpret those labels as MiB/s in this historical raw output. Selector and crypto rows do not measure XML input parsing throughput. Avoid speed ratios or a blanket performance claim from either table. Repeat with realistic inputs, equal work, documented dependencies and isolated runs before publishing such claims.

[Safety/reporting boundaries](../SECURITY.md), [fuzzing instructions](../fuzz/README.md), and [compatibility/performance backlog #3](https://github.com/Zaryob/iksemel-rs/issues/3) record the remaining work. This change does not publish a crate or modify its registered crates.io metadata.
