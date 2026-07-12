# lxconv

Convert between data formats (JSON, CSV, YAML, XML).

## Usage

```
lxconv [OPTIONS] --to <FORMAT> [INPUT]
```

Reads from stdin if no positional input is given.

## Options

| Flag | Description |
|------|-------------|
| `--to <FORMAT>` | Target format: `json`, `csv`, `yaml`, `xml` (required) |
| `--json` | Output as JSON envelope `{"content":"...","method":"..."}` |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show input without sending to LLM |
| `--quiet` / `-q` | Suppress diagnostic stderr messages |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes <N>` | Limit stdin read size |
| `--file <PATH>` | Read input from file |
| `--version` / `-V` | Print version |
| `--help` / `-h` | Print help |

## Examples

```sh
# JSON array → CSV
echo '[{"name":"Alice","score":95},{"name":"Bob","score":80}]' | lxconv --to csv

# CSV → JSON
cat data.csv | lxconv --to json

# JSON → YAML (LLM-assisted)
cat config.json | lxconv --to yaml

# JSON → XML (LLM-assisted)
cat data.json | lxconv --to xml

# Output JSON envelope
cat data.csv | lxconv --to json --json
```

## Conversion strategy

- **JSON → CSV** and **CSV → JSON**: performed locally in Rust (no LLM call).
- **Same-format passthrough** (JSON → JSON, CSV → CSV): normalised locally.
- **All other conversions** (→ YAML, → XML, complex structures): delegated to the LLM.

Plain mode stdout contains only the converted data — safe to pipe.
JSON mode stdout contains `{"content":"...","method":"local|llm"}`.

## Security flags

- `untrusted`: the system prompt instructs the model to ignore any instructions
  embedded in the user-provided data.
