# lxfirewall

Generate or explain firewall rules (iptables/nftables/ufw). State-aware: when you pipe in your current ruleset, output accounts for conflicts, ordering, and lockout risk.

## Usage

```sh
# Generate: no existing state
lxfirewall "allow SSH only from 10.0.0.0/8"

# Generate: account for existing rules
iptables -S | lxfirewall "block all traffic from 192.168.50.0/24"

# Explain: no intent — describe what current rules do
ufw status verbose | lxfirewall
```

**stdout** — the firewall command(s) to run (generate) or explanation (explain).  
**stderr** — warnings about conflicts, ordering, and lockout risk.

## OS target

By default `lxfirewall` generates commands for the **host OS**. Use `--target` to
target a different platform:

```sh
# Linux (iptables/nftables/ufw) — default on Linux hosts
iptables -S | lxfirewall "allow HTTPS from anywhere"

# Windows (netsh advfirewall / New-NetFirewallRule)
lxfirewall --target windows "allow RDP from 10.0.0.0/8"

# macOS (pf/pfctl)
lxfirewall --target macos "block all incoming except SSH"
```

`lxfirewall` warns on stderr when piped state (e.g. `iptables -S` output) looks
like it comes from a different OS than the target.

## Flags

`--target <linux|windows|macos>` · `--json` · `--plain` · `--quiet` · `--lang` · `--dry-run` · `--verbose` · `--file` · `--max-input-bytes` · `--allow-dangerous/-D` · `--version` · `--help`

## Security

`lxfirewall` **never executes** any generated command. It only prints to stdout.

Dangerous patterns are detected locally with deterministic string matching across all three OS targets:

- **Linux:** `iptables -F`, `nft flush ruleset`, `ufw reset`, DROP/REJECT on port 22
- **Windows:** `netsh advfirewall reset`, `delete rule name=all`, `Remove-NetFirewallRule -All`
- **macOS:** `pfctl -F`, `pfctl -d`

When a dangerous pattern is found the tool exits with code 3 unless `--allow-dangerous` / `-D` is set. The DANGER warning is always printed, even with `--quiet`.
