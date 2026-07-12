# lxcode

Generate code from a natural-language description.

## Usage

```sh
lxcode "a function that reverses a string" --code-lang rust
lxcode "read a CSV file and print each row" --code-lang python
lxcode "select all users older than 18"   # auto-detects SQL
echo "fibonacci sequence up to 100" | lxcode --code-lang go
lxcode "HTTP GET request" --code-lang typescript --json
```

## Example Output

```
$ lxcode "add two integers" --code-lang rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--code-lang <lang>` | Target language: rust, python, go, js, ts, bash, sql, … (default: auto-detect) |
| `--json` | Output as JSON `{"code": "...", "language": "..."}` |
| `--plain` | No ANSI formatting |
| `--dry-run` | Show description without sending to LLM |
| `-q, --quiet` | Suppress stderr messages (DANGER warnings are never suppressed) |
| `-D, --allow-dangerous` | Exit 0 even when output is dangerous (warning still printed to stderr) |
| `--lang <BCP-47>` | Language for comments/explanations (default: auto) |
| `--verbose` | Verbose diagnostics on stderr |
| `-V, --version` | Print version information |

## Security

**lxcode never executes the generated code.** It outputs code to stdout only (§8.3 nocmd).

Before output, a deterministic local pattern check scans for dangerous patterns and
prints a warning to **stderr**:

- `rm -rf /`, `shutil.rmtree`, `FileUtils.rm_rf` — recursive deletion
- `curl | sh`, `iwr | iex` — untrusted remote script execution
- `DROP TABLE`, `DROP DATABASE`, `DELETE FROM` (without WHERE) — destructive SQL
- `dd of=/dev/`, `mkfs`, `shred` — low-level disk operations
- Fork bombs

If a dangerous pattern is found, a prominent warning is printed to stderr,
and the tool exits with code 3. The code is still printed to stdout — review it
before running. Pass `--allow-dangerous` to exit 0 instead.

## Output Schema (JSON)

```json
{"code": "fn add(a: i32, b: i32) -> i32 { a + b }", "language": "rust", "dangerous": false}
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (config/auth, network, or LLM error) |
| 2 | No description provided |
| 3 | Dangerous output — use `--allow-dangerous` to get exit 0 |
| 5 | Security abort |
