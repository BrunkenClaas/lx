# lxnotes

Summarise and structure meeting notes or freeform text into organised sections.

## Usage

```sh
# Pipe raw meeting notes from stdin
pbpaste | lxnotes

# Read notes from a file
lxnotes --file meeting-2026-05-31.txt

# Get structured JSON output
lxnotes --file standup.txt --json

# Extract action items (who/what/due) instead of structured notes
lxnotes --actions --file kickoff.txt
```

## Example Output

```
$ lxnotes --file kickoff.txt
Decisions
  - Use PostgreSQL for the new service
  - Deploy window moved to Thursdays at 9pm

Action Items
  - Alice to set up the schema by Friday
  - Bob to provision the staging environment

Discussion Points
  - Staging environment is slow (raised by Carol)
  - Budget confirmation still pending from finance
```

```json
$ lxnotes --file kickoff.txt --json
{
  "sections": [
    {
      "title": "Decisions",
      "content": [
        "Use PostgreSQL for the new service",
        "Deploy window moved to Thursdays at 9pm"
      ]
    },
    {
      "title": "Action Items",
      "content": [
        "Alice to set up the schema by Friday",
        "Bob to provision the staging environment"
      ]
    },
    {
      "title": "Discussion Points",
      "content": [
        "Staging environment is slow (raised by Carol)",
        "Budget confirmation still pending from finance"
      ]
    }
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--actions` | Output extracted action items (owner, task, due date) instead of structured notes |
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show redacted input that would be sent to the LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |
| `--file <PATH>` | Read input from file instead of stdin |
| `--no-redact` | Disable secret redaction (not recommended — notes may contain credentials or PII) |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (missing/invalid args) |
| 5 | Security abort (redaction failed) |

## Security

- Meeting notes may contain passwords, tokens, or personal data. All input is redacted through `lx-redact` before reaching the LLM.
- `--no-redact` disables redaction and prints a prominent warning.
- Treats all input as untrusted data: the system prompt instructs the model to ignore any instructions embedded in the notes.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
