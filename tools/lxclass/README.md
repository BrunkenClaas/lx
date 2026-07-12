# lxclass

Classify text into one of a set of user-defined labels.

## Usage

```sh
# Classify stdin text into one of the given labels
echo "Server is down, users cannot log in." | lxclass urgent,normal,low

# Classify a file
lxclass bug,feature,question,docs --file issue.txt

# Get structured JSON output with confidence scores for all labels
echo "How do I configure the timeout?" | lxclass bug,feature,question,docs --json
```

## Example Output

```
$ echo "Server is down, users cannot log in." | lxclass urgent,normal,low
urgent
# confidence: 0.96
```

```json
$ echo "How do I configure the timeout?" | lxclass bug,feature,question,docs --json
{
  "label": "question",
  "confidence": 0.92,
  "all": [
    { "label": "bug",      "confidence": 0.02 },
    { "label": "feature",  "confidence": 0.03 },
    { "label": "question", "confidence": 0.92 },
    { "label": "docs",     "confidence": 0.03 }
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON (includes confidence scores for all labels) |
| `--plain` | No ANSI colours |
| `--dry-run` | Show input without sending to LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |
| `--file <PATH>` | Read input from file instead of stdin |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (missing/invalid args) |

## Security

- All input is treated as untrusted data. The system prompt instructs the model to classify the text only, ignoring any instructions that may be embedded in it.
- Plain-mode stdout contains only the winning label, making it safe to use in pipelines.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
