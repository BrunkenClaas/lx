# lxmock

Generate realistic mock/fixture data from a plain-English description.

## Usage

```
lxmock [OPTIONS] [DESCRIPTION]
```

The description can be provided as a positional argument or via stdin.

## Examples

```sh
# Generate JSON mock data
lxmock "5 users with handle, display_name, and age as JSON"

# Generate CSV mock data via stdin
echo "10 transactions with date, amount, currency as CSV" | lxmock

# Output as structured JSON (includes format hint)
lxmock --json "3 products with id, name, price"

# Read description from a file
lxmock --file description.txt
```

## Output

- **Plain mode**: generated data only on stdout; format hint on stderr.
- **JSON mode**: `{"data":"...","format":"json|csv|..."}` on stdout.

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show prompt without calling the LLM |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider/token info on stderr |
| `--max-input-bytes <n>` | Limit stdin read size |
| `--file <PATH>` | Read description from file |
| `--version` / `-V` | Print version and exit |
