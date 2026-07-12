# lxrsync

Generate an rsync command from a plain-English description.

## Usage

```
lxrsync [OPTIONS] [DESCRIPTION]
```

Provide a natural-language description of what you want to sync and `lxrsync`
will generate the appropriate `rsync` command. The command is printed to stdout.
It is **never executed**.

```bash
lxrsync "copy /home/user/docs to backup@remote:/backup/docs"
# rsync -avz /home/user/docs/ backup@remote:/backup/docs/

lxrsync --json "sync /var/www to web@server:/var/www and delete stale files"
# {"command":"rsync -avz --delete /var/www/ web@server:/var/www/","dangerous":true}
```

## Options

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object (`{"command":"...","dangerous":bool}`) to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Print what would be sent to the LLM, then exit without calling it |
| `-q, --quiet` | Suppress diagnostic stderr (DANGER warnings are never suppressed) |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider/lang diagnostics on stderr |
| `--max-input-bytes <n>` | Limit stdin input size |
| `--file <PATH>` | Read description from file instead of positional arg |
| `-V, --version` | Print version in canonical suite format |
| `-h, --help` | Show help |

## Security (nocmd)

`lxrsync` implements the `nocmd` security flag:

- The command is **never executed** — only printed to stdout.
- Local pattern matching detects destructive rsync options (e.g. `--delete`,
  `--delete-before`, syncing from `/`, piping to a shell).
- When a dangerous pattern is detected, a `DANGER:` warning is printed to stderr
  and the `dangerous` field in JSON output is set to `true`.
- DANGER warnings are **never** suppressed by `--quiet`.

## Output

**Plain mode** (default): only the `rsync` command on stdout (pipe-safe).

```
rsync -avz /home/user/docs/ backup@remote:/backup/docs/
```

**JSON mode** (`--json`):

```json
{
  "command": "rsync -avz /home/user/docs/ backup@remote:/backup/docs/",
  "dangerous": false
}
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (network, LLM, config) |
| 2 | Bad usage (missing or invalid arguments) |
| 5 | Security abort |
