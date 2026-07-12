# lxkill

Generate the exact shell command to find and kill a described process.

## Usage

```
lxkill [OPTIONS] <DESCRIPTION>
```

Pipe `ps` or `ss` output as stdin for additional context (optional).

## Examples

```bash
# Kill a process by port
lxkill "process listening on port 3000"
# Output: kill $(lsof -ti:3000)

# Kill a named process
lxkill "the nginx worker processes"
# Output: pkill -f 'nginx: worker'

# With context from ps
ps aux | lxkill "the stalled python worker"

# Get JSON output with target and reason
lxkill --json "zombie processes"
```

## OS target

By default `lxkill` generates commands for the **host OS**. Use `--target` to
target a different platform:

```sh
# Linux (kill/pkill/lsof)
lxkill "process listening on port 3000"           # default on Linux

# Windows (Stop-Process/Get-NetTCPConnection)
lxkill --target windows "process on port 3000"

# macOS (kill/pkill/lsof)
lxkill --target macos "stalled python worker"
```

## Flags

| Flag | Description |
|------|-------------|
| `--target <linux\|windows\|macos>` | OS to generate commands for (default: host OS) |
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show input and system prompt, then exit |
| `--quiet` / `-q` | Suppress stderr diagnostics (DANGER warnings always shown) |
| `--allow-dangerous` / `-D` | Exit 0 even when dangerous pattern detected (warning still shown) |
| `--lang <BCP-47>` | Output language (default: auto-detected) |
| `--verbose` | Show verbose diagnostics |
| `--max-input-bytes <n>` | Limit stdin context bytes |
| `--file <PATH>` | Read process context from file |
| `--version` / `-V` | Print version |
| `--help` / `-h` | Print help |

## Output

**Plain mode** (stdout): the kill command only.
**JSON mode** (stdout): `{"command":"...","target":"...","reason":"...","dangerous":false}`

Explanations go to stderr and are suppressed by `--quiet`.

## Security

- `nocmd`: This tool **never executes** any command. It only prints text.
- `untrusted`: Piped `ps`/`ss` output is treated as untrusted data.
- Dangerous patterns (killing PID 1, init, systemd, broadcast kills) are detected locally and reported on stderr. Exit code 3 is returned unless `--allow-dangerous` is set.
