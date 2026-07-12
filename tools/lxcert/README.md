# lxcert

Explain a TLS/X.509 certificate in plain language.

## Usage

```sh
# Pipe a PEM certificate into lxcert
cat server.crt | lxcert

# Read the certificate from a file
lxcert --file server.crt

# Fetch and examine a live certificate, then explain it
openssl s_client -connect example.com:443 -showcerts </dev/null 2>/dev/null | lxcert

# Get structured JSON output
lxcert --file server.crt --json
```

## Example Output

```
$ lxcert --file server.crt
Subject:    CN=example.com, O=Example Corp, C=US
Issuer:     CN=R3, O=Let's Encrypt, C=US
Valid until: 2025-09-15

Notes:
  - Domain-validated certificate for example.com
  - Issued by Let's Encrypt, a free and automated CA
  - Valid for 90 days as is standard for Let's Encrypt certificates
  - Subject Alternative Names include www.example.com
```

```json
$ lxcert --file server.crt --json
{
  "subject": "CN=example.com, O=Example Corp, C=US",
  "issuer": "CN=R3, O=Let's Encrypt, C=US",
  "valid_until": "2025-09-15",
  "notes": [
    "Domain-validated certificate for example.com",
    "Issued by Let's Encrypt, a free and automated CA",
    "Valid for 90 days as is standard for Let's Encrypt certificates",
    "Subject Alternative Names include www.example.com"
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
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

- All certificate data is treated as untrusted input. The system prompt instructs the model to ignore any instructions that might be embedded in certificate fields.
- The tool does not make network requests to verify certificates, fetch CRL/OCSP status, or contact any external service beyond the configured LLM provider.
- The complete Distinguished Name (DN) — including all RDN components — is extracted and reported exactly as encoded in the certificate, without interpretation.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
