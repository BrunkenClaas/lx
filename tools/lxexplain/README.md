# lxexplain

Explain any command, error message, or code snippet in plain language.

## Usage

```sh
# Explain a command passed as an argument
lxexplain "tar -xzf archive.tar.gz"

# Explain text from stdin
echo "ENOENT: no such file or directory" | lxexplain

# Get structured JSON output
lxexplain "git rebase -i HEAD~3" --json
```

## Example Output

```
$ lxexplain "tar -xzf archive.tar.gz"
Extracts a gzip-compressed tar archive into the current directory.
  • '-x' extracts files from the archive
  • '-z' decompresses using gzip
  • '-f archive.tar.gz' specifies the archive file
```

```json
$ lxexplain "tar -xzf archive.tar.gz" --json
{
  "summary": "Extracts a gzip-compressed tar archive into the current directory.",
  "details": [
    "'-x' extracts files from the archive",
    "'-z' decompresses using gzip",
    "'-f archive.tar.gz' specifies the archive file"
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show input without sending to LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (no input) |

## Security

- Does **not** execute any command it explains.
- Treats all input as untrusted data: the system prompt instructs the model to ignore embedded instructions.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
