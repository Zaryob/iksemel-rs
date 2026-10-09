# Security policy

The crate forbids unsafe code in its own library. Dependencies and platform TLS implementations have separate safety properties; this is not a guarantee against denial of service, authentication mistakes or protocol bugs. No security-support lifetime or completed independent security audit is published.

Use explicit `ParserLimits` and application-level input, time and memory budgets when handling untrusted XML. XMPP examples and unit tests are not an interoperability or transport-security certification.

Report sensitive problems privately to **zaryob.dev@gmail.com**, the maintainer address already listed in this repository's README. Include the affected commit, minimal input, configured limits, dependency versions and platform. Do not include live credentials. No response-time guarantee is published.

[Local validation and benchmark scope](docs/VALIDATION.md) describe what has actually been exercised. Fuzzing instructions and bounded targets are in [fuzz/README.md](fuzz/README.md).
