# lxjson

Repair and explain broken JSON.

## Usage

```sh
# Repair malformed JSON from stdin
echo "{name: 'Alice', age: 30,}" | lxjson

# Repair a JSON file
lxjson --file broken.json

# Pass broken JSON directly as an argument
lxjson "{name: 'Bob', active: true}"

# Get structured JSON output with a list of fixes
lxjson --file broken.json --json
```

## Example Output

```
$ echo "{name: 'Alice', age: 30,}" | lxjson
{"name":"Alice","age":30}
```

```json
$ echo "{name: 'Alice', age: 30,}" | lxjson --json
{
  "fixed": "{\"name\":\"Alice\",\"age\":30}",
  "errors": [
    {
      "description": "Keys must be quoted strings in JSON.",
      "fix": "Wrapped 'name' and 'age' in double quotes."
    },
    {
      "description": "String values must use double quotes, not single quotes.",
      "fix": "Replaced single-quoted value 'Alice' with \"Alice\"."
    },
    {
      "description": "Trailing comma after the last property is not valid JSON.",
      "fix": "Removed trailing comma after the 'age' value."
    }
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON envelope (includes fixed JSON and list of errors) |
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
| 2 | Bad usage (missing/invalid args) |

## Security

- Treats all input as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the JSON.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
