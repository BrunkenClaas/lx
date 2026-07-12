# lxcommit

Generate a [Conventional Commit](https://www.conventionalcommits.org/) message from a staged git diff.

## Usage

```sh
# Typical usage: pipe staged diff into lxcommit
git diff --staged | lxcommit

# JSON output
git diff --staged | lxcommit --json

# Preview what gets sent to the LLM (redacted)
git diff --staged | lxcommit --dry-run
```

## Example Output

```
$ git diff --staged | lxcommit
feat(auth): add token refresh method

Allows callers to exchange a refresh token for a new access token without re-authenticating.
```

## Security

**Redaction is mandatory.** Before the diff is sent to the LLM, `lx-redact` scans it for
secrets and API keys. Any detected secrets are replaced with `[REDACTED]`. If redaction fails,
the tool exits with code 5 rather than sending raw secrets.

Use `--no-redact` only if you have verified there are no secrets in the diff. A prominent
warning is printed to stderr.

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON `{type, scope, subject, body}` |
| `--plain` | No ANSI formatting |
| `--dry-run` | Show redacted diff without sending |
| `-q, --quiet` | Suppress stderr messages |
| `--no-redact` | Skip secret redaction (warns on stderr) |
| `--lang <code>` | Output language (BCP-47) |
| `--max-input-bytes <n>` | Override diff size limit |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | No diff provided |
| 5 | Redaction failed (security abort) |
