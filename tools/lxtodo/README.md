# lxtodo

Extract TODO/FIXME/HACK comments and action items from code or text.

## Usage

```
lxtodo [OPTIONS] [INPUT]
```

Reads from stdin if no input is given. Accepts source code, log files, or any
text containing TODO-style markers.

## Examples

```bash
# Scan stdin
cat src/main.rs | lxtodo

# Scan a file
lxtodo --file src/main.rs

# JSON output
lxtodo --file src/main.rs --json

# Local scan only (no LLM call)
lxtodo --file src/main.rs --no-net

# Scan with a specific language
lxtodo --lang de --file src/main.rs
```

## Output

**Plain mode** (stdout): one TODO per line.

- With file and line: `src/main.rs:42: TODO: fix this`
- With file only:     `src/main.rs: FIXME: broken`
- Without location:   `TODO: add tests`

**JSON mode** (`--json`):

```json
{
  "todos": [
    { "file": "src/main.rs", "line": 42, "text": "TODO: fix this" },
    { "text": "FIXME: no location" }
  ]
}
```

## How it works

1. **Local scan**: Searches the input for lines containing `TODO:`, `FIXME:`,
   `HACK:`, `XXX:`, `NOTE:`, `OPTIMIZE:`, `BUG:`, `REVIEW:`.
2. **LLM enrichment**: Sends the pre-extracted lines (with line numbers) to the
   LLM to catch non-standard markers and enrich the results.

Use `--no-net` to skip the LLM call and get only the local regex scan results.

## Security flags

- `fsbound`: When reading a file via `--file`, symlinks that escape the file's
  parent directory are rejected.
- `untrusted`: The LLM system prompt instructs the model to ignore any
  instructions embedded in the user-provided data.

## Options

| Flag                    | Description                                      |
|-------------------------|--------------------------------------------------|
| `--json`                | Output as JSON                                   |
| `--plain`               | Disable ANSI colours                             |
| `--dry-run`             | Show input without calling the LLM               |
| `--quiet` / `-q`        | Suppress stderr diagnostics                      |
| `--lang <BCP-47>`       | Output language (default: auto-detect)           |
| `--verbose`             | Show model and token usage on stderr             |
| `--max-input-bytes <n>` | Limit input size (default: 512 KiB)              |
| `--file <PATH>`         | Read input from file                             |
| `--no-net`              | Local scan only; skip LLM call                   |
| `--version` / `-V`      | Print version                                    |
| `--help` / `-h`         | Print help                                       |
