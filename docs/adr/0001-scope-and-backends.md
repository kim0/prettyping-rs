# ADR 0001: Scope and Ping Backends

- **Status:** Accepted
- **Date:** 2026-02-22

## Context

The Rust port needs a locked M0 baseline so follow-up milestones do not re-open core product decisions.

## Decision

1. Use a **pure Rust ping engine** (no external `ping` process).
2. Do **not** implement `httping` support.
3. Keep **integer RTT behavior** (truncate display/stats to integer milliseconds).
4. Remove legacy passthrough flags `--awkbin` and `--pingbin`.
5. Platforms:
   - Linux + macOS: first-class targets.
   - Windows: best-effort secondary target.
6. Do **not** add JSON output mode.
7. Use a normalized internal engine interface with platform backends:
   - Unix (Linux/macOS): `surge-ping`
   - Windows: `ping-async`

## Consequences

- The CLI and runtime are Rust-native and do not shell out to system ping.
- Some shell-era features are intentionally out of scope.
- Permission behavior depends on OS policy and environment; diagnostics must be explicit.
- Windows support is expected to improve iteratively and may lag Linux/macOS parity.
