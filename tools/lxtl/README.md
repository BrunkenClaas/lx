# lxtl

Translate text to another language.

## Usage

```sh
# Translate stdin to German
lxtl --to de < readme.txt

# Translate a file
lxtl --to fr --file document.md

# Get structured JSON output
lxtl --to ja --json < message.txt
```

## Example Output

```
$ echo "The build failed due to a missing dependency." | lxtl --to de
Der Build ist wegen einer fehlenden Abhängigkeit fehlgeschlagen.
```

```json
$ echo "The build failed due to a missing dependency." | lxtl --to de --json
{
  "translation": "Der Build ist wegen einer fehlenden Abhängigkeit fehlgeschlagen.",
  "source_lang": "en",
  "target_lang": "de"
}
```

## Flags

| Flag | Description |
|------|-------------|
| `-t, --to <code>` | Target language as BCP-47 code, e.g. `de`, `fr`, `ja` (required) |
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show input without sending to LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Language for diagnostic messages (BCP-47) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |
| `--file <PATH>` | Read input from file instead of stdin |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (missing `--to` or no input) |

## Security

- Treats all input as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the text being translated.
- Does not execute any commands or write to any files beyond stdout.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
