# lxmount

Generate a `mount` command and matching fstab entry from a plain-English description.
Supports create mode (no stdin) and edit mode (existing fstab/lsblk piped in).

Never executes the generated command. Security flags: `nocmd`, `untrusted`.

## Usage

```
lxmount "<description>" [OPTIONS]
```

Pass an optional `/etc/fstab` or `lsblk`/`mount` snapshot on stdin for context-aware output.

## Examples

```bash
# Basic USB mount
lxmount "mount my NTFS USB drive read-write at /media/usb"

# NFS share
lxmount "mount NFS share from 192.168.1.10:/exports/data at /mnt/data"

# With current fstab context
cat /etc/fstab | lxmount "mount tmpfs at /tmp with size 512m"

# JSON output (includes fstab_line, notes, dangerous)
lxmount --json "mount an ext4 data partition at /mnt/data"
```

## Output

- **Plain mode**: the `mount` command is written to stdout. The fstab line and notes go to stderr.
- **`--json` mode**: full object `{"command":"...","fstab_line":"...","notes":"...","dangerous":false}` to stdout.

## OS target

By default `lxmount` generates commands for the **host OS**. Use `--target` to
target a different platform:

```sh
# Linux (mount + /etc/fstab)
lxmount "mount NTFS USB at /media/usb"                         # default on Linux

# Windows (New-PSDrive / mountvol — no fstab line)
lxmount --target windows "map \\server\share to drive Z:"

# macOS (diskutil / mount + /etc/fstab)
lxmount --target macos "mount exFAT at /Volumes/Data"
```

`fstab_line` is always `null` on Windows target (Windows has no fstab).

`lxmount` warns on stderr when piped state looks like it comes from a different OS
than the target.

## Flags

| Flag | Description |
|------|-------------|
| `--target <linux\|windows\|macos>` | OS to generate commands for (default: host OS) |
| `--json` | Output as JSON |
| `--plain` | Disable ANSI colour |
| `--dry-run` | Show prompt without calling the LLM |
| `-q`, `--quiet` | Suppress stderr diagnostics |
| `--lang <BCP-47>` | Output language (default: auto-detect) |
| `--verbose` | Show model/provider on stderr |
| `--file <PATH>` | Read context from file instead of stdin |
| `-D`, `--allow-dangerous` | Accept dangerous output; exit 0 (warning still printed) |
| `-V`, `--version` | Print version |

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | LLM or config error |
| 2 | Bad usage |
| 3 | Dangerous command detected (use `-D` to override) |
