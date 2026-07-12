# lxrename

Generate a safe rename script from natural-language intent.

## Usage

```
lxrename [OPTIONS] <INTENT>
```

Pipe a file list to stdin or use `--in <path>` to specify a directory. The tool
produces an executable `mv` script on stdout.

## Examples

```sh
ls *.py | lxrename "rename test files to use snake_case"
lxrename --in ./src "add a v2_ prefix to all json files"
lxrename --in ./photos/vacation "add folder name as prefix"
lxrename --in ./photos --recursive "rename test files to snake_case"
lxrename --json "rename to snake_case" < file_list.txt
lxrename --in ./src "add _YYYYMMDD suffix from creation date"
```

## Output (plain)

```
mv "testFoo.py" "test_foo.py"
mv "testBar.py" "test_bar.py"
```

## Output (--json)

```json
{
  "renames": [
    {"from": "testFoo.py", "to": "test_foo.py"},
    {"from": "testBar.py", "to": "test_bar.py"}
  ],
  "script": "mv \"testFoo.py\" \"test_foo.py\"\nmv \"testBar.py\" \"test_bar.py\"",
  "dangerous": false
}
```

## Security

- **nocmd**: the tool never executes any command. It outputs a script to stdout only.
  Classic destructive shell patterns (e.g. `rm -rf /`) are detected locally and
  trigger a DANGER warning on stderr. Exit code 3 if dangerous unless `--allow-dangerous`/`-D` is set.
- **fsbound**: all renames are constrained to the root (`--in <path>` or cwd). Any
  rename pair that would escape the root is skipped with a warning on stderr (exit 0).
  If a rename target already exists on disk, a warning is printed but the tool exits 0.

## File metadata

When `--in <PATH>` is used, the tool automatically annotates each file with its
`created`, `modified`, and `size` from the filesystem. This lets the model handle
intents like "add creation date suffix" or "add folder name as prefix" correctly.

When reading from stdin or `--file`, plain filenames are expected — the caller
controls what metadata (if any) is included.

## Flags

| Flag | Description |
|------|-------------|
| `--in <PATH>` | Directory to list files from; annotates each file with created/modified/size metadata |
| `--recursive` / `-r` | Walk subdirectories recursively (requires `--in`); files listed as relative paths |
| `--allow-dangerous` / `-D` | Accept dangerous output and exit 0 instead of 3 |
| `--json` | Output as JSON |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent to the LLM, then exit |
| `--quiet` / `-q` | Suppress diagnostic messages (safety warnings are never suppressed) |
| `--lang <BCP-47>` | Output language |
| `--verbose` | Show verbose diagnostics |
| `--max-input-bytes <n>` | Maximum bytes to read from stdin |
| `--file <PATH>` | Read file list from file instead of stdin |
| `--version` / `-V` | Print version information |
| `--help` / `-h` | Print help |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (network, LLM, config) |
| 2 | Bad usage (no intent, invalid flags) |
| 3 | Dangerous pattern detected in generated script (use `-D` to override) |
