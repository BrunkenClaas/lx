# lxsum

Summarise a file or command output. Reads from stdin and produces a concise
`tldr` sentence plus a short bullet list of key points.

## Usage

```sh
# Summarise a log file
cat deploy.log | lxsum

# Summarise command output
kubectl describe pod my-pod | lxsum

# One-sentence summary
cat article.md | lxsum --short

# Suggest a title or subject line
cat article.md | lxsum --headline

# Output as JSON
cat report.txt | lxsum --json

# Preview the redacted input without calling the LLM
cat notes.txt | lxsum --dry-run
```

## Output format (plain)

```
Summary: <one-sentence tldr>

  • <key point 1>
  • <key point 2>
  ...
```

## Output format (JSON)

```json
{
  "tldr": "one-sentence summary ≤120 chars",
  "bullets": ["key point 1", "key point 2"]
}
```

## Flags

| Flag | Description |
|---|---|
| `--headline` | Emit a short title or subject line instead of a summary |
| `--short` | Emit a single sentence instead of bullets |
| `--json` | Output as JSON |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show the redacted input that would be sent; exit without calling LLM |
| `-q`, `--quiet` | Suppress diagnostics on stderr |
| `--lang <BCP-47>` | Output language, e.g. `de`, `fr` (default: auto-detected) |
| `--verbose` | Show model, provider, and redaction status on stderr |
| `--max-input-bytes <n>` | Override the stdin read limit (default: 512 KiB) |
| `--no-redact` | Skip secret redaction — **not recommended** |
| `-V`, `--version` | Print version and exit |
| `-h`, `--help` | Print help and exit |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (e.g. no input provided) |
| 5 | Security abort (redaction failed) |

## Security flags

**`redact`** — All input is piped through `lx_redact::redact()` before being
sent to the LLM. API keys, tokens, and PII are masked. If redaction fails,
the tool exits with code 5. Use `--no-redact` only if you have audited the
input and accept the risk; a prominent warning is printed to stderr.

**`untrusted`** — The system prompt instructs the model to ignore any
instructions embedded in user-provided data. System prompt (static, trusted)
and user data (dynamic, untrusted) are always kept separate.
