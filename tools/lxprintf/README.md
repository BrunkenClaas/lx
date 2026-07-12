# lxprintf

Build a `printf` or `date`/strftime format string from a plain-English description.

## Usage

```
lxprintf [OPTIONS] [DESCRIPTION]
```

Provide the description as a positional argument or pipe it via stdin.

## Examples

```sh
lxprintf "ISO date and time"
# %Y-%m-%d %H:%M:%S

lxprintf "price with two decimal places and a dollar sign"
# $%.2f

echo "left-padded integer in a 6-wide field" | lxprintf
# %06d

lxprintf --json "compact date like 20240115"
# {"format":"%Y%m%d","explanation":"..."}
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show input and system prompt without calling the LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detected) |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes <n>` | Limit stdin bytes read |
| `--file <PATH>` | Read description from file |
| `-V, --version` | Print version string |
| `-h, --help` | Print help |

## Output

**Plain mode** — the format string only goes to stdout; explanation goes to stderr.

**JSON mode** — `{"format":"...","explanation":"..."}` to stdout.
