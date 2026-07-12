# lxffmpeg

Generate an ffmpeg command from a plain-English description.

## Usage

```
lxffmpeg [OPTIONS] [DESCRIPTION]
```

### Arguments

| Argument      | Description                                                   |
|---------------|---------------------------------------------------------------|
| `DESCRIPTION` | Plain-English description of the ffmpeg task (positional arg) |

### Flags

| Flag                      | Description                                              |
|---------------------------|----------------------------------------------------------|
| `--json`                  | Output as JSON `{"command":"...","dangerous":bool}`      |
| `--plain`                 | Disable ANSI colours                                     |
| `--dry-run`               | Show what would be sent to the LLM, then exit            |
| `-q`, `--quiet`           | Suppress diagnostic stderr (DANGER warnings never suppressed) |
| `--lang <BCP-47>`         | Output language (default: auto-detect)                   |
| `--verbose`               | Show verbose diagnostics on stderr                       |
| `--max-input-bytes <n>`   | Max bytes to read from stdin                             |
| `--file <PATH>`           | Read description from file                               |
| `-V`, `--version`         | Print version information                                |
| `-h`, `--help`            | Print help                                               |

## Examples

```bash
# Generate an ffmpeg command from a description
lxffmpeg "convert video.mp4 to audio mp3"
# → ffmpeg -i video.mp4 output.mp3

# Output as JSON
lxffmpeg --json "compress video.avi to h264 mp4"
# → {"command":"ffmpeg -i video.avi -vcodec libx264 -crf 23 output.mp4","dangerous":false}

# Read description from stdin
echo "trim video.mp4 from 10 to 30 seconds" | lxffmpeg

# Read description from file
lxffmpeg --file task.txt
```

## Security

`[SEC: nocmd]` — This tool **never executes** the generated command. It outputs the
command text to stdout only.

Local danger detection runs on every generated command before output. The following
patterns trigger a `DANGER:` warning on stderr:

- Pipes to a shell (`| sh`, `| bash`, `| zsh`, `| iex`, etc.)
- Writes to raw block devices (`/dev/sd*`, `/dev/nvme*`, `/dev/disk*`)
- Writes into system configuration directories (`/etc/`)

DANGER warnings are **never suppressed** by `--quiet`.

## Exit codes

| Code | Meaning                              |
|------|--------------------------------------|
| 0    | Success                              |
| 1    | General error (config, network, LLM) |
| 2    | Bad usage (missing description)      |
| 5    | Security abort                       |
