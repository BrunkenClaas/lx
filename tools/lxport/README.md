# lxport

Explain what service runs on a port and flag any security risk.

## Usage

```
lxport <port> [OPTIONS]
lxport <port> < netstat_output.txt
```

## Examples

```sh
lxport 22
lxport 5432 --json
ss -tlnp | lxport 443
lxport 8080 --file context.txt
```

## Output

Plain mode prints the explanation to stdout; service name and risk level go to stderr:

```
Port 22 is the standard SSH (Secure Shell) port used for remote login...
# service: SSH | risk: medium
```

When invoked without piped context, a tip is printed to stderr:

```
# tip: pipe ss/netstat output for machine-specific results (e.g. ss -tlnp | lxport 22)
```

JSON mode (--json) emits the full object to stdout:

```json
{
  "port": 22,
  "likely_service": "SSH",
  "explanation": "...",
  "risk": "medium"
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show input and system prompt without calling the LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show token usage on stderr |
| `--max-input-bytes <n>` | Limit stdin context size |
| `--file <PATH>` | Read network context from file |
| `-V, --version` | Print version information |
| `-h, --help` | Print help |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (network, LLM, config) |
| 2 | Bad usage (invalid port, unknown flag) |
