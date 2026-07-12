# lxssl

Diagnose TLS/certificate errors from `openssl` or `curl` output.

## Usage

```sh
openssl s_client -connect example.com:443 2>&1 | lxssl
curl -v https://api.example.com 2>&1 | lxssl api.example.com
lxssl < openssl_output.txt
```

**stdout** — plain-language explanation (result field: `explanation`).  
**stderr** — likely cause and suggested fix (suppressed by `--quiet`).

## Flags

`--json` · `--plain` · `--quiet` · `--lang` · `--dry-run` · `--verbose` · `--file` · `--max-input-bytes` · `--version` · `--help`
