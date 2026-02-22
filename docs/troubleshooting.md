# Troubleshooting

## Quick triage

1. **Check exit code**
   - `2` = usage/config error (bad flag/value)
   - `1` = runtime/network/permission issue
2. **Test localhost first**
   - `prettyping -c 3 127.0.0.1`
   - `prettyping -c 3 ::1` (if IPv6 enabled)
3. **Then test a known public target**
   - `prettyping -c 3 1.1.1.1`

If localhost fails, focus on local permissions/firewall before remote routing.

## Linux: ICMP permission denied

Symptoms:
- startup/runtime error mentioning permission denied or ICMP socket access

Checks:

```bash
id -g
sysctl net.ipv4.ping_group_range
```

Diagnostic-only temporary setting:

```bash
sudo sysctl -w net.ipv4.ping_group_range="0 2147483647"
```

If that fixes it, persist via `/etc/sysctl.d/*.conf` according to your distro policy.

## macOS: socket access blocked

Symptoms:
- permission/socket failures even though command is valid

Try:
- run in normal Terminal/iTerm session (outside sandboxed/container shell)
- test once with elevated privileges to confirm permission boundary
- review local security tools/policies that may block ICMP

## Windows: access/network unreachable

Common causes:
- firewall or endpoint security policy
- route/interface issues

Checks:

```powershell
ipconfig
route print
```

Try once from elevated PowerShell/terminal to separate privilege issues from policy issues.

## DNS / family mismatch (`-4` or `-6`)

Symptoms:
- resolution works, but no address for selected family

Fix:
- remove `-4`/`-6` and retry
- or choose a host that has A/AAAA record for that family

## Invalid CLI values

Examples that fail validation:
- `-c 0`
- `-i 0`
- `-W 0`
- `-t 0` or `-t 300`
- `--rttmin` greater than or equal to `--rttmax`
- using `-4` and `-6` together

See full rules in [CLI reference](./cli-reference.md).
