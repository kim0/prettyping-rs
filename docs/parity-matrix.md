# Shell vs Rust parity matrix (through M6)

This matrix captures **implemented behavior as of M6**.

Legend:
- ✅ Match: behavior is equivalent for practical use.
- ⚠️ Intentional difference: changed on purpose by locked scope decisions.
- 🟨 Partial: broadly similar, with known caveats.

| Area | prettyping-shell | prettyping-rs (M6) | Parity |
| --- | --- | --- | --- |
| Ping engine model | Wraps external `ping` + `awk` pipeline | Pure Rust backend (`surge-ping` on Linux/macOS, `ping-async` on Windows) | ⚠️ |
| External tool overrides | `--awkbin`, `--pingbin` supported | `--awkbin`, `--pingbin` are hard errors (exit 2) | ⚠️ |
| Host resolution | Delegated to external ping process | Resolve once at startup, family-filtered (`-4`/`-6`) | 🟨 |
| Output mode auto-detection | Terminal mode when stdout is a TTY | Same default; `--terminal` / `--noterminal` override | ✅ |
| Graph styles | unicode/ascii + color/multicolor flags | Same flags and palette families | ✅ |
| Legend/global/recent toggles | `--[no]legend`, `--[no]globalstats`, `--[no]recentstats` | Same | ✅ |
| RTT bucket tuning | `--rttmin`, `--rttmax` with shell formulas | Same formulas, integer buckets | ✅ |
| RTT precision | RTT rendered as integer ms (`int(...)`) | RTT rendered/stats as integer ms (`as_millis` truncation) | ✅ |
| Timeout symbol | `!` (red when color enabled) | `!` (red when color enabled) | ✅ |
| Non-terminal output | Appends chars; wraps at `--last`; prints stats blocks | Same overall behavior | ✅ |
| Terminal stats overlay | Live overlay with cursor save/restore escapes | Same approach (`ESC7/ESC8`, erase line, redraw stats) | ✅ |
| Resize handling | `SIGWINCH` trap triggers resize logic in awk | `SIGWINCH` sets resize flag; size applied on next render event | 🟨 |
| Interrupt handling | Shell/pipeline signal behavior | Explicit interrupt event path, clean renderer finish/reset | 🟨 |
| Exit codes | Mostly inherited from shell/pipeline behavior | Contracted: `0` success/interrupt, `1` runtime error, `2` usage error, `>2` preserved | ⚠️ |
| IPv4/IPv6 | Depends on passthrough flags to system ping | Native `-4` / `-6` handling with conflict checks | ✅ |
| Native ping options in scope | Any extra options can be passed through | Explicit only: `-c`, `-i`, `-W`, `-s`, `-t`, `-4`, `-6` | ⚠️ |
| Legacy unsupported flags | `-f`, `-R`, `-q` rejected; `-a` ignored/TODO; `-v` ignored | `-f`, `-R`, `-q`, `-a` rejected (exit 2); `-v` accepted+ignored | ⚠️ |
| Single-dash long legacy forms | Accepts forms like `-color`, `-help` | Same compatibility for prettyping legacy long forms | ✅ |
| Duplicate replies | Not explicitly specified (shell has TODO notes) | Deterministic: duplicates flagged; not double-counted in stats | 🟨 |
| Late replies (after timeout) | Not explicitly specified | Deterministic: late replies flagged and counted separately | 🟨 |
| `httping` line parsing | Supported by shell parser | Not supported (out of scope) | ⚠️ |
| JSON mode | None | None | ✅ |
| Platform support level | Historically Unix shell environments | Linux/macOS first-class; Windows best-effort | ⚠️ |

## Notes behind intentional differences

1. Rust port is intentionally **pure-Rust** (no subprocess ping/awk).
2. `httping` is intentionally out of scope.
3. Legacy passthrough model is replaced by explicit Rust-native CLI contract.
4. Exit code behavior is intentionally normalized/documented.

For unsupported flags, caveats, troubleshooting, and release checklist, see:
- `docs/release-readiness.md`
