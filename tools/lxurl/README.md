# lxurl

Fetch a URL and answer questions about its content.

## Usage

```
lxurl https://example.com
lxurl https://example.com --question "what is this page about?"
echo "summarise the main points" | lxurl https://docs.rust-lang.org/book/
lxurl https://example.com --json
```

## Flags

| Flag | Description |
|------|-------------|
| `--question <QUESTION>` | Question to answer about the page |
| `--file <PATH>` | Read question from file instead of stdin |
| `--json` | Output as JSON |
| `--dry-run` | Fetch and show extracted text without calling LLM |
| `--lang <BCP-47>` | Output language |

## Output

Plain: URL, optional title, answer/summary.

JSON: `{"url":"...","title":"..." | null,"answer":"...","truncated":true|false}`

## Security

- Only http/https URLs are accepted.
- Loopback, RFC-1918, and link-local addresses are rejected (SSRF protection).
- Page content is treated as untrusted — the LLM is instructed to ignore
  instructions embedded in the page.
