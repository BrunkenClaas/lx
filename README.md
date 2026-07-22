# LX Coreutils

[![CI](https://github.com/BrunkenClaas/lx/actions/workflows/ci.yml/badge.svg)](https://github.com/BrunkenClaas/lx/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Write a commit message from your diff, explain a scary command, turn plain
English into a shell command — without leaving the terminal or pasting into a
chat window. LX Coreutils is a suite of small, composable commands that each do
one such job and pipe into the next.

![lxcommit, lxexplain, lxsh and lxlog running in a terminal: each command's real output appears within about a second](docs/assets/demo.gif)

**Runs on a local [Ollama](https://ollama.com) model by default — no API key,
nothing leaves your machine.** Or point it at a hosted model (Anthropic, OpenAI,
Gemini, Groq, +6) with one env var: each call is deterministic and tightly
scoped, so a fast, cheap model costs a fraction of a cent. Either way, cold
start < 15 ms.

```sh
# with a local model (default) or a cheap hosted one — then:
git diff --staged | lxcommit               # write the commit message
lxexplain "tar -xzf archive.tar.gz"        # explain any command
lxsh "find all .log files older than 30d"  # natural language → shell
journalctl -u nginx | lxlog                # triage a wall of logs
```

## Finding the right tool

`lx` itself is the catalog — no network needed:

```sh
lx                                         # browse all 72 tools (offline)
lx tools commit                            # find tools related to "commit"
lx tools --cat code                        # list Code & Development tools
```

## From tools you know

| If you reach for… | Try lx instead |
|---|---|
| `grep` for meaning, not syntax | `lxgrep "failed logins" nginx.log` |
| `man` for a quick answer | `lxman git rebase` |
| writing commit messages by hand | `git diff --staged \| lxcommit` |
| googling "curl with bearer token" | `lxcurl "POST to api.example.com/users with auth"` |
| jq trial-and-error | `lxjq "extract all email fields from users array"` |
| sed/awk trial-and-error | `lxsed "print 3rd column where first is ERROR"` |
| reading a dense diff | `git diff main \| lxdiff` |
| squinting at a stack trace | `cat error.log \| lxdebug` |
| decoding a JWT | `lxjwt < token.txt` |
| figuring out why curl failed | `curl -v https://api.example.com 2>&1 \| lxhttp` |
| googling DNS errors | `dig example.com \| lxdns` |

## Hero pipe examples

```sh
# Review, then commit if it looks good
git diff --staged | lxdiff && git diff --staged | lxcommit

# Edit a Dockerfile, review the change, apply it
lxdockerfile "add a healthcheck" < Dockerfile | lxdiff

# Generate a firewall rule accounting for existing rules
iptables -S | lxfirewall "allow SSH only from 10.0.0.0/8"

# Find and summarise all TODO comments
lxgrep "TODO" src/ | lxsum

# Diagnose a slow DNS lookup
dig +stats example.com | lxdns
```

All tools share a consistent interface:
- `--json` for structured output
- `--dry-run` to preview the system prompt and redacted input before sending
- `--lang <code>` to set output language (BCP-47)
- Exit codes 0–5 with human and JSON error formats

## Installation

### Quick install (recommended)

One command — downloads the latest prebuilt binaries for your platform, verifies
the checksum, and installs them to a bin directory. No Rust toolchain, no compiling.

**Linux (x86_64 or aarch64, incl. 64-bit Raspberry Pi OS):**

```sh
curl -fsSL https://raw.githubusercontent.com/BrunkenClaas/lx/main/scripts/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/BrunkenClaas/lx/main/scripts/install.ps1 | iex
```

The installer puts binaries in `~/.local/bin` (Linux) or `%USERPROFILE%\bin`
(Windows) and tells you if that directory needs adding to your PATH. Override the
location with `LX_INSTALL_DIR`, or pin a version with `LX_VERSION=1.0.2`.

> Piping a script from the internet into your shell runs it with your
> permissions. The script is short and does only what is described above — read
> it first if you prefer: [`scripts/install.sh`](scripts/install.sh) /
> [`scripts/install.ps1`](scripts/install.ps1). macOS has no prebuilt binary yet;
> build from source (below).

### Manual install from a release ZIP

Prefer to do it by hand, or on a platform the installer doesn't cover? Download the
suite ZIP for your platform from [GitHub Releases](https://github.com/BrunkenClaas/lx/releases)
and verify the checksum:

```sh
sha256sum -c lx-coreutils-1.0.2-x86_64-unknown-linux-musl.zip.sha256
```

#### Linux — install to PATH

```sh
mkdir -p ~/.local/bin
unzip lx-coreutils-1.0.2-x86_64-unknown-linux-musl.zip
mv lx-coreutils-1.0.2-x86_64-unknown-linux-musl/lx* ~/.local/bin/
```

If `~/.local/bin` is not yet on your PATH, add this to `~/.bashrc` or `~/.zshrc`:

```sh
export PATH="$HOME/.local/bin:$PATH"
```

#### Windows — install to PATH

```powershell
# Unzip and copy binaries to a local bin folder
$dest = "$env:USERPROFILE\bin"
New-Item -ItemType Directory -Force $dest | Out-Null
Expand-Archive lx-coreutils-1.0.2-x86_64-pc-windows-gnu.zip -DestinationPath .
Copy-Item lx-coreutils-1.0.2-x86_64-pc-windows-gnu\*.exe $dest
```

Then add `%USERPROFILE%\bin` to your PATH permanently (run once):

```powershell
[Environment]::SetEnvironmentVariable(
    "PATH", "$env:USERPROFILE\bin;$([Environment]::GetEnvironmentVariable('PATH','User'))",
    "User"
)
```

Restart your terminal for the PATH change to take effect.

Individual tools are also available as standalone binaries on the Releases page
if you only need one or two tools.

### Build from source

```sh
git clone https://github.com/BrunkenClaas/lx
cd lx
cargo build -p lxexplain --release
```

## Configuration

No configuration is required to get started — drop a binary in your PATH and run it.
The default provider is **Ollama** (local, no API key needed).

Run `lx config` for an interactive setup wizard that creates the config file for you.

### Providers

Ollama is the default and needs no key. To use a different provider, set
`LX_PROVIDER` (and `LX_API_KEY` for the cloud ones):

```sh
export LX_PROVIDER=ollama      # local default, no key needed
export LX_PROVIDER=lmstudio    # local, no key needed
export LX_PROVIDER=anthropic   && export LX_API_KEY=sk-ant-...
export LX_PROVIDER=openai      && export LX_API_KEY=sk-...
export LX_PROVIDER=gemini      && export LX_API_KEY=AIza...
export LX_PROVIDER=groq        && export LX_API_KEY=gsk_...
export LX_PROVIDER=openrouter  && export LX_API_KEY=sk-or-...
export LX_PROVIDER=mistral     && export LX_API_KEY=...
export LX_PROVIDER=deepseek    && export LX_API_KEY=...
export LX_PROVIDER=azure       && export LX_API_KEY=...  # requires LX_BASE_URL
```

Override the model or endpoint for any provider:

```sh
export LX_MODEL=claude-opus-4-8        # override default model
export LX_BASE_URL=https://...         # custom endpoint (Azure, Bedrock, Vertex…)
```

**API keys must never be stored in the config file** — use env vars or the OS credential store.

### Provider defaults

Each provider has a built-in default model (fast/cheap tier):

| Provider     | Default model                              |
|--------------|--------------------------------------------|
| `ollama`     | `llama3.1:8b`                              |
| `lmstudio`   | `llama3.1-8b-instruct`                     |
| `anthropic`  | `claude-haiku-4-5`                         |
| `openai`     | `gpt-4o-mini`                              |
| `gemini`     | `gemini-2.5-flash-lite`                    |
| `groq`       | `llama-3.1-8b-instant`                     |
| `openrouter` | `meta-llama/llama-3.1-8b-instruct:free`    |
| `mistral`    | `mistral-small-latest`                     |
| `deepseek`   | `deepseek-chat`                            |
| `azure`      | *(no default — set `LX_MODEL`)*            |

### Local model size guide

| Size   | Suitable for |
|--------|-------------|
| < 3 B  | Not recommended — too many hallucinations and schema errors. |
| 3 B    | Simple command lookups only (`lxsh`, `lxsql`, `lxcurl`, …). Avoid long-output tools. |
| 7–8 B  | **Recommended minimum.** Handles nearly the full suite. `llama3.1:8b` or `qwen2.5:7b`. |
| 14 B   | Near-remote quality; requires ≥16 GB VRAM. |
| remote | Full suite, no constraints. |

Local models need a context window of at least **32 768 tokens** to cover every tool.
With **Ollama** this is automatic — lx requests it (`num_ctx`) on every call. With
**LM Studio** you must set it yourself: choose a **Context Length ≥ 32k** in the GUI when
loading the model (LM Studio ignores the value from the API).

> **Avoid reasoning/thinking models** (e.g. QwQ, Gemma 4 QAT, DeepSeek-R1, o1/o3).
> These emit a chain-of-thought that consumes the token budget before the JSON answer,
> causing tools to fail with truncated output. Use instruct variants instead or deactivate reasoning/thinking in the model settings.

### Config file (optional)

For persistent settings create a config file with the interactive wizard or by hand:

```sh
lx config          # interactive wizard (creates the file for you)
lx config --print  # preview only, no file written
```

Alternatively create it by hand at:
- Linux/macOS: `~/.config/lx/config.toml`
- Windows: `%APPDATA%\lx\config.toml`

```toml
[llm]
provider = "anthropic"
# model = ""        # leave empty to use provider default
# base_url = ""     # leave empty to use provider default
timeout_secs = 30
max_retries = 3

[limits]
max_input_bytes = 524288   # 512 KiB
max_output_tokens = 1024

[redact]
level = "standard"   # or "strict"

[output]
lang = "auto"        # BCP-47 code or "auto" (detect from locale)
color = "auto"       # auto | always | never
```

All values are optional — omitted keys fall back to compiled defaults.
Build-from-source users can find a fully annotated template at
`crates/lx-config/config.example.toml`.

## Shell Integration

Optional shell scripts wire lx tools into your interactive shell as keyboard
shortcuts and helper functions. After sourcing the appropriate script, you get:

| Shortcut / Command | What it does |
|--------------------|--------------|
| `Ctrl+K` | Converts the current command-line buffer into a shell command via `lxsh`. Type plain English, press Ctrl+K, get a real command. If `lxsh` returns nothing the buffer is left unchanged. |
| `Ctrl+E` | Explains the current command-line buffer via `lxexplain`. The command is echoed, the explanation prints below, and a fresh prompt appears. |

### Setup

Replace `/path/to/shell-integration` with the actual path (repo checkout or
the `shell-integration/` folder from the release ZIP). Run once to install permanently:

**bash:**
```sh
echo 'source /path/to/shell-integration/lx.bash' >> ~/.bashrc && source ~/.bashrc
```

**zsh:**
```sh
echo 'source /path/to/shell-integration/lx.zsh' >> ~/.zshrc && source ~/.zshrc
```

**fish:**
```sh
echo 'source /path/to/shell-integration/lx.fish' >> ~/.config/fish/config.fish
source ~/.config/fish/config.fish
```

**PowerShell** (requires PSReadLine, included by default on Windows):
```powershell
Add-Content $PROFILE ". /path/to/shell-integration/lx.ps1"
. $PROFILE
```

**CMD (Command Prompt):** not supported. CMD has no readline API, so
key bindings cannot be implemented. Use PowerShell instead.

## System Requirements

| Platform | Minimum |
|----------|---------|
| Linux    | Kernel 3.17+ |
| Windows  | Windows 10 1903+ |
| macOS    | 11.0+ (build from source) |
| Rust (build) | Exact pinned toolchain, see `rust-toolchain.toml` |

Static binaries (musl on Linux, `+crt-static` on Windows) require no runtime libraries.

## Security

- **Redaction** — tools that process potentially sensitive input (diffs, logs, env vars) run the input through `lx-redact` before the LLM call. Use `--dry-run` to see exactly what gets sent. Redaction is best-effort: it covers a broad set of known prefixed secret formats (AWS, GitHub, GitLab, GCP, Slack, Stripe, SendGrid, Twilio, npm, Anthropic, …) and credential-context names (`API_KEY`, `token`, `client_secret`, …). Each prefixed detector applies a Shannon-entropy floor + placeholder filter (the gitleaks approach), so documentation examples like `AKIAIOSFODNN7EXAMPLE` and placeholders like `sk-your_api_key_here_…` are left alone. It cannot reliably mask a secret in an unrecognised variable with a short value, and the entropy gate does not catch every false positive (a value built from English words has key-like entropy). Treat it as a strong safety net, not a guarantee.
- **`lxsecret` / `lxredact --strict`** — for a thorough scan, `lxsecret` detects committed secrets (add `--strict` for a keyword-independent high-entropy sweep), and `lxredact --strict` masks PII (IPs, hostnames, paths) plus an expanded set of niche service tokens.
- **No command execution** — command-generating tools (`lxsh`, `lxsql`, …) output text only. Nothing is ever executed.
- **No telemetry** — the tools call only the configured LLM endpoint, never anything else.
- **`--dry-run`** — shows the (redacted) text that would be sent to the LLM without sending it.

## Tool List

Run `lx` or `lx tools` to browse all 72 tools offline. Short list by category:

**Text & Analysis:** `lxexplain` `lxsum` `lxtl` `lxclass` `lxpull` `lxproof`

**Code & Dev:** `lxcode` `lxdebug` `lxdoc` `lxregex` `lxregexplain` `lxsql` `lxsh`
`lxtypehint` `lxrename` `lxfixcmd` `lxfixscript` `lxpatch`

**Cmd-gen:** `lxjq` `lxcurl` `lxsed` `lxffmpeg` `lxkubectl` `lxdockercmd` `lxrsync`
`lxmount` `lxkill` `lxcron` `lxfirewall` `lxip` `lxprintf`

**Filesystem & Data:** `lxfind` `lxgrep` `lxdigest` `lxcsv` `lxjson` `lxconv` `lxtable` `lxmock`

**Search & Knowledge:** `lxask` `lxman` `lxerrno`

**Productivity:** `lxdraft` `lxcommit` `lxclog` `lxpr` `lxstandup` `lxtodo` `lxnotes`
`lxgitignore` `lxdockerfile` `lxmakefile`

**Docs & Format:** `lxmd` `lxmermaid` `lxdiff` `lxgraph`

**Security:** `lxsecret` `lxredact` `lxperm` `lxcve` `lxcert` `lxjwt` `lxchmod`

**Network & System:** `lxlog` `lxconf` `lxport`

**Diagnostics:** `lxdns` `lxssl` `lxping` `lxhttp`

**Meta & Shell:** `lxundo`

**Web:** `lxurl`

## How this was built

LX Coreutils was designed and written to a hand-authored specification —
[`docs/design_document.md`](docs/design_document.md) sets out the architecture,
the security model, the naming and I/O conventions, and the exact contract of
every one of the 72 tools. That spec drove the implementation: 72 tools sharing
5 libraries, each conforming to the same rules by design. AI was used as a tool
throughout, directed against that specification and shaped by hand. Every tool
is covered by tests and a self-grading acceptance harness
(`crates/lx-acceptance/`).

## License

Dual-licensed under **MIT OR Apache-2.0**. See `LICENSE-MIT` and `LICENSE-APACHE`.

## Contributing

See `CONTRIBUTING.md`.
