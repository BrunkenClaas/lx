# lxperm

Explain file permissions and their security risks.

## Usage

```
# Pipe ls -l output
ls -l | lxperm

# Scan a directory (fsbound — stays within the given path)
lxperm --file /path/to/dir

# Read a saved ls -l output from a file
lxperm --file listing.txt

# Output as JSON
ls -l | lxperm --json

# Quiet (suppress stderr diagnostics)
ls -l | lxperm --quiet
```

## Output

Plain mode (stdout — explanation IS the result):

```
deploy.sh  [-rwxr-xr-x]  risk: warning
  Owner can read, write, and execute. Group and others can read and execute.
  This script is world-executable — anyone on the system can run it.

config.txt  [-rw-r--r--]  risk: standard
  Owner can read and write; group and others can only read.
  Standard read-only sharing with no security concerns.
```

JSON mode:

```json
{
  "items": [
    {
      "perm": "-rwxr-xr-x",
      "file": "deploy.sh",
      "risk": "warning",
      "explanation": "Owner can read, write, and execute. Group and others can read and execute. This script is world-executable — anyone on the system can run it."
    }
  ]
}
```

## Risk levels

| Level    | Meaning |
|----------|---------|
| critical | SUID/SGID on non-standard files, world-writable directories |
| warning  | World-executable scripts, group-writable system files, SUID/SGID on any file |
| info     | Unusual but not dangerous permissions |
| standard | Common, expected permissions with no security concerns |

## Security flags

- **nonet**: No network calls other than the LLM completion.
- **fsbound**: When `--file` points to a directory, only files within that directory are scanned. Symlinks that escape the root are rejected.

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent to the LLM without calling it |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider/lang info on stderr |
| `--max-input-bytes <n>` | Maximum bytes to read from stdin (default: 512 KiB) |
| `--file <PATH>` | Read from file or scan directory (fsbound) |
| `--version` / `-V` | Print version information |
| `--help` / `-h` | Show help |
