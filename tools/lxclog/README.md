# lxclog

Generate a Keep-a-Changelog changelog from git log output.

## Usage

```sh
git log --oneline | lxclog
git log --format="%h %s" | lxclog
git log --oneline | lxclog --json
lxclog --file git_log.txt
```

## Input

Pipe any `git log` output into `lxclog`. Supports oneline format, full format,
or anything readable. The tool sends it (after redaction) to the LLM and gets
back a structured changelog.

## Output

**Plain mode (default):** Keep-a-Changelog markdown to stdout.

```markdown
# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added

- Add token refresh endpoint
- Add --json output flag to all commands

### Fixed

- Handle null response from upstream
```

**JSON mode (`--json`):**

```json
{
  "entries": [
    {
      "version": "Unreleased",
      "date": "",
      "added": ["Add token refresh endpoint"],
      "fixed": ["Handle null response from upstream"]
    }
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show redacted input without calling LLM |
| `-q`, `--quiet` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes <n>` | Limit stdin read size |
| `--no-redact` | Skip secret redaction (not recommended) |
| `--file <PATH>` | Read from file instead of stdin |
| `-V`, `--version` | Print version |
| `-h`, `--help` | Print help |

## Security

`lxclog` has the `redact` security flag. All input is passed through
`lx-redact` before being sent to the LLM. Secrets, API keys, and PII
are masked to `[REDACTED]` automatically. Use `--dry-run` to inspect
the redacted content before sending.

Use `--no-redact` only if you have audited the git log and accept that
its full content will reach your LLM provider.
