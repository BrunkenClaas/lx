# lxproof

Correct grammar and spelling errors in text using an LLM.

## Usage

```
echo "I recieved you're letter yesturday." | lxproof
```

The corrected text is printed to stdout. A list of changes is printed to stderr.

## Options

| Flag | Description |
|------|-------------|
| `--json` | Output full JSON object with `text` and `changes` fields |
| `--quiet` / `-q` | Suppress changes list on stderr |
| `--plain` | Disable ANSI formatting |
| `--dry-run` | Show what would be sent to the LLM without sending |
| `--lang <BCP-47>` | Output language (e.g. `en`, `de`, `fr`) |
| `--verbose` | Show model/provider/token info on stderr |
| `--max-input-bytes <n>` | Limit input size (default 512 KiB) |
| `--file <PATH>` | Read input from file instead of stdin |
| `--version` / `-V` | Print version |
| `--help` / `-h` | Show help |

## Output

**Plain mode** (default):
- stdout: the corrected text
- stderr: list of changes made (`original -> corrected: reason`)

**JSON mode** (`--json`):
```json
{
  "text": "I received your letter yesterday.",
  "changes": [
    {
      "original": "recieved",
      "corrected": "received",
      "reason": "Spelling: ie/ei rule"
    }
  ]
}
```

## Security

- `untrusted`: The model is instructed to ignore any instructions embedded in
  the user-provided text. Input data is treated as untrusted content to proofread,
  not as commands.

## Examples

```sh
# Proofread from a file
lxproof --file draft.txt

# Get JSON output for scripting
cat essay.txt | lxproof --json | jq '.changes | length'

# Suppress the changes list and just get the corrected text
echo "She dont know nothing." | lxproof --quiet
```
