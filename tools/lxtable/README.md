# lxtable

Convert unstructured text into a Markdown table.

## Usage

```sh
# Convert a list of key-value pairs to a table
echo "Alice: PM, Bob: Dev Lead, Carol: QA" | lxtable

# Read from a file
lxtable --file team.txt

# Get structured JSON output
lxtable --json < data.txt
```

## Example Output

```
$ echo "Alice: PM, Bob: Dev Lead, Carol: QA" | lxtable
| Name  | Role     |
|-------|----------|
| Alice | PM       |
| Bob   | Dev Lead |
| Carol | QA       |
```

```json
$ echo "Alice: PM, Bob: Dev Lead, Carol: QA" | lxtable --json
{
  "columns": ["Name", "Role"],
  "rows": [
    ["Alice", "PM"],
    ["Bob", "Dev Lead"],
    ["Carol", "QA"]
  ]
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
| `--file <PATH>` | Read input from file instead of stdin |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (no input) |

## Security

- Treats all input as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the text.
- Does not execute any commands or write to any files beyond stdout.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
