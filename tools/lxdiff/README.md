# lxdiff

Explain a git or file diff in plain language.

Pipe any `git diff` output (or a patch file) into `lxdiff` and get a concise,
human-readable summary of what changed and why — ideal for code reviews.

## Usage

```sh
git diff HEAD~1 | lxdiff
git diff --staged | lxdiff
git diff --staged | lxdiff --json
cat my.patch | lxdiff --lang de
```

## Output

Plain text (default):
```
Adds a bounded cache with eviction support.
  - A max_entries field is added to the Cache struct to limit its size.
  - A with_capacity constructor allows callers to set the entry limit.
  - The insert method evicts the oldest entry when the cache is full.
```

JSON (`--json`):
```json
{
  "summary": "Adds a bounded cache with eviction support.",
  "changes": [
    "A max_entries field is added to the Cache struct to limit its size.",
    "A with_capacity constructor allows callers to set the entry limit.",
    "The insert method evicts the oldest entry when the cache is full."
  ]
}
```

## Security flags

- **redact** — The diff is scanned for secrets (API keys, tokens, passwords) and
  they are replaced with `[REDACTED]` before the diff is sent to the LLM.
  Use `--no-redact` only if you have audited the diff yourself and accept the risk.
- **untrusted** — The system prompt instructs the model to ignore any instructions
  embedded in the diff content, preventing prompt injection attacks.

## Options

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show the redacted diff that would be sent, then exit |
| `--no-redact` | Skip secret redaction (NOT recommended) |
| `--lang <BCP-47>` | Output language (default: auto-detected) |
| `--quiet` / `-q` | Suppress diagnostic messages |
| `--verbose` | Show model/provider/redaction details on stderr |
| `--max-input-bytes <n>` | Maximum bytes read from stdin (default: 512 KiB) |
| `--version` / `-V` | Print version in LX Coreutils canonical format |
