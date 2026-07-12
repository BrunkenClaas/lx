# lxpr

Generate a pull-request description from a git diff or commit log.

**Security flags:** `redact`, `untrusted`

## Usage

```
git diff HEAD~1 | lxpr
git log -p -1   | lxpr
lxpr --file my.diff
```

Pipe a diff or `git log -p` output into `lxpr`. It produces a PR title and a
markdown body with Summary, Changes, and Test Plan sections.

## Output

**Plain mode** (default): prints `title\n\nbody` to stdout — the full PR text
ready to paste into GitHub or pipe to a file.

**JSON mode** (`--json`): prints `{"title":"...","body":"..."}` to stdout.

## Options

| Flag | Description |
|---|---|
| `--json` | Output as JSON |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show redacted input that would be sent, then exit |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider/lang diagnostics on stderr |
| `--max-input-bytes <n>` | Override maximum stdin bytes (default: from config) |
| `--no-redact` | Disable secret redaction (NOT recommended) |
| `--file <PATH>` | Read input from file instead of stdin |
| `-V, --version` | Print version information |
| `-h, --help` | Print help |

## Examples

```sh
# Basic usage
git diff HEAD~1 | lxpr

# JSON output for scripting
git diff HEAD~1 | lxpr --json | jq .title

# Write PR body to file
git log -p -1 | lxpr > pr-body.md

# Preview redacted input without calling the LLM
git diff HEAD~1 | lxpr --dry-run

# German output
git diff HEAD~1 | lxpr --lang de
```

## Security

Input is always piped through `lx-redact` before reaching the LLM provider,
masking API keys, tokens, connection string passwords, JWTs, and other secrets.

Use `--dry-run` to inspect the redacted input before it leaves your machine.

Use `--no-redact` only after manually auditing the diff for secrets.
