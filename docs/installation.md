# Installation

## Prerequisites

- Rust toolchain (stable)
- Cargo

Install Rust via `rustup` if needed: <https://rustup.rs>

## Build

From the repository root:

```bash
cargo build
```

Release build:

```bash
cargo build --release
```

Binary path after release build:

```text
target/release/prettyping
```

## Install locally with Cargo

```bash
cargo install --path .
```

This installs the `prettyping` binary to your Cargo bin directory (usually `~/.cargo/bin`).

## Verify install

```bash
prettyping --help
```

Then try a short run:

```bash
prettyping -c 3 127.0.0.1
```

## Uninstall

```bash
cargo uninstall prettyping-rs
```

`prettyping-rs` is the package/repo name; `prettyping` is the executable command.
