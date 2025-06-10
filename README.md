# Iksemel-Rs

A Rust implementation of iksemel library for XML parsing and manipulation.

## Overview

Iksemel-rs, which is reimplementation of [iksemel](https://github.com/meduketto/iksemel) written in C, is a Rust library that provides XML parsing and manipulation capabilities. It's designed to be efficient, safe, and easy to use in Rust applications.

## Features

- XML parsing and manipulation
- Built-in tools for XML processing:
  - `ikslint`: XML validation tool
  - `iksperf`: Performance testing tool
  - `iksroster`: XML roster management tool
- Async support with Tokio

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
iksemel = "0.1.0"
```

## Usage

```rust
use iksemel;

// Example usage will be added as the project develops
```

## Tools

The project includes several command-line tools:

- `ikslint`: XML validation tool
- `iksperf`: Performance testing tool
- `iksroster`: XML roster management tool

## License

This project is licensed under the LGPL-2.1 License - see the [LICENSE](LICENSE) file for details.

## Original Iksemel Implementation

- Gurer Ozen - [iksemel](https://github.com/meduketto/iksemel)

## Rust implementation 

- Süleyman Poyraz

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Building from Source

1. Clone the repository:
```bash
git clone https://github.com/yourusername/iksemel.git
cd iksemel
```

2. Build the project:
```bash
cargo build
```

3. Run tests:
```bash
cargo test
```

## Dependencies

- thiserror: Error handling
- clap: Command-line argument parsing
- rpassword: Secure password input
- sha1: SHA1 hashing
- tokio: Async runtime
- native-tls: TLS support 