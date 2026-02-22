# prettyping-rs

[![CI](https://github.com/kim0/prettyping-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/kim0/prettyping-rs/actions/workflows/ci.yml)
[![Release](https://github.com/kim0/prettyping-rs/actions/workflows/release.yml/badge.svg)](https://github.com/kim0/prettyping-rs/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/kim0/prettyping-rs)](https://github.com/kim0/prettyping-rs/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/kim0/prettyping-rs)

A beautiful, real-time ping graph for your terminal — built in pure Rust.

`prettyping` helps you spot latency spikes and packet loss at a glance, without the noisy wall of classic ping output.

This project is inspired by the excellent [prettyping](https://github.com/denilsonsa/prettyping.git) shell script.

<p align="center">
  <img src="docs/images/prettyping-rust-screenshot.png" alt="prettyping terminal screenshot" width="880" />
</p>

## Why prettyping?

- **Instant visual signal**: see connection quality trends in one line
- **Pure Rust runtime**: no `ping | awk` subprocess pipeline
- **Built for long runs**: clean terminal mode and log-friendly plain mode
- **Cross-platform direction**: first-class Linux/macOS, Windows best-effort

## Features

- Unicode or ASCII graph output
- Optional color and multi-color latency buckets
- Global stats and recent-window stats
- Terminal auto-detection with manual override
- Native flags for count, interval, timeout, payload size, TTL, IPv4/IPv6

## Quick start

### Install from source

```bash
cargo install --path .
```

Repo/package name: `prettyping-rs`  
Installed command: `prettyping`

### Run

```bash
prettyping 1.1.1.1
```

### More examples

```bash
# Stop after 10 probes
prettyping -c 10 1.1.1.1

# Faster probing
prettyping -i 0.2 -W 1 8.8.8.8

# Force plain output (good for logs/pipes)
prettyping --noterminal --nounicode --nocolor -c 20 example.com

# Force IPv6
prettyping -6 example.com
```

## Platform support

| Platform | Status | Backend |
| --- | --- | --- |
| Linux | First-class | `surge-ping` |
| macOS | First-class | `surge-ping` |
| Windows | Best-effort | `ping-async` |

> ICMP permissions can be restricted by OS/network policy. See [Troubleshooting](docs/troubleshooting.md).

## Documentation

- [Installation](docs/installation.md)
- [Usage guide](docs/usage.md)
- [CLI reference](docs/cli-reference.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Developer guide](docs/developer-guide.md)

## Compatibility notes

- Removed legacy passthrough flags: `--awkbin`, `--pingbin` (hard error)
- Unsupported legacy flags: `-f`, `-R`, `-q`, `-a` (hard error)
- Legacy `-v` is accepted and ignored
- No `httping` mode
- No JSON output mode

## Exit codes

- `0` success (including controlled interrupt)
- `1` runtime/backend/network failure
- `2` usage/config error

## License

MIT
