# lx shell integration

Optional scripts that wire lx tools into your interactive shell as keyboard
shortcuts and helper functions. Sourcing one of these files adds two
features to your shell — nothing more, nothing less. The scripts never modify
your rc files or PATH automatically.

## What you get

### `Ctrl+K` — plain English → shell command

Type a description of what you want to do, press `Ctrl+K`, and the current
command-line buffer is replaced with the shell command that `lxsh` generates.
If `lxsh` returns nothing (e.g. no LLM available), the buffer is left
unchanged so you never lose input.

```
$ list all jpg files modified in the last week[Ctrl+K]
$ find . -name "*.jpg" -mtime -7
```

### `Ctrl+E` — explain current buffer before running

Press `Ctrl+E` to explain the current command-line buffer via `lxexplain`
before executing it. The command is echoed on its own line, the explanation
prints below it, and a fresh prompt appears.

```
$ tar -xzf archive.tar.gz[Ctrl+E]
tar -xzf archive.tar.gz
Extracts (-x) a gzip-compressed (-z) archive (-f) named archive.tar.gz
into the current directory.
$
```

## Installation

Replace `/path/to/shell-integration` with the actual path to this directory
(repo checkout or the `shell-integration/` folder from the release ZIP).
Run the setup command once — it appends the source line to your rc file
permanently so the bindings load on every new terminal.

**bash:**
```sh
echo 'source /path/to/shell-integration/lx.bash' >> ~/.bashrc
source ~/.bashrc
```

**zsh:**
```sh
echo 'source /path/to/shell-integration/lx.zsh' >> ~/.zshrc
source ~/.zshrc
```

**fish:**
```sh
echo 'source /path/to/shell-integration/lx.fish' >> ~/.config/fish/config.fish
source ~/.config/fish/config.fish
```

**PowerShell** (requires PSReadLine, included by default on Windows 10+):
```powershell
Add-Content $PROFILE ". /path/to/shell-integration/lx.ps1"
. $PROFILE
```

If `$PROFILE` does not exist yet, create it first:
```powershell
New-Item -ItemType File -Force $PROFILE
```

## CMD (Command Prompt) — not supported

CMD has no readline API. There is no way to intercept keystrokes while the
user is editing the command line, so `Ctrl+K` and `Ctrl+E` cannot be
implemented.

**Use PowerShell instead.** PowerShell is included with Windows 10+ and
provides the full shell integration experience via `lx.ps1`. If you must use
CMD, you can still call all lx tools directly on the command line — the shell
integration scripts are optional conveniences, not required for tool usage.

## Requirements

- The `lx*` binaries must be in your `PATH`.
- PowerShell: PSReadLine 2.0+ (ships with PowerShell 5.1+ on Windows).
- The `Ctrl+K` binding conflicts with the standard "kill line" readline
  shortcut. If you rely on that, rebind `_lx_suggest` to another key in your
  rc file after sourcing this script.
