# lxconf

Check a config file for typical errors and misconfigurations.

Reads config file content (TOML, YAML, INI, JSON, `.env`, or any text-based
format) and returns a list of findings — errors, warnings, and informational
notices — identified by an LLM.

## Usage

```sh
# Check a config file for errors (read-only)
lxconf --file myapp.toml
cat /etc/app/config.yaml | lxconf

# Edit mode (pipe existing config) — apply described change only
lxconf "set log level to warn" < myapp.toml
lxconf "enable TLS" < nginx.conf | lxdiff

# Machine-readable JSON output
lxconf --file myapp.toml --json

# Restrict file access to a specific directory (fsbound)
lxconf --file configs/app.toml --root ./configs
```

In edit mode `lxconf` changes **only what the intent describes** — comments,
whitespace, and unrelated keys are preserved verbatim. The result goes to stdout;
the tool never writes to the original file.

## Output

Plain mode (stdout): one finding per line in the format:

```
[severity] line N: message
  hint: how to fix it
```

JSON mode (`--json`):

```json
{
  "findings": [
    {
      "line": 3,
      "severity": "error",
      "message": "port value 99999 is outside the valid range 1-65535",
      "hint": "Use a port number between 1 and 65535"
    }
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--file <PATH>` | Read config from file instead of stdin |
| `--root <DIR>` | Restrict file access to this directory (fsbound) |
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show redacted input without calling the LLM |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider/redaction info on stderr |
| `--max-input-bytes <n>` | Limit bytes read from stdin or file |
| `--no-redact` | Disable secret redaction (NOT recommended) |
| `--version` / `-V` | Print version and exit |
| `--help` / `-h` | Show help and exit |

## Security flags

- **redact**: Config file content is redacted before being sent to the LLM.
  Credentials, connection strings, and other sensitive values are masked.
- **fsbound**: When `--file` is used, the resolved path must stay within the
  allowed root (default: current directory). Symlinks that escape the root are
  rejected with exit code 5.
- **untrusted**: The system prompt instructs the LLM to ignore any instructions
  embedded in the config file content.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (config/network/LLM) |
| 2 | Bad usage (missing input, unknown flag) |
| 5 | Security abort (redaction failed or fsbound violation) |
