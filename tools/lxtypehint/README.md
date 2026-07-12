# lxtypehint

Add type hints and annotations to source code. Infers the programming language
from the input and returns the same code with type information added.

## Usage

```sh
# From stdin
cat myscript.py | lxtypehint

# From file
lxtypehint --file myscript.py

# Output as JSON
lxtypehint --file myscript.py --json
```

## Example

Input:
```python
def add(x, y):
    return x + y
```

Output:
```python
def add(x: int, y: int) -> int:
    return x + y
```

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON `{"code":"..."}` to stdout |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show input without calling the LLM |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Response language (default: auto-detect) |
| `--verbose` | Show model/provider/lang on stderr |
| `--max-input-bytes <n>` | Limit stdin bytes read (default: 512 KiB) |
| `--file <PATH>` | Read input from file instead of stdin |
| `--version` / `-V` | Print version string |
| `--help` / `-h` | Print help |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | LLM / config / network error |
| 2 | Bad usage (no input, unknown flag) |

## Security flags

- **nocmd**: The tool outputs text only. The annotated code is never executed.
  Dangerous patterns in the output are flagged to stderr.
- **untrusted**: The system prompt instructs the model to ignore any instructions
  embedded in the user-provided code.
