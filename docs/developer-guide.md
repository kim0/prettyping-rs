# Developer guide

## Project layout

```text
src/
  app.rs                 # probe scheduling + event/state machine
  cli.rs                 # argument parsing and compatibility handling
  config.rs              # config normalization + validation
  runtime.rs             # signal handling + render driving
  stats.rs               # global/recent stats math
  net/dns.rs             # one-time host resolution
  engine/
    mod.rs               # PingEngine trait/contracts
    unix_surge.rs        # Linux/macOS backend
    windows_ping_async.rs# Windows backend
  render/
    plain.rs             # non-terminal renderer
    terminal.rs          # terminal renderer with overlay lines
tests/
  cli_compat.rs
  runtime_semantics.rs
  render_snapshots.rs
  app_state_bounded.rs
  unix_live_smoke.rs     # optional env-gated live test
```

## Local workflow

Run all checks before committing:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Release workflow

This repo uses `cargo-release` to keep Cargo versions and git tags in sync.

```bash
cargo release patch --execute
```

Examples: `minor`, `major`.

- Tag format is `v<version>` (for example: `v1.2.3`).
- CI enforces that `Cargo.toml` version matches tag version.

Crates publishing is handled by `.github/workflows/publish-crates.yml` using crates.io Trusted Publishing (OIDC).

## Running locally

```bash
cargo run -- --help
cargo run -- -c 5 127.0.0.1
```

## Optional live Unix smoke test

```bash
PRETTYPING_RS_RUN_UNIX_LIVE_SMOKE=1 cargo test --test unix_live_smoke -- --nocapture
```

Optional custom target:

```bash
PRETTYPING_RS_RUN_UNIX_LIVE_SMOKE=1 PRETTYPING_RS_UNIX_LIVE_TARGET=1.1.1.1 cargo test --test unix_live_smoke -- --nocapture
```

## Design notes

- Keep behavior explicit; avoid hidden magic in CLI and runtime transitions.
- Prefer bounded memory paths for long-running sessions.
- Keep backend-specific differences localized under `src/engine/`.
- Preserve clear user-facing errors (especially permission and network guidance).

## Changing CLI behavior safely

When adding/changing flags:

1. Update parser/help text in `src/cli.rs`
2. Update normalization/validation in `src/config.rs`
3. Map to runtime contract in `src/lib.rs`
4. Add or update tests in `tests/cli_compat.rs`
5. Update `docs/cli-reference.md`

## Renderer changes

For rendering updates:

- update `src/render/plain.rs` and/or `src/render/terminal.rs`
- verify snapshot tests:

```bash
cargo test --test render_snapshots
```

## Cross-platform backend work

- Keep engine contract stable (`src/engine/mod.rs`)
- Validate monotonic poll/send semantics
- Ensure errors remain actionable for users on each OS
