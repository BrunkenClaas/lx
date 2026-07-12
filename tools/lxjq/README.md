# lxjq

Generate a `jq` expression from a plain-English description.

## Usage

```sh
# Create mode (no stdin) — generate jq from scratch
lxjq "extract all email fields from array of users"
lxjq --input '{"users":[{"id":1}]}' "get all ids"

# Edit mode (pipe existing expression) — apply described change only
echo '.users[].email' | lxjq "also include the id field"
```

`DESCRIPTION` is the natural-language description of the JSON transformation you want.
If omitted, the description is read from stdin.

### Optional flags

| Flag | Description |
|------|-------------|
| `--input <JSON>` | Provide a JSON sample as structural context |
| `--json` | Output as JSON (`expression`, `explanation`, `dangerous` fields) |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent to the LLM, then exit |
| `-q`, `--quiet` | Suppress diagnostic messages on stderr |
| `--lang <BCP-47>` | Override output language (default: auto-detect) |
| `--verbose` | Show model/provider/lang on stderr |
| `--max-input-bytes <n>` | Override the stdin size limit |
| `-V`, `--version` | Print version information |
| `-h`, `--help` | Print help |

## Output

Plain-text (default): the `jq` expression on the first line (copy-paste ready),
followed by a blank line and `# <explanation>`.

```
.users[] | select(.active) | .name

# Filters the users array to active users and extracts their names
```

JSON (`--json`):

```json
{
  "expression": ".users[] | select(.active) | .name",
  "explanation": "Filters the users array to active users and extracts their names",
  "dangerous": false
}
```

## Security flags

`nocmd` — The generated expression is printed to stdout only. It is **never
executed**. Expressions containing potentially dangerous jq built-ins (`@sh`,
`env`, `$ENV`, `halt`, `debug`, `input`) are flagged with a warning on stderr.

## Examples

```sh
# Simple field extraction
lxjq "extract all names from the users array"

# With JSON context so the model can tailor the expression
lxjq --input '{"users":[{"name":"Alice","active":true}]}' \
     "filter active users and return their names"

# Pipe a description from another tool
echo "count items in the results array" | lxjq

# JSON output for scripting
lxjq --json "get the value of the .version key"
```
