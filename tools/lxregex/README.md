# lxregex

Generate a regular expression from a plain-English description.

## Usage

```sh
# Create mode (no stdin) — generate regex from scratch
lxregex "match ISO 8601 dates"
lxregex --flavor rust "IPv4 address"

# Edit mode (pipe existing regex) — apply described change only
echo '^[a-z]+$' | lxregex "also allow digits"
```

If `DESCRIPTION` is omitted, it is read from stdin.

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--flavor` | `pcre` | Regex flavor: `pcre`, `rust`, `python`, `js`, `go`, `ere` |
| `--json` | off | Output as JSON (`pattern`, `explanation`, `dangerous`) |
| `--plain` | off | Disable ANSI formatting |
| `--dry-run` | off | Print the description that would be sent; do not call the LLM |
| `-q, --quiet` | off | Suppress stderr diagnostics |
| `--lang` | `auto` | Output language (BCP-47) |
| `--verbose` | off | Verbose diagnostics on stderr |
| `--max-input-bytes` | 524288 | Maximum bytes read from stdin |
| `-V, --version` | — | Print version in suite format |
| `-h, --help` | — | Print help |

## Examples

```sh
# PCRE email pattern (default flavor)
lxregex "email address"

# Rust regex for digits
lxregex --flavor rust "one or more digits"

# Go-compatible IPv4 pattern, JSON output
lxregex --flavor go --json "IPv4 address"

# JavaScript URL pattern piped from a file
cat tests/fixtures/url.txt | lxregex --flavor js
```

## Output

Plain output (default):
```
^[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}$
# Matches a full email address: local part, @ symbol, domain, and TLD of 2+ letters.
```

JSON output (`--json`):
```json
{
  "pattern": "^[a-zA-Z0-9._%+\\-]+@[a-zA-Z0-9.\\-]+\\.[a-zA-Z]{2,}$",
  "explanation": "Matches a full email address: local part, @ symbol, domain, and TLD of 2+ letters.",
  "dangerous": false
}
```

## Security flags

`nocmd` — The pattern is printed to stdout only. It is **never executed**. If the
generated pattern contains nested quantifiers that may cause catastrophic
backtracking (ReDoS), a warning is printed to stderr and `dangerous` is set to
`true` in the output. Always review patterns before use in production.

### ReDoS detection

`lxregex` performs a local, deterministic check for patterns with potentially
exponential backtracking complexity, such as `(a+)+`, `(a*)+`, `(.+)+`. When
detected, the tool warns on stderr:

```
WARNING: generated pattern may have catastrophic backtracking (ReDoS) risk.
         Review '(a+)+b' before using in production.
         Pattern was NOT executed.
```
