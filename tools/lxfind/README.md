# lxfind

Semantic file search: find files by description using an LLM.

## Usage

```
lxfind <description> [path]
lxfind --json <description> [path]
```

- `<description>` — natural-language description of what you are looking for
- `[path]` — directory to search within (default: current directory)

## Examples

```sh
# Find the backup script in the current directory tree
lxfind "the script that runs database backups"

# Search within a specific directory
lxfind "nginx configuration file" /etc/nginx

# Pipe results to xargs
lxfind "Python test files for authentication" src/ | xargs grep -l "login"

# Get JSON output
lxfind --json "main entry point" . | jq '.paths[]'
```

## How it works

1. Walks the directory tree locally (never follows symlinks outside the root).
2. Collects file metadata: name, size, first line / snippet.
3. Skips binaries, large files (>1 MiB), and vendor directories (`.git`,
   `node_modules`, `target`, etc.).
4. Sends a compact catalogue to the LLM — **never sends full file contents**.
5. The LLM returns the paths most relevant to the description, ranked by relevance.
6. Results are capped at **60 paths** (most relevant first). If the cap is reached,
   a note is printed to stderr; plain stdout remains pipe-safe.
7. Validates that all returned paths remain within the allowed root.

## Output

**Plain mode** (default): one path per line — pipe-safe.

```
src/backup.sh
scripts/db_dump.sh
```

**JSON mode** (`--json`):

```json
{
  "paths": [
    "src/backup.sh",
    "scripts/db_dump.sh"
  ],
  "truncated": false
}
```

`truncated` is `true` when the result set was capped at 60 paths.

## Flags

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent without calling the LLM |
| `--quiet` / `-q` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model, provider, and language on stderr |
| `--max-input-bytes <n>` | Maximum bytes read from stdin |
| `--file <PATH>` | Read description from file instead of positional arg |
| `--no-net` | Included for flag consistency (lxfind always requires LLM access) |
| `--version` / `-V` | Print version string |
| `--help` / `-h` | Print help |

## Security flags

- **`fsbound`**: stays within the start directory; resolves symlinks and rejects
  any that escape the root; never accesses `/etc`, `~/.ssh`, or system paths
  without explicit user opt-in.
- **`untrusted`**: the file description comes from the user and is kept strictly
  in the LLM user message — never mixed into the static system prompt. The system
  prompt instructs the model to ignore any instructions embedded in the catalogue
  data.
