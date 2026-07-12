# lx

Suite umbrella command — browse and discover all 72 LX Coreutils tools.

The catalog surface (`lx` / `lx tools`) is **fully offline** — no LLM,
no API key, no network. The `lx model` diagnostic sub-command optionally
contacts the LLM to verify reachability; use `--no-verify` to keep it offline.
`lx config` is an interactive wizard that creates or updates the user config
file without requiring manual TOML editing.

## Usage

```sh
# Grouped overview of all 72 tools
lx

# Same, explicit subcommand
lx tools

# Keyword search (substring match over name + purpose)
lx tools commit

# Filter by category
lx tools --cat code
lx tools --cat security
lx tools --cat "command generation"

# Machine-readable JSON
lx tools --json
lx tools --json | jq '.[].name'

# Plain output (no ANSI color)
lx tools --plain

# Report the effective model the suite will use (resolved from config, not LX_MODEL)
lx model                  # resolves + verifies with a live LLM call
lx model --no-verify      # resolves offline (no API key needed)
lx model --json           # {"model","provider","reachable","error"}
MODEL=$(lx model --no-verify)   # capture for scripting

# Create or update the user config file interactively
lx config                 # interactive wizard (prompts for provider, model, etc.)
lx config --yes           # non-interactive: accept all defaults, write immediately
lx config --print         # preview the resulting TOML; do not write a file
lx config --force         # skip overwrite confirmation

# CI / scripting: preview config without writing, pipe to a file
lx config --yes --print > ~/.config/lx/config.toml
```

## Example Output

```
$ lx

Text & Analysis
  lxexplain     explain anything          lxsum         summarise text
  lxtl          translate text            lxrewrite     rewrite style
  ...

Code & Development
  lxcode        generate code             lxdebug       debug error
  lxrefactor    refactor code             lxdoc         write docstrings
  ...

$ lx tools commit
  lxcommit      Generate a Conventional Commit message from a git diff
  lxclog        Generate a changelog from git log
  lxpr          Generate a PR description from a diff
```

## Available Categories

| Short ID | Category |
|----------|----------|
| `text` | Text & Analysis |
| `code` | Code & Development |
| `cmd` | Command Generation |
| `fs` | Filesystem & Data |
| `know` | Search & Knowledge |
| `prod` | Productivity & Comms |
| `docs` | Docs & Format |
| `sec` | Security |
| `net` | Network & System |
| `auto` | Automation |
| `meta` | Meta & Shell |
| `web` | Multimodal & Web |

## Flags — `lx tools`

| Flag | Description |
|------|-------------|
| `--cat <name>` | Show only one category (short id or name substring) |
| `--json` | Output as JSON array `[{name,category,short,purpose}]` |
| `--plain` | Disable ANSI color and formatting |
| `-h, --help` | Show help |
| `-V, --version` | Print version information |

## Flags — `lx model`

| Flag | Description |
|------|-------------|
| `--no-verify` | Resolve from config only; skip the live LLM call (no API key needed) |
| `--json` | Output `{"model","provider","reachable","error"}` to stdout |
| `--verbose` | Print config/connection diagnostics to stderr |
| `-h, --help` | Show help |

## Flags — `lx config`

| Flag | Description |
|------|-------------|
| `-y, --yes` | Non-interactive: accept all defaults and write without prompting |
| `--print` | Print the resulting TOML to stdout; do not write a file |
| `--force` | Skip the overwrite confirmation when the config file already exists |
| `-h, --help` | Show help |

### What `lx config` writes

- **Provider**, model, base URL, timeout, retries, limits, redact level, output
  language and color — all written to the user config file.
- **API key** — never written to disk. The wizard prints provider-specific
  instructions for `LX_API_KEY` (or the OS credential store) instead.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Config error, or live verification failed (`lx model` only) |
| 2 | Bad usage (invalid flags or subcommand) |

## Notes

- `lx` without a subcommand is equivalent to `lx tools` — it shows
  the full grouped overview.
- The tool catalog is embedded in the binary and requires no config files or
  network access.
- `lx tools | less` works naturally: piped output is always plain (no ANSI).
- `lx model` reads the *same config* the productive tools use, so the
  reported model is always the one that will actually run — not just `LX_MODEL`.
  The acceptance harness uses `lx model --no-verify --json` to label reports.
- `lx config` never stores the API key in the config file — use `LX_API_KEY`
  env var or the OS credential store (cmdkey on Windows, keyctl on Linux).

## Requirements

- Linux: Kernel 3.17+
- Windows: Windows 10 1903+
- No runtime dependencies (statically linked)
- No API key required for `lx tools` and `lx model --no-verify`
