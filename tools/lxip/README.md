# lxip

Generate or explain `ip` commands (addr/route/link). State-aware: pipe `ip route` or `ip addr` output to account for existing configuration.

## Usage

```sh
# Generate: no existing state
lxip "add a static route to 10.0.0.0/24 via 192.168.1.254"

# Generate: account for existing routes
ip route show | lxip "add a static route to 10.0.0.0/24 via 192.168.1.254"

# Explain: describe what current ip state means
ip addr show | lxip
```

**stdout** — the `ip` command(s) to run (generate) or explanation (explain).
**stderr** — warnings about conflicts or risks.

## OS target

By default `lxip` generates commands for the **host OS**. Use `--target` to
target a different platform:

```sh
# Linux (iproute2: ip addr/route)
ip route show | lxip "add route to 10.0.0.0/24"          # default on Linux

# Windows (netsh / New-NetIPAddress / New-NetRoute)
lxip --target windows "add static IP 192.168.1.50/24 to Ethernet"

# macOS (ifconfig / route / networksetup)
lxip --target macos "set DNS to 1.1.1.1 on en0"
```

`lxip` warns on stderr when piped state looks like it comes from a different OS
than the target.

## Flags

`--target <linux|windows|macos>` · `--json` · `--plain` · `--quiet` · `--lang` · `--dry-run` · `--verbose` · `--file` · `--max-input-bytes` · `--allow-dangerous/-D` · `--version` · `--help`

## Security

`lxip` **never executes** any generated command. It only prints to stdout.

Dangerous patterns are detected locally across all three OS targets:

- **Linux:** `ip link set dev ... down`, `ip route flush`, `ip addr flush`
- **Windows:** `route delete all`, `route delete *`, `route -f`
- **macOS:** `route … flush`

When a dangerous pattern is found the tool exits with code 3 unless `--allow-dangerous` / `-D` is set. The DANGER warning is always printed, even with `--quiet`.
