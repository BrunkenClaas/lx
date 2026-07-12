# lxstandup

Generate a standup summary from git activity or work notes.

Reads git log output, ticket updates, or raw work notes from stdin and
produces a structured standup with done items, next steps, and blockers.

## Usage

```
git log --oneline --since=yesterday | lxstandup
lxstandup --file work_notes.txt
lxstandup --json
```

## Output (plain)

```
Done:
- Implemented export endpoint for reports
- Fixed null pointer in data parser

Next:
- Review open pull requests

Blockers:
(none)
```

## Output (--json)

```json
{
  "done": ["Implemented export endpoint for reports", "Fixed null pointer in data parser"],
  "next": ["Review open pull requests"],
  "blockers": []
}
```

## Security flags

`redact` — input is redacted before being sent to the LLM. Git commit messages
and work notes may contain secrets. Use `--no-redact` only after auditing the
input (prominent warning is shown).

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show redacted input without calling the LLM |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model and provider info on stderr |
| `--max-input-bytes <n>` | Limit input size (default: 512 KiB) |
| `--file <PATH>` | Read from file instead of stdin |
| `--no-redact` | Skip redaction (NOT recommended) |
| `--version` / `-V` | Print version information |
| `--help` / `-h` | Show help |
