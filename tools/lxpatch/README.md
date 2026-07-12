# lxpatch

Turn a described change into an applyable unified diff.

## Usage

```
lxpatch <description> [options]
```

Pass the file content via stdin (or `--file <PATH>`). The change description is the positional argument.

```sh
# Generate a diff for renaming a variable
cat myfile.py | lxpatch "rename variable count to total"

# Apply the diff
cat myfile.py | lxpatch "rename variable count to total" | patch -p0 myfile.py

# Or with git apply
cat myfile.py | lxpatch "rename variable count to total" | git apply
```

## Options

```
<description>                 Description of the change to make
--file <PATH>                 Read input from file instead of stdin
--json                        Output full JSON object to stdout
--quiet / -q                  Suppress stderr messages
--lang <BCP-47>               Output language (default: auto-detect)
--dry-run                     Show input and system prompt; do not call LLM
--verbose                     Show model and provider on stderr
--max-input-bytes <n>         Truncate input at n bytes (default: 512 KiB)
--allow-dangerous / -D        Allow output even when dangerous patterns detected
--version / -V                Print version and exit
--help / -h                   Print help and exit
```

## Output

**Plain mode (default):** The unified diff is written to stdout. Apply it with `patch -p0` or `git apply`. A one-sentence summary is written to stderr.

**JSON mode (`--json`):**
```json
{
  "diff": "--- a/file\n+++ b/file\n...",
  "summary": "One sentence describing what changed.",
  "dangerous": false
}
```

## Security

`lxpatch` is a `nocmd` tool — it never executes or writes files. The generated diff is scanned for destructive patterns (`rm -rf`, `dd of=/dev/`, fork bombs, `curl|sh`, etc.). If detected, a `DANGER` warning is printed to stderr and the tool exits with code 3. Pass `--allow-dangerous` / `-D` to output anyway (warning still fires).
