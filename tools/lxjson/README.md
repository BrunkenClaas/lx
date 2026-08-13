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
  "json": "{\"name\":\"Alice\",\"age\":30}",
  "method": "local",
  "changes": [
    "converted single quotes to double quotes",
    "added double quotes around unquoted keys",
    "removed trailing comma(s) before closing bracket or brace"
  ],
  "response_truncated": false
}
```

`method` is `"local"` when the repair needed no model call, `"llm"` otherwise.
`response_truncated` is `true` only when a model reply was cut at its token
limit, in which case `json` may be missing its tail.

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
