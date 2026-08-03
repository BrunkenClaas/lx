# Changelog

All notable changes to LX Coreutils are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning: each tool has independent versions; the suite release label is `YYYY-MM`.

## [Unreleased]

### Fixed

- **`lxgrep` now states plainly that capped results are incomplete.** When the
  input exceeded the search budget, the warning read "a sampled subset was
  analysed", which is easy to read as a performance note rather than a
  correctness one — results that were missing matches looked authoritative. The
  warning now leads with "results are INCOMPLETE", says matching lines may be
  missing, and suggests narrowing the input. Behaviour is unchanged; only the
  wording is clearer.

## [1.0.5] - 2026-08-01

### Fixed

- **OS credential store now actually works for API keys.** The README promised
  keys could live in the OS credential store, but the client path never consulted
  it — the keyring reader was orphaned, so a key stored with `keyctl` (Linux) or
  `cmdkey` (Windows) was never found and lx reported "no API key". `resolve_api_key`
  now falls back to the credential store after `LX_API_KEY`. Windows Credential
  Manager reads are implemented via `CredReadW` (the `unsafe` FFI lives in
  `lx-core::platform`; `lx-config` stays `#![forbid(unsafe_code)]`). The stored-key
  hints also corrected `cmdkey /add:` → `cmdkey /generic:` (a generic credential is
  what `CredReadW` looks up), and the README now documents how to store the key.

## [1.0.4] - 2026-07-27

### Fixed

- **`reasoning = false` now actually disables reasoning on OpenRouter** (was only
  hiding it). 1.0.3 sent `reasoning: {exclude: true}`, which hides the chain-of-thought
  in the response but lets the model keep reasoning and consuming the output-token
  budget — so a long reasoning pass could still truncate the JSON answer, the exact
  problem the toggle exists to prevent. It now sends `reasoning: {effort: "none"}`,
  which disables reasoning entirely. Gemini, DeepSeek, and Ollama were already correct.

## [1.0.3] - 2026-07-27

### Added

- **Reasoning toggle (`llm.reasoning`, `LX_REASONING`), default off.** For providers
  that only offer reasoning models — or when reasoning wastes the output budget — lx
  now asks the provider to disable reasoning. It's best-effort: the disable request is
  sent only where verified safe (OpenRouter, Gemini, DeepSeek, and Ollama natively);
  providers that reject it (Anthropic, Groq) are sent nothing, so a working request is
  never broken. Set `reasoning = true` to allow reasoning. A genuine non-reasoning
  model is still preferable when available.
- **One-line install scripts.** `scripts/install.sh` (Linux, x86_64 + aarch64
  incl. 64-bit Raspberry Pi OS) and `scripts/install.ps1` (Windows) download the
  latest prebuilt release for the host platform, verify its SHA-256 checksum, and
  install the binaries to a bin directory — no Rust toolchain, no compilation.
  Location is overridable via `LX_INSTALL_DIR` and the version via `LX_VERSION`.

### Fixed

- **`lxrename` no longer rejects filenames made entirely of blanks.** On Linux a
  filename may consist only of spaces or tabs. The empty-input guard trimmed the
  whole file list before testing it, so a lone `"   "` entry was indistinguishable
  from no input and was rejected with `error[E2]: no file list provided`. The
  guard now tests line by line, and the list is no longer trimmed before being
  sent to the model, so such an entry also survives at the start or end of a list.
  (Filenames containing newlines remain unsupported by the line-delimited input
  format — use `--in <path>`.)
- **Shell entry-point scripts are now executable on Linux checkouts.** The
  executable bit was not tracked in the Git index, so scripts landed non-executable
  after a fresh clone on Linux.

## [1.0.2] - 2026-07-17

### Fixed

- **Release archives no longer bundle the internal `lx-acceptance` harness.**
  The suite ZIPs previously shipped `lx-acceptance` — the internal self-grading
  development tool — alongside the user tools. It is now excluded; only `lx` and
  the 72 user tools are packaged.

## [1.0.1] - 2026-07-17

### Fixed

- **Ollama no longer silently truncates input.** lx now talks to Ollama's native
  `/api/chat` endpoint, which honours `num_ctx` (default 32768, configurable via
  `llm.num_ctx` / `LX_NUM_CTX`); the OpenAI-compatible `/v1` endpoint it used before
  ignores `num_ctx` and clamps the context to ~2048 tokens, cutting off larger prompts
  and causing malformed or prose output. Every other provider is unchanged (uniform
  `/v1` body, no `num_ctx`). LM Studio users: set the context window in the LM Studio
  GUI ("Context Length" ≥ 32k when loading the model) — LM Studio ignores `num_ctx`
  from the API.
- **`limits.max_output_tokens` now takes effect.** It was previously loaded but
  ignored; each request's output is now clamped to `min(per-tool budget, this)`.
  The default is raised to 4096 (the largest per-tool budget) so it never caps a
  tool by default; lower it to shorten every tool's output globally.
- **Clearer error when a model replies with prose instead of JSON.** Small local
  models can drop the required JSON format on large inputs and answer in plain
  text. The parse error no longer misattributes this to truncation / `max_tokens`;
  it now states the model returned text, shows a short excerpt of the reply, and
  suggests a stronger model or a smaller input. Genuinely truncated responses
  still point at `max_tokens` as before.

## [1.0.0] - 2026-07-12

First public release.

LX Coreutils is a suite of 72 small, fast, composable LLM-powered CLI tools —
AI-native equivalents of the Unix tools you already know. Each tool does one
thing, reads stdin, writes stdout, and pipes into the next. Cold start is under
15 ms and every tool runs on cheap models, including local 7–8B models via
Ollama or LM Studio with no API key required.

### Highlights

- **72 single-purpose tools** across text/analysis, code/dev, command
  generation, filesystem/data, security, network, diagnostics, and
  productivity. Run `lx` to browse the full catalog offline.
- **Ten providers, local-first.** Ollama (default, no key), LM Studio,
  Anthropic, OpenAI, Gemini, Groq, OpenRouter, Mistral, DeepSeek, and Azure —
  selected by config or `LX_PROVIDER`.
- **Consistent interface** on every binary: `--json`, `--plain`, `--dry-run`,
  `--lang <BCP-47>`, `--quiet`, `--verbose`, `--file`, `--max-input-bytes`,
  and a strict stdout (result) / stderr (diagnostics) split that is safe to pipe.
- **Secret & PII redaction** runs before the LLM call on every tool that
  handles sensitive input; `--dry-run` shows exactly what would be sent.
- **No command execution, no telemetry.** Command-generating tools emit text
  only; dangerous patterns are flagged locally and exit non-zero unless
  `--allow-dangerous` is passed.
- **Static single-file binaries** (musl on Linux, `+crt-static` on Windows)
  with no runtime dependencies.
- **Deterministic by design:** `temperature = 0.0` everywhere; JSON validity is
  prompt-driven with a salvage pass, keeping one uniform request shape across
  all providers.

### Distribution

- Suite ZIP per platform from GitHub Releases, containing all binaries, the
  user documents, `config.example.toml`, and the shell-integration scripts.
- Individual per-tool binaries also published on Releases.
- Optional shell integration (bash/zsh/fish/PowerShell) adds `Ctrl+K`
  (natural-language → command via `lxsh`) and `Ctrl+E` (explain via `lxexplain`).

---

*Development history prior to the 1.0.0 public release is preserved in the
project's private repository and is intentionally not reproduced here.*
