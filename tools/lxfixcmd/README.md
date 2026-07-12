# lxfixcmd

Fix the last failed shell command.

## Usage

```
lxfixcmd <failed-command> [< error-output]
```

Pass the failed command as a positional argument. Optionally pipe the error output from stderr via stdin.

## Examples

```sh
# Fix a typo in a git command
lxfixcmd "git psh origin main"
# → git push origin main

# Fix with error context from stderr
cargo buidl 2>&1 | lxfixcmd "cargo buidl"
# → cargo build

# JSON output
lxfixcmd --json "docker run ubunut"
# → {"command":"docker run ubuntu","reason":"'ubunut' is a typo for 'ubuntu'","dangerous":false}

# Explicitly target PowerShell (auto-detected by default)
lxfixcmd --shell powershell "ls -la"
# → Get-ChildItem -Force
```

## Security

- `nocmd`: lxfixcmd generates text only and never executes commands.
- `untrusted`: the system prompt instructs the model to ignore instructions in user data.
- Local dangerous-pattern scanning runs on every generated command before output.
- Use `--allow-dangerous` / `-D` to suppress exit 3 on dangerous output (warning still fires).

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show input/prompt without calling LLM |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--allow-dangerous` / `-D` | Exit 0 even if dangerous pattern detected |
| `--shell <SHELL>` | Target shell (`bash`, `zsh`, `powershell`, `cmd`); auto-detected if omitted |
| `--lang <BCP-47>` | Output language |
| `--verbose` | Show verbose diagnostics |
| `--max-input-bytes <n>` | Limit stdin bytes |
| `--file <PATH>` | Read error context from file |
| `--version` / `-V` | Print version |
| `--help` / `-h` | Print help |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (network, LLM, config) |
| 2 | Bad usage |
| 3 | Dangerous pattern detected (use `-D` to override) |
