# lxsed

Generate a `sed` or `awk` text-transformation one-liner from a plain-English description.

## Usage

```
lxsed [OPTIONS] [DESCRIPTION]
```

If `DESCRIPTION` is omitted, it is read from stdin.

## Examples

```sh
lxsed "replace all occurrences of foo with bar"
# → sed 's/foo/bar/g'

lxsed "delete all blank lines"
# → sed '/^$/d'

lxsed "print the second column of each line"
# → awk '{print $2}'

lxsed --json "sum the values in the third column"
# → {"command":"awk '{sum+=$3} END {print sum}'","tool":"awk","dangerous":false}
```

## Flags

| Flag | Description |
|---|---|
| `--json` | Output full JSON to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show input that would be sent to LLM, then exit |
| `-q`, `--quiet` | Suppress stderr diagnostics (DANGER warnings are never suppressed) |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes <n>` | Limit stdin input size |
| `--file <PATH>` | Read input from file |
| `-V`, `--version` | Print version information |
| `-h`, `--help` | Print help |

## Output

**Plain mode** (default): prints only the command to stdout. Any explanations or warnings go to stderr.

**JSON mode** (`--json`): prints the full object to stdout:
```json
{
  "command": "sed 's/foo/bar/g'",
  "tool": "sed",
  "dangerous": false
}
```

## Security

`lxsed` has the `nocmd` security flag: it **never executes** the generated command. It performs local pattern detection for dangerous constructs (pipe-to-shell, destructive operations) and prints a `DANGER` warning to stderr if any are found.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | General error (config, network, LLM) |
| 2 | Bad usage (missing description, unknown flag) |
| 5 | Security abort |
