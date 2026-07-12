# lxgitignore

Generate a `.gitignore` file for a project based on its language and framework stack.

## Usage

```sh
# Create mode (no stdin) — generate from scratch
lxgitignore "rust cli project with vscode"
lxgitignore                         # auto-detects stack from current directory

# Edit mode (pipe existing file) — apply described change only
lxgitignore "add macOS DS_Store rules" < .gitignore
lxgitignore "ignore /dist and /coverage" < .gitignore > .gitignore.new

# Read a project file listing from stdin (create mode)
find . -maxdepth 3 | lxgitignore

# Get structured JSON output with sections
lxgitignore "python django postgres" --json
```

In edit mode `lxgitignore` adds or modifies only what the intent describes
and preserves existing rules verbatim.

## Example Output

```
$ lxgitignore "rust cli project with vscode"
# Rust
/target/
Cargo.lock

# IDE — VS Code
.vscode/
*.code-workspace

# Debug symbols
*.pdb
```

```json
$ lxgitignore "rust" --json
{
  "gitignore": "# Rust\n/target/\nCargo.lock\n\n# Debug\n*.pdb\n",
  "sections": [
    {
      "title": "Rust",
      "patterns": ["/target/", "Cargo.lock"]
    },
    {
      "title": "Debug",
      "patterns": ["*.pdb"]
    }
  ]
}
```

## Flags

| Flag | Description |
|------|-------------|
| `--path <PATH>` | Directory to scan for project structure (default: current directory) |
| `--json` | Output as JSON |
| `--plain` | No ANSI colours |
| `--dry-run` | Show input without sending to LLM |
| `-q, --quiet` | Suppress stderr diagnostics |
| `--lang <code>` | Output language (BCP-47, e.g. `de`, `fr`) |
| `--verbose` | Show token usage |
| `--max-input-bytes <n>` | Override stdin size limit |
| `--file <PATH>` | Read project structure listing from file instead of scanning a directory |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (logical failure, config/auth, network, or LLM error) |
| 2 | Bad usage (missing/invalid args) |

## Security

- Never reads file contents — only file names and directory structure are collected.
- Symlinks are resolved; any that escape the scanned root are skipped.
- No data is sent to any endpoint other than the configured LLM provider.

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
