# lxping

Interpret ping, traceroute, or mtr output: is the problem at the network, host, DNS, or all ok?

## Usage

```sh
ping -c 4 example.com | lxping
traceroute 8.8.8.8 | lxping
mtr --report example.com | lxping
```

**stdout** — plain-language explanation (result field: `explanation`).
**stderr** — verdict category: `network`, `host`, `dns`, or `ok` (suppressed by `--quiet`).

## Flags

`--json` · `--plain` · `--quiet` · `--lang` · `--dry-run` · `--verbose` · `--file` · `--max-input-bytes` · `--version` · `--help`
