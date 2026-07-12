# lxsh

Generate a shell command from a plain-English description.

## Usage

```sh
lxsh "find all Python files larger than 1MB"
lxsh "compress the logs directory into logs.tar.gz"
echo "list processes using more than 1GB of memory" | lxsh
lxsh "delete all .tmp files" --json
```

## Example Output

```
$ lxsh "find all Python files larger than 1MB"
find . -name "*.py" -size +1M
```

## Security

**lxsh never executes the generated command.** It outputs text to stdout only.

Before output, a deterministic local pattern check scans for dangerous patterns:
- `rm -rf /` or variations
- `curl | sh` (untrusted remote script execution)
- `dd of=/dev/…` (direct device writes)
- `mkfs` (filesystem creation)
- `Invoke-Expression` / `iwr | iex` (PowerShell remote execution)
- Fork bombs

If a dangerous pattern is found, a prominent warning is printed to **stderr**,
`dangerous: true` is set in JSON output, and the tool exits with code 3.
The command is still printed to stdout for review. Pass `--allow-dangerous` to exit 0 instead.

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON `{command, shell, dangerous}` |
| `--plain` | No ANSI formatting |
| `--dry-run` | Show description without sending to LLM |
| `-q, --quiet` | Suppress stderr messages (DANGER warnings are never suppressed) |
| `-D, --allow-dangerous` | Exit 0 even when output is dangerous (warning still printed to stderr) |
| `--shell <shell>` | Target shell: `bash`, `zsh`, `sh`, `fish`, `powershell`, `cmd` (auto-detected if omitted) |
| `--lang <code>` | Output language (BCP-47) |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (config/auth, network, or LLM error) |
| 2 | No description provided |
| 3 | Dangerous output — use `--allow-dangerous` to get exit 0 |
| 5 | Security abort |
