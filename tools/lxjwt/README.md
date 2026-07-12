# lxjwt

Decode and explain a JWT token's claims.

`lxjwt` decodes the header and payload sections of a JWT locally in Rust (no
network call for decoding). The decoded JSON is then sent to the LLM to
generate a plain-language explanation of the claims. The raw JWT and the
signature are never sent to the LLM.

## Usage

```sh
# Pass JWT as a positional argument
lxjwt eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...

# Pipe from another command
echo "$JWT" | lxjwt

# JSON output (all fields)
lxjwt --json "$JWT"

# Show decoded claims without calling LLM
lxjwt --dry-run "$JWT"
```

## Output

Plain mode (stdout):
```
Header:  Signed with HMAC-SHA256 algorithm, standard JWT type.
Payload: Issued by example-service for subject user-42 with a viewer role; valid for one hour.
Notes:
  • Token has a 1-hour lifetime (iat to exp)
  • role claim grants viewer-level access
  • No audience (aud) claim is set
```

JSON mode (`--json`):
```json
{
  "header": "Signed with HMAC-SHA256 algorithm, standard JWT type.",
  "payload": "Issued by example-service for subject user-42 with a viewer role; valid for one hour.",
  "notes": [
    "Token has a 1-hour lifetime (iat to exp)",
    "role claim grants viewer-level access",
    "No audience (aud) claim is set"
  ]
}
```

## Security flags

`nonet redact`

- **nonet**: All JWT decoding happens locally in Rust. No raw JWT or signature is
  ever sent to the LLM.
- **redact**: The decoded payload is run through `lx-redact` before being sent to
  the LLM. Use `--no-redact` only if you have audited the payload and accept the risk.

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show decoded claims without calling the LLM |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--no-redact` | Skip redaction of decoded payload (not recommended) |
| `--lang <BCP-47>` | Output language (default: auto-detected) |
| `--verbose` | Show model/provider/token usage on stderr |
| `--max-input-bytes <n>` | Maximum input size (default: 512 KiB) |
| `--file <PATH>` | Read JWT from file instead of stdin |
| `--version` / `-V` | Print version information |
