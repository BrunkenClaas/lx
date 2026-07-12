# lxhttp

Explain why an HTTP request failed — paste `curl -v` output or HTTP response headers.

## Usage

```sh
curl -v https://api.example.com/users 2>&1 | lxhttp
lxhttp < curl_output.txt
```

**stdout** — plain-language explanation (result field: `explanation`).  
**stderr** — HTTP status, likely cause, and suggested fix (suppressed by `--quiet`).

Pairs naturally with `lxcurl` (generate the curl command) and `lxssl` (TLS errors).

## Flags

`--json` · `--plain` · `--quiet` · `--lang` · `--dry-run` · `--verbose` · `--file` · `--max-input-bytes` · `--version` · `--help`
