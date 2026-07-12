# lxfixscript

Fix a broken shell script. Reads the script from stdin (or `--file`) and an optional error message as a positional argument. Outputs the corrected script to stdout.

## Usage

```sh
# Pipe a broken script; include the error message
cat broken.sh | lxfixscript "line 5: syntax error near unexpected token fi"

# From file
lxfixscript --file broken.sh "unexpected fi"

# Without error message
cat broken.sh | lxfixscript

# JSON output (includes changes list)
cat broken.sh | lxfixscript --json
```

## Output

Plain mode (stdout): the corrected script only — safe to redirect or pipe.
Changes are printed to stderr and suppressed with `--quiet`.

JSON mode (`--json`):
```json
{
  "script": "#!/bin/bash\n...",
  "changes": ["Removed extra fi on line 5"],
  "dangerous": false
}
```

The result field is `script`.

## Security

- `nocmd`: the tool never executes any script. The output is text only.
- `untrusted`: instructions embedded in the script are ignored.
- Dangerous patterns (e.g. `rm -rf /`, fork bombs, `curl|sh`) trigger a warning on stderr and exit code 3. Pass `-D` / `--allow-dangerous` to suppress the non-zero exit (warning is still printed).

## OS target

By default `lxfixscript` targets the **host OS**. Use `--target` to target a
different platform:

```sh
# Linux/macOS bash/sh script (default on Linux)
cat broken.sh | lxfixscript "unexpected fi"

# Windows PowerShell script
cat broken.ps1 | lxfixscript --target windows "Write-Hos typo"

# macOS bash/zsh script
cat setup.sh | lxfixscript --target macos
```

## Flags

| Flag | Description |
|------|-------------|
| `[error_msg]` | Optional error message to guide the fix |
| `--target <linux\|windows\|macos>` | Script dialect to fix for (default: host OS) |
| `--file PATH` | Read script from file instead of stdin |
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show input and system prompt without calling the LLM |
| `-q`, `--quiet` | Suppress stderr diagnostics |
| `--lang BCP-47` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider info on stderr |
| `--max-input-bytes N` | Truncate input at N bytes |
| `-D`, `--allow-dangerous` | Exit 0 even when dangerous patterns detected |
| `-V`, `--version` | Print version |
