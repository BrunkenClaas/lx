# lxredact

Mask secrets and PII in a data stream.

## Usage

```
lxredact [OPTIONS]
```

Reads from stdin (or `--file`) and writes the redacted text to stdout.
Secrets and PII are replaced with `[REDACTED]`, `[EMAIL]`, `[IP]`, etc.

## Options

| Flag | Description |
|------|-------------|
| `--anon` | Replace names/roles with placeholders (`[Person A]`, `[Company B]`) instead of masking secrets |
| `--strict` | Mask more aggressively (see "Strict mode" below) |
| `--explain` | Use LLM to explain what was redacted (never sends values) |
| `--json` | Output JSON summary instead of redacted text |
| `--plain` | Disable ANSI formatting |
| `--quiet / -q` | Suppress diagnostic messages on stderr |
| `--no-redact` | Disable redaction (WARNING: secrets are NOT masked) |
| `--file <PATH>` | Read input from file instead of stdin |
| `--max-input-bytes <N>` | Limit input size (default: 512 KiB) |
| `--lang <BCP-47>` | Output language for explain mode (default: auto) |
| `--dry-run` | Show what would happen without redacting |
| `--verbose` | Show verbose diagnostics on stderr |
| `--version / -V` | Print version information |
| `--help / -h` | Show help |

## Security flags

- `nonet` — no network calls by default; LLM is only used with `--explain`
- `redact` — this tool IS the redaction trust base; it is fully local by default

## Redaction levels

**Standard** (default): Redacts API keys, tokens, passwords, private key blocks,
JWTs, connection strings, high-entropy secrets, and email addresses. Covers the
ubiquitous, near-zero-false-positive prefixed formats (AWS, GitHub, GitLab, GCP,
Slack, Stripe, SendGrid, Twilio, npm, Anthropic, …).

**`--strict`**: A single flag that bundles *both*:
- **PII masking** — IPv4 addresses, public hostnames, and local file paths
  (`/home/user/...`, `C:\Users\...`); and
- **Aggressive secret detection** — an expanded set of niche service prefixes
  (Shopify, DigitalOcean, Hugging Face, Linear, Postman, Doppler, Atlassian,
  Cloudflare, Heroku, Telegram, Discord, PyPI, GitLab runner, Square).

### Entropy gate (applies to every level)

Every prefixed detector pairs its prefix+length match with a **Shannon-entropy
floor** (2.0–4.0 bits/byte, the same thresholds gitleaks uses) and a placeholder
filter. A string that merely *starts* with a known prefix is only masked if its
value is high-entropy and does not look like a documentation example.

This rejects placeholders (`sk-your_api_key_here_…`, AWS's own
`AKIAIOSFODNN7EXAMPLE`) and mechanically repetitive junk (`sk_live_abcabcabc…`).
It does **not** reject every false positive: a value built from English words
(e.g. `sk_live_televisionchannelnumberone`) has entropy comparable to real keys
and will still be masked. The gate is a strong filter for the common cases, not a
guarantee.

## Detected patterns

| Pattern | Example | Placeholder |
|---------|---------|-------------|
| OpenAI / generic API key | `sk-abc123...` | `[REDACTED]` |
| Anthropic key | `sk-ant-api03-...` | `[REDACTED]` |
| AWS access key | `AKIAIOSFODNN7...` | `[REDACTED]` |
| GCP API key | `AIza...` | `[REDACTED]` |
| GitHub token | `ghp_...` | `[REDACTED]` |
| Private key block | `-----BEGIN ... PRIVATE KEY-----` | `[REDACTED]` |
| JWT | `eyJ...eyJ...sig` | `[REDACTED]` |
| Connection string password | `postgres://user:PASS@host` | `[REDACTED]` |
| Context-keyed secrets | `password=VALUE` | `[REDACTED]` |
| High-entropy base64 | long base64 after `=` or `:` | `[REDACTED]` |
| GitLab PAT | `glpat-...` | `[REDACTED]` |
| Slack token / webhook | `xoxb-...` | `[REDACTED]` |
| Stripe key | `sk_live_...` | `[REDACTED]` |
| SendGrid / Twilio / npm | `SG.` / `SK<hex>` / `npm_...` | `[REDACTED]` |
| Email (standard+) | `alice@example.com` | `[EMAIL]` |
| IPv4 address (`--strict`) | `192.168.1.1` | `[IP]` |
| Hostname (`--strict`) | `api.example.com` | `[HOST]` |
| Local path (`--strict`) | `/home/user/...` | `[PATH]` |
| Niche service prefixes (`--strict`) | `shpat_`, `dop_v1_`, `hf_`, `ATATT3`, … | `[REDACTED]` |

## Examples

```bash
# Redact secrets in a .env file
cat .env | lxredact

# Anonymise a meeting transcript (names → [Person A], companies → [Company B])
cat transcript.txt | lxredact --anon

# Use strict mode to also mask IPs, hostnames, paths, and niche service tokens
cat config.yaml | lxredact --strict

# Get a JSON summary of what was redacted
cat .env | lxredact --json

# Use LLM to explain the risk of what was found (requires LX_API_KEY)
cat .env | lxredact --explain

# Read from a file
lxredact --file secrets.txt

# Pipe into another tool (stdout is always just the redacted text)
cat deploy.sh | lxredact | lxexplain
```

## Output

**Plain mode (default):** stdout contains only the redacted text. Diagnostics
(count of redacted items, location hints) go to stderr and can be suppressed
with `--quiet`.

**JSON mode (`--json`):** stdout contains a JSON object:

```json
{
  "redacted_text": "api_key=[REDACTED]",
  "redacted_count": 1,
  "items": [
    { "kind": "secret", "location": "line 1" }
  ]
}
```

With `--explain --json`, an additional `explanation` field is included.

## Security design

`lxredact` is the trust base for the suite. It is designed to be maximally local:

1. **Default mode:** fully deterministic, regex-based — zero network calls.
2. **`--explain` mode:** sends only type+location metadata to the LLM, never
   the actual secret values.
3. **`--no-redact`:** disables masking and prints a prominent WARNING. Use only
   in controlled environments.
