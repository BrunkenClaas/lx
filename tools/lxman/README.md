# lxman

Show a plain-language man page for any CLI tool — better than reading `man` pages.

## Usage

```sh
# Positional argument
lxman grep

# Explicit flag form (both work identically)
lxman --for grep

# Get structured JSON output
lxman curl --json

# Output in a specific language
lxman git --lang de
```

## Example Output

```
$ lxman grep
grep searches for lines matching a pattern in files or stdin and prints the matching lines.
  1. grep 'error' app.log — find all lines containing 'error'
  2. grep -r 'TODO' ./src — search recursively through all files in src/
  3. grep -n 'pattern' file.txt — show matching lines with their line numbers
  4. grep -i 'warning' *.log — case-insensitive match across multiple log files
  5. grep -v 'debug' app.log — print lines that do NOT match (invert)
  6. grep -c 'fail' results.txt — count the number of matching lines
```

```json
$ lxman grep --json
{
  "summary": "grep searches for lines matching a pattern in files or stdin and prints the matching lines.",
  "examples": [
    "grep 'error' app.log — find all lines containing 'error'",
    "grep -r 'TODO' ./src — search recursively through all files in src/",
    "grep -n 'pattern' file.txt — show matching lines with their line numbers"
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--for <tool>` | Tool name to explain (alternative to positional argument) |
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show input without sending to LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage and model info |
| `--max-input-bytes <n>` | Override stdin size limit |
| `-V, --version` | Print version information |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (no tool name provided) |

## Security

- Does **not** execute any command it documents.
- No data is sent to any endpoint other than the configured LLM provider.
- No security flags required (tool name is a simple identifier, not untrusted code).

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
