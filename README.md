# prettyping-rs

Rust port of prettyping with a pure Rust ping engine.

## Current Status

Milestones M0 through M4 are implemented (CLI contract, deterministic runtime loop, Unix surge-ping backend, and Windows ping-async backend).
Rendering and stats parity work starts at M5.

## Locked Scope

- Pure Rust ping engine (no external `ping` process).
- No `httping` support.
- Integer RTT behavior (truncate to integer ms for display/stats).
- Removed legacy passthrough flags: `--awkbin`, `--pingbin`.
- Platforms: Linux + macOS first-class, Windows best-effort.
- No JSON output mode.

## Backend Direction

- Linux/macOS: `surge-ping`
- Windows: `ping-async`

## Platform Permission Notes

ICMP permissions depend on OS and environment:

- **Linux:** unprivileged ICMP usually works only if `net.ipv4.ping_group_range` allows the current group.
- **macOS:** unprivileged ICMP datagram sockets often work.
- **Windows:** best-effort via ICMP APIs; environment and policy can still block traffic. Payload size (-s) is best-effort and may not be honored by the backend.

Even on supported OSes, hardened/sandboxed environments may fail. The app will provide actionable diagnostics when permission/network setup blocks ping.

## Development Checks

Run from `prettyping-rs/`:

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
