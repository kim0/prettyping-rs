# Usage guide

## Basic command

```bash
prettyping <host>
```

Examples:

```bash
prettyping 1.1.1.1
prettyping example.com
```

The app resolves the host once at startup, then runs continuous probes until interrupted (or until `-c` count is reached).

## Common options

### Stop after N probes

```bash
prettyping -c 10 1.1.1.1
```

### Probe interval and timeout

```bash
prettyping -i 0.2 -W 1 8.8.8.8
```

- `-i` = interval between probes (seconds)
- `-W` = timeout per probe (seconds)

### Force IPv4 / IPv6

```bash
prettyping -4 example.com
prettyping -6 example.com
```

### Payload size and TTL

```bash
prettyping -s 120 -t 64 1.1.1.1
```

## Output customization

### Color, Unicode, legend, and stats toggles

```bash
prettyping --nocolor --nounicode --nolegend example.com
prettyping --noglobalstats --norecentstats example.com
```

All toggles have `--no...` inverse forms.

### Terminal vs non-terminal output

- Default behavior is auto-detect based on stdout TTY.
- Force terminal mode:

```bash
prettyping --terminal example.com
```

- Force plain/non-terminal mode (useful for files/pipes):

```bash
prettyping --noterminal example.com
```

### Graph width and window

```bash
prettyping --last 120 --columns 120 --lines 30 example.com
```

- `--last` controls recent stats window size.
- `--columns` / `--lines` override terminal size detection.

### RTT bucket bounds

```bash
prettyping --rttmin 10 --rttmax 200 example.com
```

These values control how reply latency maps to graph symbols/colors.

## Exit behavior

- `Ctrl+C` stops cleanly.
- Exit code `0` on normal success/interrupt.
- See full list in [CLI reference](./cli-reference.md).
