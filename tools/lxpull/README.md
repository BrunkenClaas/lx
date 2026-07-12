# lxpull

Extract structured fields from unstructured text into a table.

## Usage

```sh
# Extract named fields from a document
lxpull --fields people,dates,places < article.md

# Read from a file
lxpull --fields name,email,phone --file contacts.txt

# Get structured JSON output
lxpull --fields title,author,version --json < manifest.txt
```

## Example Output

```
$ lxpull --fields people,dates,places < article.md
people          dates             places
────────────────────────────────────────────────
Alice Chen      March 12, 2024    Berlin
Bob Müller      March 15, 2024    Hamburg
Carol Osei                        Accra
```

```json
$ lxpull --fields people,dates,places --json < article.md
{
  "records": [
    { "people": "Alice Chen", "dates": "March 12, 2024", "places": "Berlin" },
    { "people": "Bob Müller", "dates": "March 15, 2024", "places": "Hamburg" },
    { "people": "Carol Osei", "dates": "", "places": "Accra" }
  ],
  "truncated": false
}
```

`truncated` is `true` when the input contained more than 40 entities and the result set was capped. The most prominent 40 are returned; a note is printed to stderr.

## Flags

| Flag | Description |
|------|-------------|
| `-f, --fields <list>` | Comma-separated field names to extract (required) |
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
| 2 | Bad usage (missing `--fields` or no input) |

## Security

- Treats all input as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the document.
- Does not execute any commands or write to any files beyond stdout.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
