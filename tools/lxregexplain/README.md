# lxregexplain

Explain what a regular expression does in plain language, with a token-by-token breakdown.

## Usage

```
lxregexplain [OPTIONS] [REGEX]
```

Pass the regex as a positional argument, via `--file`, or on stdin.

## Options

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent to the LLM without sending |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes <n>` | Limit stdin read size |
| `--file <PATH>` | Read regex from a file |
| `-V, --version` | Print version information |
| `-h, --help` | Print help |

## Examples

```sh
# Plain output — explanation goes to stdout
lxregexplain '^\d{4}-\d{2}-\d{2}$'

# Full JSON with parts breakdown
lxregexplain --json '#[0-9a-fA-F]{6}'

# From stdin
echo '^https?://' | lxregexplain

# From file
lxregexplain --file pattern.txt
```

## Output

**Plain mode** (`stdout`):
```
Matches an ISO date string like 2024-01-15, anchored to the full line.
```

**JSON mode** (`--json`, `stdout`):
```json
{
  "regex": "^\\d{4}-\\d{2}-\\d{2}$",
  "explanation": "Matches an ISO date string...",
  "parts": [
    {"token": "^", "means": "Start of line anchor"},
    {"token": "\\d{4}", "means": "Exactly four digits (year)"},
    ...
  ]
}
```

## Security

No security flags apply. Input is sent to the configured LLM provider as-is.
