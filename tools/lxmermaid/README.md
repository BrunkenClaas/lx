# lxmermaid

Generate a [Mermaid](https://mermaid.js.org/) diagram from a plain-language description.

## Usage

```
lxmermaid [OPTIONS] [DESCRIPTION]
```

**Create mode** (no stdin) — generate from scratch:
```bash
lxmermaid "user login flow with password validation"
lxmermaid "CI/CD pipeline with test and deploy stages" > pipeline.mmd
```

**Edit mode** (pipe existing diagram) — apply described change only:
```bash
lxmermaid "add a 2FA step after password validation" < diagram.mmd
lxmermaid "rename Login to Authenticate" < diagram.mmd | lxdiff
```

In edit mode `lxmermaid` changes **only what the intent describes** and preserves
all other nodes, edges, and labels verbatim.

## Output

Plain mode outputs only the Mermaid diagram code to stdout — safe for piping directly into files or other tools:

```bash
lxmermaid "simple state machine" > diagram.mmd
```

JSON mode includes the diagram field:

```bash
lxmermaid --json "login flow"
# {"diagram": "sequenceDiagram\n    ..."}
```

## Supported diagram types

lxmermaid selects the most appropriate Mermaid diagram type for the description:

- `flowchart` / `graph` — general flows and decision trees
- `sequenceDiagram` — message passing between participants
- `classDiagram` — object-oriented class hierarchies
- `erDiagram` — entity-relationship models
- `stateDiagram-v2` — state machines
- `gantt` — project timelines
- `pie` — proportional data

## Options

| Flag | Description |
|------|-------------|
| `--json` | Output as JSON `{"diagram": "..."}` |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show input that would be sent to LLM, then exit |
| `--quiet` / `-q` | Suppress stderr diagnostics (DANGER warnings are never suppressed) |
| `--lang <BCP-47>` | Output language (default: auto-detected) |
| `--verbose` | Show model/provider/lang on stderr |
| `--file <PATH>` | Read description from file instead of stdin |
| `--max-input-bytes <N>` | Maximum bytes to read from stdin (default: 512 KiB) |
| `--version` / `-V` | Print version information |
| `--help` / `-h` | Show help |

## Security

`[SEC: nocmd]` — lxmermaid only outputs text. It never executes any generated diagram code. The output is scanned for embedded dangerous patterns (shell pipes, destructive commands) and warnings are printed to stderr.
