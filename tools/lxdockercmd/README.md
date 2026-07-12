# lxdockercmd

Generate a Docker command from a plain-English description.

## Usage

```sh
# Generate a command from a positional description
lxdockercmd "run nginx on port 8080"

# Pipe a description from stdin
echo "start a postgres 15 container with a persistent volume" | lxdockercmd

# Get structured JSON output including danger flag
lxdockercmd "run a container with full host privileges" --json
```

## Example Output

```
$ lxdockercmd "run nginx on port 8080"
docker run -d -p 8080:80 nginx
```

```
$ lxdockercmd "remove all stopped containers"
DANGER: generated command matches a destructive pattern (rm -f)
docker container prune -f
```

```json
$ lxdockercmd "run nginx on port 8080" --json
{
  "command": "docker run -d -p 8080:80 nginx",
  "dangerous": false
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show input without sending to LLM |
| `-q, --quiet` | Suppress stderr diagnostics (DANGER warnings are never suppressed) |
| `-D, --allow-dangerous` | Exit 0 even when output is dangerous (warning still printed to stderr) |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |
| `--file <PATH>` | Read description from file instead of stdin |

The description can be supplied as a positional argument, via `--file`, or piped to stdin.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (config/auth, network, or LLM error) |
| 2 | Bad usage (no description provided) |
| 3 | Dangerous output — use `--allow-dangerous` to get exit 0 |
| 5 | Security abort |

## Security

- The tool never executes the generated command; it only prints it to stdout.
- Dangerous patterns are detected locally before the LLM call and flagged prominently on stderr: `--privileged`, `--cap-add SYS_ADMIN`, `-v /:/host`, `rm -f`, `container prune`, `image prune`, `system prune`. These warnings are never suppressed, even with `--quiet`.
- Input is treated as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the description.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
