# lxundo

Suggest how to undo a recent action or command.

## Usage

```sh
# Describe what you did as an argument
lxundo "I ran git reset --hard HEAD~2"

# Pipe the description from stdin
echo "I deleted a branch called feature/login" | lxundo

# Get structured JSON output
lxundo --json "I dropped a table in PostgreSQL by accident"
```

## Example Output

```
$ lxundo "I ran git reset --hard HEAD~2"
Use git reflog to find the commit hash from before the reset, then
restore it with git reset --hard.

Command: git reflog
         git reset --hard <hash-before-reset>

Warning: git reset --hard cannot be undone if the reflog entry expires.
         Reflog entries are kept for 90 days by default.
```

```json
$ lxundo --json "I ran git reset --hard HEAD~2"
{
  "steps": [
    "Run git reflog to list recent HEAD positions",
    "Identify the commit hash that was current before the reset",
    "Run git reset --hard <that-hash> to restore it"
  ],
  "command": "git reflog",
  "warning": "Reflog entries expire after 90 days. Act promptly.",
  "reversible": true
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
| `--file <PATH>` | Read input from file instead of stdin |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (no input) |

## Security

- Does not execute any command it suggests. Output is printed to stdout only; the user must run suggested commands explicitly.
- Dangerous undo patterns (e.g. `DROP TABLE`, `rm -rf`, forced overwrites) are flagged prominently on stderr.
- Treats all input as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the description.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
