# lxdoc

Add docstrings and comments to any code snippet using an LLM.

`lxdoc` reads a code snippet from stdin, sends it to an LLM, and prints the
same code with idiomatic docstrings inserted — without changing any logic.

## Usage

```sh
# Auto-detect language and docstring style:
cat my_module.py | lxdoc

# Explicit style:
cat my_module.rs | lxdoc --style rustdoc
cat MyClass.java | lxdoc --style javadoc

# JSON output:
cat script.py | lxdoc --json
```

## Options

| Flag | Description |
|---|---|
| `--style` | Docstring format: `auto` (default), `docstring`, `javadoc`, `rustdoc` |
| `--json` | Emit `{"code":"..."}` instead of plain text |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show input without calling the LLM |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--lang` | BCP-47 language tag for docstring language (default: auto-detect) |
| `--verbose` | Show model, provider, and token usage |
| `--max-input-bytes` | Override the maximum stdin size (default: 512 KiB) |
| `--version` / `-V` | Print version and exit |
| `--help` / `-h` | Print help and exit |

## Output schema

```json
{"code": "<documented source code as a string>"}
```

Plain-text output is the documented code directly on stdout, ready to be
written back to a file.

## Security

**SEC: untrusted** — the LLM is instructed to ignore any directives embedded
in the user-supplied code. System and user messages are always kept separate so
that prompt-injection attacks in the source code cannot alter the tool's
behaviour.

## Examples

```sh
# Document a Python file in-place:
lxdoc < src/math.py > src/math_documented.py

# Document Rust code with Rustdoc comments:
lxdoc --style rustdoc < src/lib.rs

# See what would be sent without making a network request:
lxdoc --dry-run < src/utils.js
```
