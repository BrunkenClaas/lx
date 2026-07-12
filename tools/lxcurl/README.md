# lxcurl

Generate a `curl` command from a plain-English API description.

## Usage

```
lxcurl [OPTIONS] [DESCRIPTION]
```

If `DESCRIPTION` is omitted, the tool reads from stdin.

## Examples

```sh
lxcurl "GET all users from https://api.example.com/users"
# curl -s https://api.example.com/users

lxcurl "POST {\"name\":\"Alice\"} to https://api.example.com/users"
# curl -s -X POST https://api.example.com/users -H 'Content-Type: application/json' -d '{"name":"Alice"}'

echo "DELETE resource at https://api.example.com/users/42" | lxcurl
# curl -s -X DELETE https://api.example.com/users/42
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object `{"command":"...","dangerous":bool}` |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent to the LLM, then exit |
| `--quiet / -q` | Suppress diagnostic stderr messages (DANGER warnings are never suppressed) |
| `-D, --allow-dangerous` | Exit 0 even when output is dangerous (warning still printed to stderr) |
| `--lang <BCP-47>` | Output language (default: auto-detected) |
| `--verbose` | Show model/provider/lang diagnostics on stderr |
| `--max-input-bytes <n>` | Limit stdin read size |
| `--file <PATH>` | Read description from a file |
| `--version / -V` | Print version information |
| `--help / -h` | Print help |

## Security (nocmd)

`lxcurl` only **outputs** the generated command — it never executes it.

Before printing, the tool runs local danger detection. If the generated command
matches a dangerous pattern, a prominent `DANGER:` warning is printed to stderr
and `dangerous: true` is set in the JSON output. Dangerous patterns include:

- Piping curl output to a shell (`| sh`, `| bash`, `| iex`, etc.)
- Writing to system directories (`--output /etc/`, `-o /usr/`, etc.)
- Reading system files via `file:///etc/` URIs

DANGER warnings are never suppressed, even with `--quiet`. When a dangerous pattern
is detected the tool exits 3; pass `--allow-dangerous` to get exit 0 instead.

## Output

**Plain mode (default):** The curl command is printed to stdout. Diagnostics go to stderr.

**JSON mode (`--json`):**
```json
{
  "command": "curl -s https://api.example.com/users",
  "dangerous": false
}
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (config, network, LLM) |
| 2 | Bad usage (missing or invalid arguments) |
| 3 | Dangerous output — use `--allow-dangerous` to get exit 0 |
| 5 | Security abort |
