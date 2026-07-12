# lxlog

Summarise and triage log output, surfacing anomalies, errors, and root causes.

## Usage

```sh
# Analyse logs from stdin (any format: syslog, JSON logs, application logs)
journalctl -u nginx --since "1 hour ago" | lxlog

# Analyse a log file
lxlog --file /var/log/app.log

# Get structured JSON output
lxlog --file app.log --json
```

## Example Output

```
$ journalctl -u postgres --since "1 hour ago" | lxlog
Anomalies:
  [ERROR] line 14 — Connection pool exhausted (repeated 47 times)
  [WARN]  line 3  — High memory usage: 94%

Summary: 47 connection timeout errors suggest the database has been unreachable
since 14:23. Memory pressure at 94% may be contributing to the connection pool
exhaustion. Investigate database host connectivity and consider increasing the
pool timeout.
```

```json
$ lxlog --file app.log --json
{
  "anomalies": [
    {
      "line": 14,
      "level": "ERROR",
      "message": "Connection pool exhausted — repeated 47 times in rapid succession, indicating a connection leak or database unavailability"
    },
    {
      "line": 3,
      "level": "WARN",
      "message": "Memory usage at 94% — approaching critical threshold"
    }
  ],
  "summary": "Critical resource issue: repeated connection pool exhaustion combined with high memory usage suggests the database has been unreachable since 14:23."
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show redacted input that would be sent to the LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |
| `--file <PATH>` | Read input from file instead of stdin |
| `--path <DIR>` | Restrict file access to this directory |
| `--no-redact` | Disable secret redaction (not recommended — logs may contain credentials or PII) |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (missing/invalid args) |
| 5 | Security abort (redaction failed) |

## Security

- Log files often contain tokens, passwords, and PII in request parameters and error messages. All input is redacted through `lx-redact` before reaching the LLM.
- `--no-redact` disables redaction and prints a prominent warning; do not use in shared or logged environments.
- Treats all input as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the log lines.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
