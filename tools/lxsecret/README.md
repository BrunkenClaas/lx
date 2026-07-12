# lxsecret

Scan for accidentally committed secrets and API keys.

## Security flags

`nonet` · `redact` · `fsbound`

## Usage

```
# Scan stdin
git diff --staged | lxsecret

# Scan a single file
lxsecret config.env

# Scan a directory (stays within it — fsbound)
lxsecret ./src/

# Local detection only — no LLM call, no API key needed
lxsecret --no-llm secrets.env

# Scan harder: keyword-independent high-entropy sweep
git diff --staged | lxsecret --strict

# JSON output for scripting
lxsecret --json config.env
```

## What it detects

| Pattern | Example |
|---------|---------|
| AWS Access Key ID | `AKIA…` (20 chars) |
| GitHub PAT / fine-grained / GitLab | `ghp_…`, `github_pat_…`, `glpat-…` |
| Google API keys | `AIza…` (39 chars) |
| Slack tokens | `xoxb-…`, `xoxp-…` |
| Stripe keys | `sk_live_…`, `rk_live_…` |
| SendGrid / Twilio / npm | `SG.…`, `SK<32 hex>`, `npm_…` |
| OpenAI / generic keys | `sk-…` (≥ 20 chars) |
| PEM private keys | `-----BEGIN … PRIVATE KEY-----` |
| Niche services | Shopify, DigitalOcean, Hugging Face, Linear, Postman, Doppler, Atlassian, Heroku, PyPI, Telegram |
| Values assigned to a credential keyword | `password = …`, `api_key: …`, `secret = …` |
| High-entropy strings *anywhere* (`--strict` only) | a random-looking token with no keyword nearby |

### Entropy gate

Every prefixed detector applies a **Shannon-entropy floor** (2.0–4.0 bits/byte,
matching gitleaks' thresholds) plus a placeholder filter to the value following
the prefix. A string that only *looks* like a key by its prefix — a documentation
example (`AKIAIOSFODNN7EXAMPLE`, `sk-your_api_key_here_…`) or a repetitive
placeholder (`sk_live_abcabcabc…`) — is not reported. Note this filters
placeholders and low-entropy junk; a value built from real English words has
entropy close to a real key and can still be reported.

### Credential-keyword assignments

When a credential keyword appears as an **assignment** — immediately followed by
`=` or `:` — the keyword signals that the assigned value is likely a credential.
The assignment requirement is deliberate: a keyword in prose with no separator
after it (`choosing a good password improves security`) is **not** an assignment
and is left alone. Trivial values (`password=test`) are rejected by the
placeholder filter, and the LLM assessment step sorts the borderline cases.

Keywords come in two tiers:

- **Strong keywords** — `password`, `passwd`, `passphrase`, `secret`, `api_key`
  (and `api-key` / `api key` / `apikey`), `access_key`, `secret_key`,
  `auth_token`, `private_key`, `client_secret`, `token`, `credential`, `pwd`,
  plus a curated handful of unambiguous non-English "password" words
  (`passwort`, `contraseña` / `contrasena`, `senha`, `mot de passe` /
  `motdepasse`). The keyword is unambiguous, so the assigned value is reported
  at a **lenient bar** (length ≥ 8, entropy ≥ 2.5). This catches human-chosen
  passwords like `password: Qw7k@PmRn!TvXs91` that the machine-token thresholds
  would miss.
- **Weak keyword** — bare `key`. It is also an ordinary English word and the
  universal config map-key (`key: production`), so the assigned value must clear
  the **machine-token bar** (length ≥ 20, entropy ≥ 3.5) before it is reported.
  `key: production` and `key: us-east-1` are ignored; `key: <random-looking-key>`
  fires.

## Default vs. `--strict`

| | Default | `--strict` |
|--|---------|------------|
| Prefixed formats (all gated) | ✅ | ✅ |
| High-entropy values **next to** a secret keyword | ✅ | ✅ |
| High-entropy values **anywhere** (no keyword) | — | ✅ (floor 4.0, len ≥ 24) |

`--strict` trades more findings for more noise — use it for a thorough pre-commit
sweep, the default for routine piping.

## Output

Plain mode (stdout): one finding per line — `type\tlocation\tmasked_value [assessment]`

JSON mode (`--json`):
```json
{
  "findings": [
    {
      "type": "aws_access_key",
      "location": "config.env:5",
      "masked": "AKIA****MPLE",
      "assessment": "real"
    }
  ]
}
```

## Security guarantees

- **Masked output**: Secret values are never printed in full. The middle portion is replaced with `****`.
- **No value to LLM**: The LLM receives only the type, location, masked form, and a context hint — never the actual secret value.
- **fsbound**: Directory scans stay within the specified root. Symlinks that escape are rejected with exit code 5.
- **nonet**: No network calls other than the single LLM classification request.

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show what would be scanned, then exit |
| `--quiet / -q` | Suppress stderr diagnostics |
| `--strict` | Also sweep for high-entropy values with no surrounding keyword |
| `--no-llm` | Local detection only — no LLM call |
| `--no-redact` | No-op (lxsecret always masks output) |
| `--lang <BCP-47>` | Output language |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes <n>` | Limit bytes read from stdin / per file |
| `--file <PATH>` | Alias for the positional PATH argument |
| `--version / -V` | Print version string |
| `--help / -h` | Print usage |
