# lxchmod

Suggest safe file permissions from `ls -l` output, applying the principle of least privilege.

## Usage

```sh
# Pipe ls -l output and get a chmod suggestion
ls -l config.json | lxchmod

# Analyse the current directory listing
ls -la | lxchmod

# Read ls output from a file
lxchmod --file listing.txt

# Get structured JSON output
ls -l id_rsa | lxchmod --json
```

## Example Output

```
$ ls -l config.json | lxchmod
chmod 600 config.json
# Config files should not be executable or world-readable. 600 restricts access to the owner only, protecting sensitive configuration values.
```

```json
$ ls -l id_rsa | lxchmod --json
{
  "suggestion": "chmod 600 id_rsa",
  "reason": "Private SSH keys must be 600 (owner-only read/write) to prevent unauthorized access. Any broader permission allows other users on the system to read the key.",
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
| `--file <PATH>` | Read input from file instead of stdin |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (config/auth, network, or LLM error) |
| 2 | Bad usage (missing/invalid args) |
| 3 | Dangerous output — use `--allow-dangerous` to get exit 0 |
| 5 | Security abort (fsbound violation or redaction failure) |

## Security

- The tool outputs a suggested `chmod` command as plain text only. It never executes that command.
- When `--file` is used, the path is resolved and checked against filesystem boundaries. Paths outside the current working directory are refused with exit code 5.
- The model is instructed to ignore any instructions embedded in the `ls -l` data.
- Dangerous patterns (world-writable files, executable bits on data files, overly permissive config files) are flagged on stderr and never silently ignored.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
