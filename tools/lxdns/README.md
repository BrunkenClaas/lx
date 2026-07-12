# lxdns

Diagnose DNS problems from `dig`, `nslookup`, or `host` output.

## Usage

```sh
dig example.invalid | lxdns
nslookup api.example.com | lxdns api.example.com
lxdns < dig_output.txt
```

**stdout** — plain-language explanation (result field: `explanation`).  
**stderr** — likely cause and suggested fix (suppressed by `--quiet`).

## Flags

`--json` · `--plain` · `--quiet` · `--lang` · `--dry-run` · `--verbose` · `--file` · `--max-input-bytes` · `--version` · `--help`
