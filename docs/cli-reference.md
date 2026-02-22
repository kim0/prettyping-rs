# CLI reference

## Synopsis

```text
prettyping [prettyping parameters] <host>
```

## Host argument

- `<host>` (required): hostname or IP address

## Rendering and prettyping-style flags

| Flag | Meaning | Default |
| --- | --- | --- |
| `--color` / `--nocolor` | Enable/disable ANSI color | enabled |
| `--multicolor` / `--nomulticolor` | Enable/disable multi-color buckets (if color is on) | enabled |
| `--unicode` / `--nounicode` | Enable/disable Unicode graph chars | enabled |
| `--legend` / `--nolegend` | Show/hide legend line | enabled |
| `--globalstats` / `--noglobalstats` | Show/hide global stats line | enabled |
| `--recentstats` / `--norecentstats` | Show/hide recent-window stats line | enabled |
| `--terminal` / `--noterminal` | Force terminal/plain mode | auto |
| `--last <n>` | Recent-window size | `60` |
| `--columns <n>` | Override terminal columns | auto |
| `--lines <n>` | Override terminal lines | auto |
| `--rttmin <n>` | Minimum RTT bucket bound (ms) | auto |
| `--rttmax <n>` | Maximum RTT bucket bound (ms) | auto |

## Native ping flags handled by prettyping

| Flag | Meaning | Default |
| --- | --- | --- |
| `-4`, `--ipv4` | IPv4 only | off |
| `-6`, `--ipv6` | IPv6 only | off |
| `-c <n>`, `--count <n>` | Stop after `n` probes | unlimited |
| `-i <s>`, `--interval <s>` | Probe interval in seconds | `1` |
| `-W <s>`, `--timeout <s>` | Per-probe timeout in seconds | `1` |
| `-s <n>`, `--size <n>` | Payload size in bytes | `56` |
| `-t <n>`, `--ttl <n>` | IP TTL / hop limit | backend default |

## Validation rules

- `--last`: `0..=10000`
- `--columns`: `1..=10000`
- `--lines`: `1..=10000`
- `--rttmin`: `> 0`
- `--rttmax`: `> 0`
- `--rttmin < --rttmax` when both set
- `-4` and `-6` cannot be used together
- `-c`: `> 0`
- `-i`: finite and `> 0`
- `-W`: finite and `> 0`
- `-s`: `<= 65507`
- `-t`: `1..=255`

## Compatibility behavior

### Removed legacy passthrough flags (hard error)

- `--awkbin`
- `--pingbin`

### Unsupported legacy flags (hard error)

- `-f`
- `-R`
- `-q`
- `-a`

### Legacy no-op

- `-v` is accepted and ignored.

### Legacy single-dash long spellings accepted

These are rewritten internally for compatibility (examples):

- `-help` → `--help`
- `-color` → `--color`
- `-nocolor` → `--nocolor`
- `-last` → `--last`

## Exit codes

- `0`: success (including controlled interrupt)
- `1`: runtime/backend/network failure
- `2`: usage/config error
- `>2`: preserved external/other code when applicable
