# Release readiness notes (M7)

This document is the M7 handoff doc for:
- unsupported flags/features,
- platform caveats,
- troubleshooting,
- final smoke checklist,
- version tag procedure.

It reflects behavior implemented through M6.

## 1) Unsupported and changed behavior

### 1.1 Removed legacy passthrough flags (hard error, exit code 2)

- `--awkbin`
- `--pingbin`

Reason: prettyping-rs uses a pure Rust engine and does not spawn `awk`/`ping`.

### 1.2 Unsupported legacy flags (hard error, exit code 2)

- `-f` flood
- `-R` record route
- `-q` quiet
- `-a` audible

### 1.3 Legacy compatibility no-op

- `-v` is accepted and ignored.

### 1.4 Feature intentionally not implemented

- `httping` support is out of scope.
- JSON output mode is out of scope.

### 1.5 Explicit native ping flags supported in Rust CLI

- `-4`, `-6`
- `-c`, `-i`, `-W`, `-s`, `-t`

No generic passthrough of unknown ping flags is provided.

## 2) Platform-specific caveats

### 2.1 Linux (first-class)

- Backend: `surge-ping`.
- Unprivileged ICMP can fail if `net.ipv4.ping_group_range` excludes current gid.
- On permission failures, runtime errors include guidance with `sysctl` and `id -g` hints.

### 2.2 macOS (first-class)

- Backend: `surge-ping`.
- Unprivileged ICMP usually works, but sandbox/policy contexts can still block access.
- On permission failures, runtime errors suggest running from normal Terminal and validating policy boundaries.

### 2.3 Windows (best-effort)

- Backend: `ping-async`.
- Error guidance maps common Win32/WSA failures (access denied, network/host unreachable, general failure).
- Some backend details differ from Unix path:
  - payload size (`-s`) is best-effort,
  - TTL handling and reply metadata depend on platform/backend behavior.

### 2.4 Other platforms

- Not supported currently; runtime exits with backend unsupported error.

## 3) Troubleshooting guide

Use this sequence to reduce false leads.

### Step A: verify CLI usage vs runtime failure

- If process exits with code `2`: this is a usage/config error (bad flags, invalid values, unsupported legacy flags).
- If process exits with code `1`: runtime/backend/network failure.

### Step B: basic target sanity

- Try known-good targets first:
  - `127.0.0.1` (IPv4 localhost)
  - `::1` (IPv6 localhost, if IPv6 enabled)
- If localhost fails, focus on local permissions/firewall before remote routing.

### Step C: Linux permissions

Symptoms:
- error contains permission denied / ICMP socket open failure.

Checks:
1. `id -g` (current group id)
2. `sysctl net.ipv4.ping_group_range`

Temporary allow-all range (diagnostic only):
```bash
sudo sysctl -w net.ipv4.ping_group_range="0 2147483647"
```

If that fixes it, persist policy via `/etc/sysctl.d/*.conf`.

### Step D: macOS permissions/sandbox

Symptoms:
- startup ICMP socket permission errors.

Checks:
1. Run from normal Terminal.app/iTerm session.
2. Retry outside sandboxed/containerized shell.
3. Optionally retry with elevated privileges to confirm permission boundary.

### Step E: Windows firewall/network

Symptoms:
- access denied, network unreachable, host unreachable, general IP failure.

Checks:
1. Verify interface and addressing:
   - `ipconfig`
2. Verify routing:
   - `route print`
3. Validate firewall/security policy for ICMP Echo (in/out rules).
4. Retry once in elevated PowerShell/terminal to differentiate privilege vs policy.

### Step F: DNS/family mismatch

Symptoms:
- host resolution succeeds but no address for selected family.

Checks:
- remove `-4`/`-6` and retry,
- verify target has A/AAAA records for requested family.

## 4) Smoke checklist (release candidate)

Run from `prettyping-rs/`.

### 4.1 Required quality gates

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

### 4.2 Functional smoke (local)

1. Help + compatibility notes:
```bash
cargo run -- --help
```
2. Basic finite run:
```bash
cargo run -- -c 3 127.0.0.1
```
3. Non-terminal mode:
```bash
cargo run -- --noterminal -c 3 127.0.0.1
```
4. Legacy rejection check:
```bash
cargo run -- --awkbin awk 127.0.0.1
```
(Expect usage error, exit code 2.)

### 4.3 Optional Unix live smoke test (env-gated)

```bash
PRETTYPING_RS_RUN_UNIX_LIVE_SMOKE=1 cargo test --test unix_live_smoke -- --nocapture
```

Optional target override:
```bash
PRETTYPING_RS_RUN_UNIX_LIVE_SMOKE=1 PRETTYPING_RS_UNIX_LIVE_TARGET=1.1.1.1 cargo test --test unix_live_smoke -- --nocapture
```

## 5) Version tag procedure

Assumes main branch is clean and CI passes.

1. Confirm clean tree:
```bash
git status --short
```
2. Ensure tests/checks pass (Section 4.1).
3. Bump version in `Cargo.toml` (`[package].version`) if needed.
4. Commit release prep:
```bash
git add Cargo.toml Cargo.lock docs/ README.md src/ tests/
git commit -m "release: vX.Y.Z"
```
5. Create annotated tag:
```bash
git tag -a vX.Y.Z -m "prettyping-rs vX.Y.Z"
```
6. Push commit + tag:
```bash
git push origin <branch>
git push origin vX.Y.Z
```
7. Verify CI/release workflow status.

## 6) Quick operator reference

- Exit code `0`: success (including controlled interrupt)
- Exit code `1`: runtime failure
- Exit code `2`: usage/config error
- Exit code `>2`: preserved external/other error codes where applicable
