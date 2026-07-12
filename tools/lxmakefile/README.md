# lxmakefile

Generate a Makefile or justfile from a plain-English task description.

## Usage

```sh
# Create mode (no stdin) — generate from scratch
lxmakefile "build, test, and clean a Rust project"
lxmakefile --format just "install deps, lint, and build a Node.js app"

# Edit mode (pipe existing file) — apply described change only
lxmakefile "add a lint target using cargo clippy" < Makefile
lxmakefile "add a docker-push target" < Makefile > Makefile.new

# Output as JSON
lxmakefile --json "build and deploy a Docker image"
```

In edit mode `lxmakefile` changes **only what the intent describes** and preserves
existing targets, comments, and variable definitions verbatim.

## Flags

| Flag | Description |
|------|-------------|
| `--format <make\|just>` | Output format hint: `make` (default) or `just` |
| `--json` | Output full JSON `{"content":"...","dangerous":bool}` to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show the input that would be sent to the LLM, then exit |
| `-q, --quiet` | Suppress diagnostic stderr (DANGER warnings are never suppressed) |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show verbose diagnostics on stderr |
| `--max-input-bytes <n>` | Limit stdin read size (default: 512 KiB) |
| `--file <PATH>` | Read input from file instead of stdin |
| `-V, --version` | Print version information |
| `-h, --help` | Print help |

## Output

**Plain mode:** The Makefile or justfile content is printed to stdout. If dangerous
patterns are detected (e.g. `rm -rf /`, piping to shell), a DANGER warning is
printed to stderr. The content is never executed.

**JSON mode:** `{"content":"<makefile text>","dangerous":<bool>}` is printed to stdout.

## Security

This tool has the `nocmd` security flag. It detects dangerous patterns in the
generated content (such as `rm -rf /`, `| sh`, `dd if=`, `mkfs`) and prints a
prominent DANGER warning to stderr. The generated content is never executed.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | LLM or configuration error |
| 2 | Bad usage (missing input, unknown flags) |
| 5 | Security abort |
