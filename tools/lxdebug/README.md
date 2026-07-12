# lxdebug

Analyse error output — single errors or multi-error logs — and suggest root causes
and fixes. Pipe any stderr output into `lxdebug` to get a structured diagnosis.

## Usage

```
# Basic usage — pipe error output
./my-app 2>&1 | lxdebug

# From a file
lxdebug < error.log

# JSON output for scripting
./my-app 2>&1 | lxdebug --json

# Preview the redacted input that would be sent to the LLM
./my-app 2>&1 | lxdebug --dry-run
```

## Example

Input (via stdin):
```
Error: Cannot find module 'express'
    at Function.Module._resolveFilename (node:internal/modules/cjs/loader:1039:15)
```

Output:
```
Cause:  The Node.js module 'express' is not installed in the project's node_modules directory.

Fix:    Install the missing dependency using npm or yarn.

Run:    npm install express
```

JSON output (`--json`):
```json
{
  "cause": "The Node.js module 'express' is not installed in the project's node_modules directory.",
  "fix": "Install the missing dependency using npm or yarn.",
  "command": "npm install express"
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show redacted input that would be sent to the LLM, then exit |
| `-q, --quiet` | Suppress diagnostic messages on stderr |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider/lang info on stderr |
| `--max-input-bytes <n>` | Maximum bytes to read from stdin (default: 512 KiB) |
| `--no-redact` | Disable secret redaction (NOT recommended) |
| `-V, --version` | Print version |
| `-h, --help` | Print help |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (e.g. no input provided) |
| 5 | Security abort (redaction failed) |

## Security

- **`redact`**: All input is automatically redacted through `lx-redact` before
  being sent to the LLM. API keys, tokens, and other secrets embedded in error
  output are masked. Use `--no-redact` only if you have audited the input.
- **`untrusted`**: The system prompt instructs the model to ignore any
  instructions embedded in the error output (prompt injection defence).
- **`nocmd`**: The suggested command is output as text only and is **never**
  executed by `lxdebug`. You decide whether and how to run it.
