# lxcron

Generate or explain a crontab line.

## Usage

```
lxcron [OPTIONS] [DESCRIPTION]
```

**Create mode** (positional arg — no stdin):
```sh
lxcron "every weekday at 9am run /usr/local/bin/backup.sh"
# stdout: 0 9 * * 1-5 /usr/local/bin/backup.sh
# stderr: # Runs backup.sh at 09:00 on Monday through Friday
```

**Edit mode** (pipe existing crontab — apply change only):
```sh
crontab -l | lxcron "change the backup job to run at 10pm"
```

**Explain mode** (pipe a crontab line — no positional arg):
```sh
echo "0 2 * * 0 /home/user/cleanup.sh" | lxcron
# stdout: Runs cleanup.sh at 2:00 AM every Sunday. ...
```

**JSON output:**
```sh
lxcron --json "every 15 minutes"
# {"crontab":"*/15 * * * * echo heartbeat","explanation":"...","dangerous":false}
```

## Options

| Flag | Description |
|------|-------------|
| `[DESCRIPTION]` | Plain-English description of the schedule (generate mode) |
| `--json` | Output full JSON object to stdout |
| `--plain` | Disable ANSI colours |
| `--dry-run` | Show input and system prompt without calling the LLM |
| `-q`, `--quiet` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show token usage on stderr |
| `--max-input-bytes <n>` | Truncate stdin at N bytes |
| `--file <PATH>` | Read input from file |
| `-D`, `--allow-dangerous` | Accept dangerous output (exit 0 instead of 3) |
| `-V`, `--version` | Print version |
| `-h`, `--help` | Print help |

## Security

`lxcron` is a `nocmd` tool — it outputs text only and never executes anything.
The command part of generated crontab lines is scanned for dangerous patterns
(recursive deletion, fork bombs, pipe-to-shell, etc.). If a dangerous pattern
is found, a warning is printed to stderr and the tool exits with code 3.
Use `--allow-dangerous` / `-D` to accept the output and exit 0 (warning still
printed).

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | LLM / network / config error |
| 2 | Bad usage (missing input, unknown flag) |
| 3 | Dangerous pattern detected in output |
