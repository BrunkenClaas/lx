# lxdigest

Summarise a whole directory using LLM assistance.

## Usage

```
lxdigest --path <PATH> [OPTIONS]
```

Walk a directory, collect a file listing, and ask the LLM to produce a
concise human-readable summary of what the directory contains and what it is for.

## Example

```bash
lxdigest --path ./my-project
# A Rust CLI tool that generates commit messages from git diffs using LLM assistance.

lxdigest --path ./my-project --json
# {"summary":"A Rust CLI tool...","files":["src/main.rs","Cargo.toml"]}
```

## Options

| Flag | Description |
|------|-------------|
| `--path <PATH>` | Directory to summarise (required) |
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show what would be analysed, then exit |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model and provider info on stderr |
| `--no-redact` | Disable secret masking (not recommended) |
| `--version` / `-V` | Print version information |
| `--help` / `-h` | Print help |

## Output (plain mode)

The summary text is printed to stdout — pipe-safe.
Notable files are listed on stderr (suppressed by `--quiet`).

## Output (JSON mode)

```json
{
  "summary": "High-level description of the directory.",
  "files": ["src/main.rs", "Cargo.toml"]
}
```

## Security flags

- **fsbound**: stays within the path specified; symlinks that escape root are skipped.
- **redact**: file listing is passed through `lx_redact` before reaching the LLM.
- **untrusted**: system prompt instructs the model to ignore any instructions embedded in file names or paths.
