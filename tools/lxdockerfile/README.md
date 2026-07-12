# lxdockerfile

Generate a Dockerfile from a plain-English stack description.

## Usage

```sh
# Create mode (no stdin) — generate from scratch
lxdockerfile "Node.js 18 app with npm, exposes port 3000"
lxdockerfile "Go 1.22 binary, multi-stage build, port 9000" > Dockerfile

# Edit mode (pipe existing file) — apply described change only
lxdockerfile "add a healthcheck" < Dockerfile
lxdockerfile "switch base image to alpine" < Dockerfile > Dockerfile.new

# Review the edit before applying
lxdockerfile "add a healthcheck" < Dockerfile | lxdiff

# JSON output
lxdockerfile --json "Node.js 18 app"
```

In edit mode `lxdockerfile` changes **only what the intent describes** and
preserves everything else verbatim — comments, whitespace, unrelated lines.

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON to stdout (`{"content":"...","dangerous":false}`) |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show input that would be sent to LLM, then exit |
| `-q, --quiet` | Suppress stderr diagnostics (DANGER warnings are never suppressed) |
| `--lang <BCP-47>` | Output language, e.g. `en`, `de`, `fr` (default: auto-detect) |
| `--verbose` | Show model/provider/lang on stderr |
| `--max-input-bytes <n>` | Limit stdin bytes read (default: 512 KiB) |
| `--file <PATH>` | Read description from file instead of stdin |
| `-V, --version` | Print version in canonical suite format |
| `-h, --help` | Show help |

## Security (nocmd)

The generated Dockerfile is printed to stdout only — never built or executed.
Local pattern detection warns on stderr for dangerous instructions:

- `curl | sh`, `wget | sh`, `| bash` — piping into a shell
- `--privileged` — bypasses Docker security isolation
- `rm -rf /` — recursive filesystem deletion
- `> /dev/` — direct device writes

DANGER warnings are always shown on stderr even with `--quiet`.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (config, network, LLM) |
| 2 | Bad usage (missing or invalid arguments) |
| 5 | Security abort |
