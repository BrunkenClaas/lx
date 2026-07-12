# lxdraft

Draft a polished email, ticket, reply, or message from brief notes or bullet points.

## Usage

```
lxdraft [OPTIONS] [INPUT]
```

Pass notes as a positional argument, via `--file`, or via stdin.

```sh
# Draft an email
lxdraft --kind email "meeting rescheduled, tuesday 2pm, Q3 roadmap, invite Alice"

# Draft a ticket from stdin
echo "dark mode toggle broken, chrome, stays light on refresh" | lxdraft --kind ticket

# Draft from a file, output as JSON
lxdraft --kind ticket --file notes.txt --json

# See what would be sent to the LLM (redacted)
lxdraft --dry-run "sensitive details here"
```

## Output

- **Plain mode (default):** The draft body is written to stdout. Subject line (if applicable) is written to stderr as a diagnostic header.
- **JSON mode (`--json`):** `{"subject": "...", "body": "..."}` to stdout.

## Flags

| Flag | Description |
|------|-------------|
| `--kind <KIND>` | Draft format: `email`, `ticket`, `reply`, `message` (default: `email`) |
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show redacted input that would be sent to the LLM, then exit |
| `-q, --quiet` | Suppress diagnostic messages on stderr |
| `--lang <BCP-47>` | Output language (e.g. `en`, `de`, `fr`; default: `auto`) |
| `--verbose` | Show model/provider/config diagnostics on stderr |
| `--max-input-bytes <N>` | Maximum bytes to read from stdin (default: 512 KiB) |
| `--file <PATH>` | Read input from file instead of stdin |
| `--no-redact` | Disable secret redaction (not recommended) |
| `-V, --version` | Print version information |
| `-h, --help` | Print help |

## Security

SEC flag: **redact** — all input is passed through `lx_redact` before being sent to the LLM. Sensitive values (keys, tokens, personal data) are masked automatically. Use `--no-redact` only if you have audited the input yourself.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General/logical error (config, network, LLM) |
| 2 | Bad usage (invalid flags or missing input) |
| 5 | Security abort (redaction failed) |
