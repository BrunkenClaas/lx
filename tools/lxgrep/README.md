# lxgrep

Semantic grep: find lines in files or stdin matching a **natural-language query**.

Unlike traditional grep (pattern matching), `lxgrep` understands intent.
Ask "error handling code" and it finds `Err(e) => …` blocks even if the word
"error" never appears.

## Security flags

`fsbound` `untrusted`

- **fsbound**: Only reads files within the path(s) you specify. Symlinks that
  escape the root are rejected with exit 5.
- **untrusted**: The system prompt instructs the model to ignore any instructions
  embedded in the searched content.

## Usage

```
lxgrep <query> [PATH…]        # search files/directories
echo "…" | lxgrep <query>     # search stdin
lxgrep <query> --file PATH    # search a single file via flag
```

### Examples

```bash
# Find error-handling code in a Rust project
lxgrep "error handling" src/

# Find database connection logic
lxgrep "database connection" src/ lib/

# Search stdin
git diff HEAD | lxgrep "timeout configuration"

# Machine-readable output
lxgrep --json "authentication" src/
```

## Output (plain mode)

Grep-compatible `file:line: snippet` format — safe to pipe to other tools:

```
src/main.rs:14:     Err(e) => eprintln!("error: {e}"),
src/db.rs:7:    fn connect(host: &str, port: u16) -> Result<Conn, Error> {
```

## Output (--json mode)

```json
{
  "matches": [
    { "file": "src/main.rs", "line": 14, "snippet": "    Err(e) => eprintln!(\"error: {e}\")," }
  ],
  "capped": false
}
```

## How it works

Relevance is always the LLM's decision — local code only controls how much
content fits into that one call:

1. If everything fits within the candidate-block budget, the whole input is
   sent to the LLM. No keyword matching is needed or used.
2. If the input is larger than the budget, query keywords are used to
   *prioritise* which lines are kept (their context blocks go first), and the
   remaining budget is filled with evenly-sampled blocks from the rest of the
   content — so lines relevant to the query but using different wording still
   reach the model. Across multiple files, each file gets a fair share of the
   budget so one large file cannot crowd out the others.
3. Exactly one LLM call is made; it returns which blocks truly match the
   intent of your query. An empty result always means the model found nothing
   relevant — never that local code decided not to ask.
4. When the budget was exceeded, the JSON output's `capped` field is `true`
   and a warning is printed to stderr (suppressed by `--quiet`).

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | Disable ANSI formatting |
| `--quiet / -q` | Suppress stderr diagnostics |
| `--dry-run` | Show what would be sent without calling the LLM |
| `--lang <BCP-47>` | Response language (default: auto-detected) |
| `--verbose` | Show model, provider, lang on stderr |
| `--max-input-bytes <n>` | Limit per-file read size (default: 512 KiB) |
| `--file <PATH>` | Read stdin content from this file |
| `--version / -V` | Print version |
| `--help / -h` | Show help |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success (zero matches is still success) |
| 1 | LLM / network / config error |
| 2 | Bad usage (missing query, unreadable file, etc.) |
| 5 | Security abort (path escapes fsbound root) |
