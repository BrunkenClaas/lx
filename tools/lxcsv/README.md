# lxcsv

Query or summarise CSV data in plain English.

## Usage

```sh
# Ask a question about CSV data piped from stdin
cat sales.csv | lxcsv "which region has the highest total?"

# Read CSV from a file
lxcsv --file data.csv "how many rows have a status of 'closed'?"

# Get structured JSON output
cat report.csv | lxcsv "what is the average order value?" --json
```

## Example Output

```
$ cat sales.csv | lxcsv "which region has the highest total?"
EMEA with $4.2M total
```

```json
$ cat sales.csv | lxcsv "which region has the highest total?" --json
{
  "answer": "EMEA with $4.2M total",
  "used_rows": "50 rows sampled"
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show input without sending to LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |
| `--file <PATH>` | Read CSV from file instead of stdin |
| `--root <PATH>` | Restrict file access to this directory (fsbound) |
| `--no-redact` | Disable secret redaction (not recommended) |

The question is a required positional argument.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (missing question or CSV data) |
| 5 | Redaction failed (security abort) |

## Security

- Input is treated as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the CSV.
- Secret values (API keys, passwords, tokens) are redacted from the CSV before it is sent to the LLM. Use `--no-redact` only after auditing the content.
- File access is constrained by `--root`; symlinks that escape the root are rejected.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
