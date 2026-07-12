# lxsql

Generate SQL from a natural-language description.

## Usage

```sh
# Create mode (no stdin) — generate SQL from scratch
lxsql "top 10 customers by total order value last quarter"
lxsql --schema "users(id,email,created_at)" "count users registered this week"

# Edit mode (pipe existing SQL) — apply described change only
lxsql "add a LIMIT 100" < query.sql
lxsql "group by region" < report.sql | lxdiff
```

If `DESCRIPTION` is omitted, the tool reads it from stdin.

In edit mode `lxsql` changes **only what the intent describes** and preserves
comments, formatting, and unrelated clauses verbatim.

## Options

| Flag | Description |
|------|-------------|
| `--schema <SCHEMA>` | Table/column schema to include as context |
| `--json` | Output as JSON `{"sql":"…","mutating":bool}` |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent to the LLM, then exit |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show verbose diagnostics |
| `--max-input-bytes <n>` | Limit stdin to n bytes |
| `-V, --version` | Print version in canonical suite format |
| `-h, --help` | Show help |

## Examples

```sh
# Simple SELECT
lxsql "get all active users with their email addresses"

# With schema context
lxsql --schema "$(cat schema.sql)" "count orders per customer"

# Read description from stdin
echo "find the top 10 products by revenue" | lxsql

# JSON output
lxsql --json "delete sessions older than 30 days"
```

## Output

Plain text: the SQL statement on stdout.

JSON: `{"sql":"SELECT …","mutating":false}`

## Security flags: nocmd

The generated SQL is output to stdout only — it is **never executed**.

Mutating statements (`DELETE`, `DROP`, `UPDATE`, `INSERT`, `TRUNCATE`, `ALTER`,
`CREATE TABLE`, …) are always flagged on stderr with a prominent warning:

```
⚠  WARNING: generated SQL contains mutating statement (DELETE) — review carefully before executing
```

Mutating detection runs locally (deterministic pattern matching) and overrides any
`"mutating": false` returned by the model.
