# lxmd

Format raw, unstructured text as clean, well-structured Markdown.

## Usage

```
lxmd [OPTIONS] [INPUT]
```

Reads from stdin if no positional argument is given.

## Examples

```bash
# Format meeting notes piped from stdin
cat meeting_notes.txt | lxmd

# Format a file directly
lxmd --file notes.txt

# Output as JSON
echo "project status: in progress, 60% done" | lxmd --json

# Suppress stderr diagnostics
cat notes.txt | lxmd --quiet
```

## Options

| Flag | Description |
|------|-------------|
| `--json` | Output result as JSON `{"markdown":"..."}` |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show input without calling the LLM |
| `--quiet`, `-q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes <n>` | Truncate stdin at n bytes |
| `--file <PATH>` | Read input from file instead of stdin |
| `--version`, `-V` | Print version information |
| `--help`, `-h` | Print help |

## Output

**Plain mode:** The formatted Markdown is printed to stdout.

**JSON mode:** `{"markdown": "<formatted markdown string>"}`

## Security flags

`untrusted` — the system prompt instructs the model to ignore any instructions
embedded in the user-provided data. User input is always kept separate from
the trusted system prompt.
