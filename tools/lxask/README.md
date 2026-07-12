# lxask

Ask a free-form question about piped text or a context file.

## Usage

```sh
# Ask a question about a piped document
cat README.md | lxask "what does this tool do?"

# Ask a question and supply context via --context
lxask "what port does the service listen on?" --context config.yaml

# Ask a general knowledge question with no context
lxask "what is the difference between TCP and UDP?"

# Get structured JSON output
cat deployment.md | lxask "which environment is this for?" --json
```

## Example Output

```
$ cat README.md | lxask "what does this tool do?"
lxask answers free-form questions about text piped on stdin or provided via --context. It uses an LLM to find the answer within the supplied context, or falls back to general knowledge when no context is given.
```

```json
$ cat README.md | lxask "what does this tool do?" --json
{
  "answer": "lxask answers free-form questions about text piped on stdin or provided via --context. It uses an LLM to find the answer within the supplied context, or falls back to general knowledge when no context is given.",
  "sources": ["provided context"]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show redacted input without sending to LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |
| `--file <PATH>` | Read question from file instead of stdin |
| `--context <PATH>` | Context file to include alongside the question |
| `--no-redact` | Disable automatic secret redaction (use with caution) |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (missing/invalid args) |
| 5 | Redaction failed (security abort) |

## Security

- All input (both question and context) is treated as untrusted data. The system prompt instructs the model to ignore any instructions embedded in the user-provided text.
- Both the question and any context file are automatically redacted before being sent to the LLM. Use `--no-redact` only when you are certain neither contains secrets; a warning is printed when it is active.
- Context files are subject to filesystem boundary checks: paths pointing to sensitive system directories (`.ssh`, `.aws`, `/proc`, `System32`) are refused with exit code 5.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
