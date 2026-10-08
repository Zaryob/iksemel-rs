# Bounded XML fuzzing

The `xml` target exercises DOM parsing and incremental SAX input with strict resource limits. UTF-8 inputs are split only at character boundaries. Invalid UTF-8 is outside this string API's contract. Network transport is not fuzzed.

From the repository root:

```sh
cargo install cargo-fuzz --locked
rustup toolchain install nightly --profile minimal
mkdir -p fuzz/corpus/xml
cp fuzz/seeds/*.xml fuzz/corpus/xml/
cargo +nightly fuzz run xml fuzz/corpus/xml -- -max_total_time=60 -max_len=65536
```

The target treats parser errors as expected; a panic, abort or sanitizer finding is a failure. Retain the failing artifact and add a minimized regression test before fixing it. A short smoke run is not exhaustive coverage. Tool availability and actual execution are recorded separately in [the validation record](../docs/VALIDATION.md).
