# LX Coreutils — Design Document

**Status:** Living document · **Last reviewed:** 2026-07-12 · **Audience:** maintainers and contributors

LX Coreutils is a collection of **72 small, fast, LLM-powered command-line tools**
for Linux and Windows. Each tool does exactly one thing, starts in single-digit
milliseconds, reads from stdin or a file, calls one Large Language Model, and writes
a pipe-safe result to stdout. The tools share five small library crates and nothing
else. They never execute what they generate, never phone home, and redact secrets
before any data leaves the machine.

This document is the **single authoritative reference** for the suite's
architecture, technology choices, principles, conventions, and per-tool catalog. It
is written to be *read* by anyone joining the project and *updated* by anyone
changing it. It is grounded in the code as it exists today — where intent and
implementation ever diverged, this document follows the implementation.

---

## How to keep this document alive

This file is part of the codebase, not a snapshot. Treat it like source:

- **Change the code, change this document — in the same pull request.** A PR that
  adds a tool, changes a config key, alters an exit code, adds or removes a security
  behaviour, or changes a shared library API is not complete until the corresponding
  section here is updated.
- **The code is the source of truth.** If this document and the code disagree, the
  code wins and this document is the bug. Fix it.
- **Record notable revisions** in [Appendix A — Document changelog](#appendix-a--document-changelog).
- **Update the "Last reviewed" date** at the top whenever you make a substantive pass.
- Keep it self-contained. This is the only design document in the repository; do not
  introduce links to transient or external design notes that may disappear.

---

## Table of contents

1. [Introduction & Purpose](#1-introduction--purpose)
2. [Core Principles](#2-core-principles)
3. [Architecture Overview](#3-architecture-overview)
4. [The Library Crates](#4-the-library-crates)
5. [Tech Stack & Dependencies](#5-tech-stack--dependencies)
6. [Build System & Distribution](#6-build-system--distribution)
   - [6.5 Shell integration](#65-shell-integration)
7. [LLM Integration](#7-llm-integration)
8. [Configuration Reference](#8-configuration-reference)
9. [I/O & UX Conventions](#9-io--ux-conventions)
10. [Security Model](#10-security-model)
11. [Testing & Quality Strategy](#11-testing--quality-strategy)
12. [Conventions & Governance](#12-conventions--governance)
13. [Tool Catalog](#13-tool-catalog)
14. [Adding a New Tool](#14-adding-a-new-tool)
15. [Glossary & References](#15-glossary--references)
- [Appendix A — Document changelog](#appendix-a--document-changelog)

---

## 1. Introduction & Purpose

### 1.1 What LX Coreutils is

LX Coreutils brings the Unix philosophy to Large Language Models — the same way GNU
Coreutils brought it to the shell: a toolbox of small, predictable programs instead
of one monolith. Instead of one large chat application, it is **72 focused
binaries**, each named with the `lx` prefix and a short verb (`lxexplain`,
`lxcommit`, `lxsh`, `lxsum`, …). Each tool:

- does **one** job well;
- reads input from a positional argument, stdin, or `--file`;
- sends a tightly-scoped, deterministic request to a single LLM;
- prints a **pipe-safe** result to stdout and diagnostics to stderr;
- is a single static binary with no runtime dependencies.

Because each tool obeys the standard stdin/stdout contract, they compose with each
other and with classic Unix tools through ordinary pipes:

```sh
lxexplain "tar -xzf archive.tar.gz"        # explain any command
git diff --staged | lxcommit               # generate a commit message
lxsh "find all .log files older than 30d"  # generate a shell command (never run)
cat error.log | lxdebug                    # analyse an error and suggest a fix
cat README.md | lxsum                      # summarise a document
```

### 1.2 The problem it solves

LLMs are useful for small, well-defined text transformations — explain this, generate
that, summarise, classify, extract — but the dominant interface is a browser chat
window that breaks the developer's flow and cannot be scripted. A monolithic
"AI CLI" tends to grow unfocused flags and an unpredictable interface. LX Coreutils
takes the opposite bet: many tiny, predictable, composable tools that each fit one
task and behave like a well-mannered Unix utility.

### 1.3 Who it is for

Developers, sysadmins, and power users who live in a terminal and want LLM help
inside their existing pipelines and scripts — without giving up determinism,
privacy, or composability.

### 1.4 Non-goals

- **Not a chatbot.** No conversational state, no multi-turn memory.
- **Not an agent.** Tools never execute commands, edit files autonomously, or call
  each other. Composition is the *user's* job, via pipes.
- **Not offline.** Every tool needs the LLM for its core job; there is no fully local
  mode. (Security tools do the heavy lifting locally and use the LLM only for
  explanation — see §10.)
- **Not a framework.** The shared surface is five small libraries; there is no plugin
  system and no inter-tool runtime.

---

## 2. Core Principles

These are non-negotiable. Every tool and every library obeys them.

| Principle | What | Why |
|-----------|------|-----|
| **One job per tool** | Each binary does exactly one thing. | Predictable, learnable, composable; no flag soup. |
| **Composability** | Standard stdin → stdout contract on every tool. | Tools pipe into each other and into classic Unix tools. |
| **Pipe safety** | In plain mode, stdout carries *only* the result; everything else goes to stderr. | A tool's output can be fed straight into the next command without contamination. |
| **Determinism** | `temperature = 0.0` on every request, sent in the actual HTTP body. | Identical input yields identical output — essential for scripting and tests. |
| **Privacy by default** | Secret/PII redaction before the LLM call on flagged tools; no telemetry; no network calls except the LLM. | Data leaves the machine only deliberately and visibly. |
| **Never execute** | Command-generating tools emit text only; nothing is run, no profile/crontab/registry is touched. | A code tool must never become an attack vector. |
| **Cheap models suffice** | Prompts are tight enough that a small model (e.g. `claude-haiku-4-5`, `gpt-4o-mini`) produces valid output. | Low cost and latency; if a cheap model fails, fix the prompt, not the model floor. |
| **Fast cold start** | Target < 15 ms for `--help`; no async runtime; blocking single HTTP call. | A CLI tool must feel instant. |
| **Memory safety** | `#![forbid(unsafe_code)]` on every crate, with one reviewed exception in `lx-core::platform`. | Safety by construction. |
| **Minimal, permissive dependencies** | A short allow-list of MIT/Apache crates, enforced by `cargo deny`. | Small attack surface, fast builds, no license risk. |

---

## 3. Architecture Overview

### 3.1 Workspace model

The repository is a single Cargo workspace (`resolver = "2"`) containing:

- **5 library crates** under `crates/` — the *only* shared foundation.
- **72 binary crates** under `tools/` — one per tool, each producing one binary, plus `lx` (the umbrella/discovery command, see §13.13).

Tools depend on the libraries; **no tool depends on another tool.** There is no
shared runtime, no plugin registry, and no inter-tool calls. This keeps each binary
small, independently buildable, and independently releasable.

### 3.2 Repository layout

```
.
├── Cargo.toml              # workspace: members, shared deps, release profile
├── deny.toml               # cargo-deny: license allow-list, advisory & source policy
├── rust-toolchain.toml     # pinned channel = "stable"
├── README.md               # user-facing install/usage
├── CLAUDE.md               # operational quick-reference for AI coding agents
├── CONTRIBUTING.md         # contribution rules
├── CHANGELOG.md            # Keep-a-Changelog history
├── docs/
│   └── design_document.md  # ← this file (the single design reference)
├── .github/workflows/      # ci.yml, eval.yml, release.yml
├── crates/
│   ├── lx-core/            # platform, exit codes, I/O, error printing, version, locale
│   ├── lx-llm/             # two LLM clients + lang/schema/fragments
│   ├── lx-config/          # config loading & types
│   ├── lx-redact/          # secret/PII masking
│   └── lx-testkit/         # dev-only: Mock/Recording clients + assertions
└── tools/
    └── lx<name>/
        ├── Cargo.toml
        ├── README.md           # the tool's authoritative usage contract
        ├── src/
        │   ├── main.rs         # thin: arg parsing, I/O, exit codes
        │   └── run.rs          # pure: logic, no direct I/O, no process::exit
        ├── prompts/
        │   └── system.txt      # embedded via include_str!, never loaded at runtime
        └── tests/
            ├── integration.rs  # Level 1: MockLlmClient, no network
            ├── system.rs       # Level 2: binary as subprocess
            ├── eval.rs         # Level 4: #[ignore], real API
            ├── fixtures/       # committed realistic inputs
            └── snapshots/      # committed insta snapshots
```

### 3.3 Anatomy of a tool

Every tool has the same shape, which makes the suite learnable and testable:

- **`main.rs` (thin).** Parses arguments with `clap`, enables ANSI on Windows,
  loads config, resolves input (arg → `--file` → stdin), handles `--version` /
  `--dry-run`, constructs the LLM client, calls `run()`, and owns the
  **stdout/stderr split** and the process exit code. `main.rs` is the *only* place
  that performs I/O or calls `process::exit`.
- **`run.rs` (pure).** A single `run(input, config, client) -> Result<Output, LxError>`
  function holding all logic: local pre-processing, request building, the LLM call,
  schema validation, and returning a typed `Output`. It performs no direct I/O and
  never exits the process — which is exactly what makes it unit-testable with a mock
  client and no network.
- **`prompts/system.txt`.** The static, trusted system prompt, embedded at compile
  time with `include_str!`. Never read from disk at runtime.
- **`tests/`.** Integration, system, and eval tests plus committed fixtures and
  snapshots (see §11).

### 3.4 The `run()` contract

```rust
pub fn run(
    input: &str,            // already redacted if the tool has the `redact` flag
    config: &Config,        // from lx-config
    client: &dyn LlmClient, // injected — a MockLlmClient in tests
) -> Result<Output, LxError> {
    // 1. Local pre-processing in Rust (filter, truncate, aggregate)
    // 2. Build the request — tight max_tokens, temperature 0.0, static system prompt
    // 3. client.complete(&req)
    // 4. Validate the response against the tool's JSON schema (lx_llm::schema)
    // 5. Return a typed Output — no println!, no eprintln!, no exit
}
```

`main.rs` then decides which fields of `Output` go to stdout (the result) and which
go to stderr (the explanation). `run()` never makes that decision and never prints.

### 3.5 Data flow

```
            ┌─────────── main.rs (I/O, exit codes) ───────────┐
            │                                                 │
 stdin /    │   resolve_input ─▶ [redact?] ─▶ run(input, …)   │
 --file /   │        │              │            │            │
 arg ───────┼────────┘   lx_redact::redact   build Request    │
            │                                  (temp 0.0,      │
            │                                   tight tokens,  │
            │                                   static system) │
            │                                       │          │
            │                              client.complete()   │  ── HTTP ──▶  LLM
            │                                       │          │  ◀── JSON ──
            │                              lx_llm::schema       │
            │                              validate response    │
            │                                       │          │
            │                              Ok(Output) ──┬── --json ─▶ full object → stdout
            │                                           └── plain ──▶ result → stdout
            │                                                        explanation → stderr
            └─────────────────────────────────────────────────┘
```

---

## 4. The Library Crates

Five crates, each with a narrow responsibility. Every crate carries
`#![forbid(unsafe_code)]` (the sole exception is `lx-core::platform`, §4.1). All are
free of any async runtime.

### 4.1 `lx-core` — platform, I/O, errors, exit codes

The platform-neutral foundation. Modules:

- **`exit`** — the canonical exit-code constants and the unified `LxError` type:
  | Constant | Code | Meaning |
  |----------|------|---------|
  | `SUCCESS` / `EXIT_OK` | `0` | Success. |
  | `LOGICAL_ERROR` / `EXIT_ERROR` | `1` | Logical failure, **and** config/auth and network/LLM errors. |
  | `BAD_USAGE` / `EXIT_USAGE` | `2` | Wrong arguments or missing input. |
  | `DANGEROUS` / `EXIT_DANGEROUS` | `3` | Output contains a dangerous pattern; use `--allow-dangerous` to suppress (warning still printed to stderr). |
  | `SECURITY_ABORT` / `EXIT_SECURITY` | `5` | Redaction failure, path escape, dangerous pattern. |

  `LxError` has variants `LogicalError`, `BadUsage`, `ConfigAuth`, `NetworkLlm`,
  `SecurityAbort`; `exit_code()` maps each to a code (note: `ConfigAuth` and
  `NetworkLlm` both map to `1`). There is **no exit code 4.**
- **`error`** — `print_error(&LxError, json: bool)` writes the canonical error format
  to **stderr** (never stdout). Plain: `error[E<n>]: <message>` plus an optional
  `  hint: <how to fix>`. JSON: `{"error":{"code":<n>,"message":"…","hint":"…"}}`.
- **`io`** — uniform input handling: `resolve_input(file, max_bytes, timeout_ms)`
  (priority: `--file` → stdin), `read_stdin` / `read_file` (chunked, size-limited,
  truncate-with-warning), `write_atomic` (temp-file + rename), and the fsbound
  `read_file(path, max, allowed_root)` which rejects symlink escapes with
  `SecurityAbort`. `read_stdin` errors immediately if stdin is a TTY; for piped
  stdin it blocks until EOF with no timeout (slow pipes and SSH commands work).
  Default: `DEFAULT_MAX_INPUT_BYTES = 512 KiB`.
- **`version`** — `LX_SUITE_LABEL` (currently `"2026-07"`) and
  `build_version_string(binary, version)` producing
  `lxexplain 1.0.0 (lx-coreutils 2026-07, <target-triple>)`.
- **`platform`** — the **one** place allowed to use `unsafe` and
  `#[cfg(target_os)]`. Provides `config_dir()` (XDG on Linux, `%APPDATA%` on
  Windows), `is_tty(Fd)`, `os() -> &'static str` (returns `"linux"`, `"windows"`,
  or `"macos"` at compile time), locale detection, and Windows ANSI/UTF-8 console
  enablement. Every `unsafe` block carries a `// SAFETY:` comment.
- **`locale`** — a thin compatibility shim re-exporting `platform::locale` as
  `detect_lang`.

### 4.2 `lx-llm` — LLM clients and prompt utilities

Provider-agnostic LLM access. The public surface:

- **`LlmClient` trait** — `fn complete(&self, req: &Request) -> Result<Response, LlmError>`;
  `Send + Sync` so it can live behind a `Box<dyn LlmClient>`.
- **`Request`** — `{ system: &str, user: &str, max_tokens: u32, temperature: f32,
  image: Option<ImageData> }`. **`Response`** — `{ content, prompt_tokens?,
  completion_tokens? }`.
- **Two always-compiled clients:** `anthropic::AnthropicClient` (native
  `/v1/messages`) and `openai::OpenAiClient` (OpenAI-compatible
  `/v1/chat/completions`, covering OpenAI, Gemini, Groq, OpenRouter, Mistral,
  DeepSeek, Ollama, LM Studio, Azure, and any compatible endpoint). Both are
  built in by default, so switching providers is a config change, not a rebuild.
- **`client_from_config(&Config)`** — parses `config.llm.provider`, resolves the
  effective base URL and model via `config.effective_base_url()` /
  `config.effective_model()` (empty field → provider default), resolves the API
  key (local providers use a placeholder if no key is set; cloud providers error
  with a provider-specific hint), and returns the right boxed client. Only
  `"anthropic"` uses the Anthropic wire; all other provider names use OpenAI-compat.
- **`lang`** — `inject_lang(template, lang)` fills the `{lang}` placeholder in a
  system prompt; `strip_lang_fallback`. Shell-aware tools additionally replace
  `{shell}` and `{examples}` in `run.rs` after `inject_lang`.
  `inject_os(template, os_override)` fills the `{os}` placeholder (parallel to
  `inject_lang`); falls back to `lx_core::platform::os()` when override is `""` or
  `"auto"`; unlike `inject_lang` it does NOT append anything when the placeholder
  is absent — only OS-aware tools include `{os}` in their `system.txt`.
- **`schema`** — `parse_response`, `validate_json`, `extract_text` for turning the
  model's text into a validated, typed result. Tolerant parsing: strips code
  fences and `[lang-fallback]` prefixes, escapes bare control characters
  (U+0000–U+001F emitted literally by local models), fixes invalid backslash
  escapes (`\p`, `\d`, `\1`, etc. in awk/sed/regex strings → `\\p`, `\\d`, …),
  and extracts the first balanced JSON value from surrounding prose. If the response
  was truncated at `max_tokens` (unbalanced JSON, EOF mid-value), it salvages the
  largest valid prefix — closing at the outermost open collection and dropping
  any partial trailing element — then emits a one-line stderr warning rather than
  failing. This means oversized outputs degrade gracefully (a partial table/list)
  instead of erroring.
- **`fragments`** — reusable prompt constants: `UNTRUSTED_DATA_INSTRUCTION`
  (prompt-injection hardening), `JSON_ONLY_INSTRUCTION`,
  `DANGEROUS_COMMAND_INSTRUCTION`, and a `render(template, vars)` helper.
- **Robustness** — retries on transient errors (429, 5xx, network) up to
  `max_retries` with back-off; honours `Retry-After` on 429.

### 4.3 `lx-config` — configuration loading and types

Loads `Config` (nested `llm` / `limits` / `redact` / `output` sections,
all `#[serde(default)]`) from layered sources and validates it. Key points:

- **Load order**, highest priority first: CLI overrides → `LX_*` env vars →
  project-local `./.lx.toml` (with secret keys filtered out and warned about) →
  user config (`config_dir()/config.toml`) → compiled defaults.
- **Forward compatibility** — unknown TOML sections produce a stderr warning, not an
  abort. Unknown `LX_*` numeric values warn and are ignored.
- **API key never from files** — `resolve_api_key()` reads `LX_API_KEY` (or an
  injected value); the `api_key` field is `#[serde(skip)]`. Secret-looking keys in
  `.lx.toml` are stripped with a warning.
- **Typed helpers** in `types.rs`: `Provider` (10 named variants: `ollama`,
  `lmstudio`, `anthropic`, `openai`, `gemini`, `groq`, `openrouter`, `mistral`,
  `deepseek`, `azure`; see `Provider::default_base_url()` and
  `Provider::default_model()` for the per-provider defaults), `RedactLevel`
  (`Standard` | `Strict`; `off` is rejected from config — only the `--no-redact`
  flag can disable redaction), `ColorMode` (`auto` | `always` | `never`), and
  `ConfigOverrides` (the CLI-flag carrier). See §8 for every key and default.
- **`Config::effective_base_url()` / `Config::effective_model()`** — return the
  explicit value when set, otherwise the provider's built-in default. Code that
  needs the resolved URL/model must call these helpers, not read `llm.base_url` /
  `llm.model` directly (both can be empty strings).

### 4.4 `lx-redact` — secret/PII masking

Deterministic, local redaction applied *before* the LLM call on every redact-flagged
tool. `redact(input, level) -> Result<String, LxError>`:

- **`Standard`** masks API keys, bearer tokens, AWS credentials, GitHub PATs (incl.
  fine-grained), GitLab PATs, GCP keys, Slack tokens/webhooks, Stripe keys, SendGrid,
  Twilio, npm, Anthropic keys, generic secret/password assignments, connection-string
  passwords, private-key blocks, JWTs, high-entropy blobs, and email addresses
  (placeholders like `[REDACTED]`, `[EMAIL]`).
- **`Strict`** additionally masks IPv4 addresses, public hostnames, and
  home-directory paths (`[IP]`, `[HOST]`, `[PATH]`).
- **`Aggressive`** is `Strict` plus an expanded set of niche service prefixes
  (Shopify, DigitalOcean, Hugging Face, Linear, Postman, Doppler, Atlassian,
  Cloudflare, Heroku, Telegram, Discord, PyPI, GitLab runner, Square). It is what
  `lxredact --strict` selects.
- **Entropy gate** — every prefixed detector (Standard, Strict, and Aggressive
  tiers alike) pairs its prefix+length match with a per-format **Shannon-entropy
  floor** (2.0–4.0 bits/byte, matching the thresholds gitleaks uses) and a
  placeholder filter. The value following the prefix is masked only if it is
  high-entropy and does not look like a documentation example. The shared
  `lx_redact::entropy` module (`shannon_entropy`, `looks_like_placeholder`) is the
  single implementation used here and by `lxsecret`.
- **Safety guard** — if redaction would remove more than ~80 % of the input, it
  returns `LxError::SecurityAbort` rather than sending a near-empty string.
- **`has_secrets(input)`** — a fast check used in tests and the
  `assert_no_secrets_in_request` assertion.

Redaction is **best-effort, not waterproof.** It recognises known secret formats
and values assigned to a broad set of secret-context keywords (`API_KEY=`,
`token:`, `client_secret`, `refresh_token`, `webhook_secret`, …). It cannot
reliably catch a secret whose variable name carries no such keyword *and* whose
value is too short to register as high-entropy, since such a value is
indistinguishable from ordinary pipeline data (a commit SHA, a version string,
an identifier) — masking it would break pipe safety. Conversely, the entropy gate
filters placeholders and low-entropy junk that merely *match* a prefix, but a
value built from real English words (`sk_live_televisionchannelnumberone`) has
entropy comparable to a real key and is still masked. Redaction is a strong
safety net, not a guarantee.

### 4.5 `lx-testkit` — test helpers (dev-only)

A `dev-dependency` only; never compiled into production binaries. Provides:

- **`MockLlmClient`** — returns a fixed response and captures the request
  (`CapturedRequest`) so tests can assert on what was sent.
- **`RecordingLlmClient`** — wraps a real client for eval tests.
- **`binary::BinaryUnderTest`** — runs a built tool binary as a subprocess for system
  tests.
- **`assertions`** — shared checks: `assert_request_invariants` (temperature 0.0,
  non-empty system, `max_tokens` in `1..=4096`), `assert_no_secrets_in_request`,
  `assert_image_in_request`, `assert_lang_placeholder_in_system`.
- **`binary::BinaryUnderTest::for_tool_release`** — locates a tool's
  `target/release` binary (sibling of the debug `for_tool`), used by the extended
  acceptance harness.

### 4.6 `lx-acceptance` — extended acceptance harness (dev-only)

A dev-only workspace member (binary `lx-acceptance`, `publish = false`), not a
productive tool and not part of the §13 catalog. It is the self-grading,
data-driven counterpart to the human-graded smoke scripts in `acceptance/`.

- Intents live in `crates/lx-acceptance/intents/intents.toml` — one `[[intent]]`
  per graded tool invocation, carrying *necessary-truth* assertions
  (`must_contain` / `must_not_contain` / `must_match` / `expect_exit` /
  `expect_dangerous`) evaluated against the tool's `--json` output. Prose/
  structured intents assert the structural invariant (valid JSON, present
  non-empty field).
- **Few-shot contamination guard** (`src/fewshot.rs`, a `cargo test` build gate):
  an intent that merely paraphrases a few-shot example from the tool's own
  `system.txt` measures prompt *overfit*, not generalisation. The guard fails the
  build when an intent's `arg` / `--for` content is ≥0.70 word-set similar to a
  single-line example, or when a `stdin` fixture shares a contiguous run of 3+
  distinctive lines with a multi-line example artifact (word-set similarity is
  useless for fixtures — two diffs or two TLS errors share domain vocabulary
  without copying). Set `allow_fewshot_overlap = true` (with a justifying comment)
  to exempt an intent — reserved for genuinely destructive regressions where the
  canonical phrasing *is* the test (e.g. lxdockercmd "update images" → pull, not
  prune). See §7 on the few-shot overfit risk this defends against.
- **Execution oracles** grade where output is checkable by *using* it, with only
  pure (side-effect-free) functions: `regex` (compile the generated pattern with
  the same `regex` crate the tools target), `json` (artifact must parse), and
  opt-in external `jq`/`sed` (probe-or-SKIP). **Command-shaped output of
  dangerous tools is never executed** — graded structurally only, consistent with
  the suite's own "never execute" rule (§10).
- Run with `target/release/lx-acceptance --yes`; `--tool <name>` filters,
  `--target <os>` selects the OS for OS-aware tools, `--extended` also runs
  intents tagged `extended = true` (the 2nd/3rd intent of each class — skipped by
  default to save cost). Without `--yes` the harness prompts `[y/N]` before making
  LLM calls. Exits non-zero if any intent fails (CI-gateable). Uses only
  allow-listed crates (`toml`, `serde_json`, `regex`, `clap`, `once_cell`) plus
  `lx-testkit`/`lx-llm`/`lx-config`/`lx-core`.
- **`--judge`** adds an advisory, non-gating section that runs a *dedicated*
  strong model (configured separately via `LX_JUDGE_PROVIDER` / `LX_JUDGE_MODEL`
  / `LX_JUDGE_API_KEY` env vars or `--judge-provider` / `--judge-model` flags)
  over `prose = true` intents, asking three binary questions per output: relevant,
  complete, safe. Exits 2 immediately if `--judge` is used without a judge model
  configured — no silent fallback to the tool model. Never changes the exit code.

---

## 5. Tech Stack & Dependencies

- **Language:** Rust, edition 2021, pinned to an **exact** version via
  `rust-toolchain.toml` (not the floating `stable` channel — see the version
  policy below).
- **Editions of discipline:** `#![forbid(unsafe_code)]` everywhere except
  `lx-core::platform`. No async runtime anywhere.

**Version-pinning policy (reproducibility for the ~20-year horizon).** Anything
that determines a reproducible build or a CI pass/fail is pinned to an exact
version; every upgrade is a deliberate, dated, reviewed commit, never ambient
drift. Manifests express intent (ranges); the lockfile and toolchain express
reproducibility (exact). Layers: `rust-toolchain.toml` = exact Rust version,
duplicated (in lock-step) into `dtolnay/rust-toolchain@<version>` in the CI
workflows; `Cargo.toml` = caret ranges with tested-minor lower bounds; `Cargo.lock`
= committed, exact transitive pins; GitHub Actions = major tags. The full rule
and the upgrade ritual live in [`CONTRIBUTING.md`](../CONTRIBUTING.md) under
"Toolchain & dependency policy" — that is authoritative; this is the summary.

**Approved dependency allow-list** (all MIT/Apache-class, declared as
`workspace.dependencies`):

| Crate | Purpose | Notes |
|-------|---------|-------|
| `clap` (derive) | Argument parsing | The standard; derive keeps `main.rs` declarative. |
| `serde` + `serde_json` | (De)serialisation | Config and LLM JSON. |
| `toml` | Config file parsing | `0.8`. |
| `ureq` (with `json`) | **Blocking** HTTP | Chosen over `reqwest` precisely to avoid pulling in an async runtime. |
| `thiserror` | Error derive | Backs `LxError` / `LlmError`. |
| `regex` + `once_cell` | Pattern matching | For `lx-redact` and local pre-processing; lazy-compiled. |
| `insta` (dev) | Snapshot tests | Dev-dependency only. |
| `rustls` | TLS | Transitive via `ureq`; no OpenSSL/system TLS dependency. |

**Explicit bans:**

- **No async runtime** — no `tokio`, `async-std`. Tools are short-lived; one blocking
  HTTP call is correct and faster to start.
- **No `reqwest`** — it drags in async; `ureq` is the approved HTTP client.
- **No copyleft** — no GPL/LGPL/AGPL/MPL code.
- **No new dependency** without an explicit, justified PR.

**Enforcement — `cargo deny` (`deny.toml`):** a license allow-list (MIT, Apache-2.0
and the Apache LLVM exception, BSD-2/3, ISC, Unlicense, Zlib, the Unicode licenses,
CDLA-Permissive-2.0, CC0-1.0); RUSTSEC advisory checks (`version = 2`, no ignores);
`multiple-versions = "warn"` and `wildcards = "warn"`; and crates restricted to
crates.io (`unknown-registry`/`unknown-git` denied). `cargo deny check` must pass in
CI.

---

## 6. Build System & Distribution

### 6.1 Release profile (tuned for cold start)

Defined once in the workspace `Cargo.toml`:

```toml
[profile.release]
opt-level     = "z"     # optimise for size → smaller binary, faster load
lto           = true    # link-time optimisation
codegen-units = 1       # better optimisation at the cost of compile time
panic         = "abort" # no unwinding tables; smaller, faster
strip         = true    # strip symbols
```

The goal is a sub-15 ms cold start for `--help`. Verify with
`hyperfine --warmup 3 'target/release/<tool> --help'`.

### 6.2 Static binaries & targets

Tools ship as single static binaries with no runtime dependencies:

- **Linux:** musl targets `x86_64-unknown-linux-musl` and
  `aarch64-unknown-linux-musl`.
- **Windows:** `x86_64-pc-windows-gnu` (release pipeline), MSVC for local dev.

CI (`ci.yml`) builds the whole workspace for both musl targets on every push/PR.

Two release pipelines exist:

- **`release.yml`** — triggered by a tag of the form `lx<tool>-vX.Y.Z`. Builds
  that single tool for all three targets and publishes a GitHub Release with
  per-binary artifacts and `.sha256` checksums.
- **`release-coreutils.yml`** — triggered by a tag of the form `suite-vX.Y.Z`.
  Builds the entire workspace for all three targets and produces one ZIP per
  target containing all binaries plus the user-facing documents (see §6.4).
  Each ZIP has a matching `.sha256` checksum.

### 6.3 Supported platforms

| Platform | Minimum |
|----------|---------|
| Linux | Kernel 3.17+ (musl static) |
| Windows | Windows 10 1903+ |
| macOS | 11.0+ (build from source) |
| Rust (build) | Exact pinned toolchain, see `rust-toolchain.toml` |

### 6.4 Installation

- **Suite ZIP** — download `lx-coreutils-<version>-<target>.zip` from a
  `suite-vX.Y.Z` GitHub Release. Contains all 72 binaries plus
  `README.md`, `CHANGELOG.md`, both licence files, `config.example.toml`,
  and the `shell-integration/` scripts. Verify with the matching `.sha256`.
- **Individual binary** — download a single `lx<tool>-<target>` artifact
  from a `lx<tool>-vX.Y.Z` GitHub Release and verify its `.sha256`.
- **Build from source:** `cargo build -p <tool> --release`.

To build a suite ZIP locally use the scripts in `scripts/`:

```sh
# Linux / macOS
./scripts/build-release-zip.sh 1.0.0

# Windows (PowerShell 7+)
.\scripts\build-release-zip.ps1 -Version 1.0.0

# Windows (CMD — wraps the PowerShell script)
scripts\build-release-zip.bat 1.0.0
```

Both scripts detect the host target triple automatically and write the ZIP to
`dist/lx-coreutils-<version>-<target>.zip`.

`scripts/build-release-zip.sh` has its executable bit set in the Git index
(`git add --chmod=+x`) so it is immediately runnable after checkout on
Linux/macOS without a manual `chmod +x`.

### 6.5 Shell integration

The `shell-integration/` directory contains optional scripts for bash, zsh,
fish, and PowerShell. They are not part of the build and not installed
automatically — users source them from their shell rc file. They add three
interactive conveniences:

| Feature | Trigger | Shells | Tools used |
|---------|---------|--------|------------|
| Plain-English → command | `Ctrl+K` | bash, zsh, fish, PowerShell | `lxsh` |
| Explain current buffer | `Ctrl+E` | bash, zsh, fish, PowerShell | `lxexplain` |

**Ctrl+E behaviour:** echoes the command on its own line, clears the buffer,
submits an empty line for a clean prompt cycle, then prints the explanation
below. The original command is not restored in the buffer — the user retypes
it if they want to run it.

**CMD (Command Prompt) — not supported.** CMD has no readline API; there is
no mechanism to intercept keystrokes during line editing. Users on Windows
should use PowerShell, which ships by default on Windows 10+ and provides the
full integration via `lx.ps1`.

**Design constraints:**

- The scripts must never write to rc files, PATH, or any persistent state
  (except for the explicit one-time setup command the user runs themselves).
- `Ctrl+K` leaves the buffer unchanged if `lxsh` produces no output, so the
  user never loses input.
- The `Ctrl+K` binding overrides readline's default "kill line". This is
  documented as a known conflict; users who need "kill line" can rebind the
  function to any other key after sourcing the script.
- The PowerShell script sets `[Console]::OutputEncoding = UTF8` on load to
  ensure lx tool output (bullets, accented characters) renders correctly.
- The PowerShell script requires PSReadLine 2.0+ (ships with Windows by
  default).

The scripts are included in the suite release ZIP under `shell-integration/`
alongside their own `README.md`.

---

## 7. LLM Integration

### 7.1 Provider agnosticism

The suite supports two client implementations, both always compiled in:

- **Anthropic-native** (`/v1/messages`) — recommended when you have an Anthropic key.
- **OpenAI-compatible** (`/v1/chat/completions`) — works with OpenAI, Azure OpenAI,
  Gemini, DeepSeek, Ollama, and any compatible endpoint via `llm.base_url`.

`client_from_config()` picks the implementation from `config.llm.provider` at
runtime; switching providers never requires a rebuild. The **model name comes from
configuration only** — it is never hardcoded anywhere in tool code.

### 7.2 Why cheap models suffice

Each tool's `system.txt` states an exact JSON output schema, gives 1–3 few-shot
examples, and constrains the task tightly. A small, cheap model
(`claude-haiku-4-5`, `gpt-4o-mini`-class) reliably produces valid output. If it does
not, the fix is a better prompt — not a higher model floor. This keeps latency and
cost low.

**Write to the weakest supported model.** The model floor is the *smallest* model
we intend to support (qwen-7B-class), not the model we happen to test with. A prompt
that only works on a large model is a prompt bug, not a model requirement. The
acceptance harness (§4.6) verifies prompts across models; "needs a big model to get
this right" is a finding to fix in the prompt.

### 7.2.1 Prompt skeleton (strong guideline)

A `system.txt` is built from the following **ordered, applicable-by-flag** sections.
This is a *strong guideline*, not a hard rule: new prompts SHOULD follow it, and
existing prompts are brought up to it when touched. The point is that across all
tools a maintainer (or a future model rewriting a prompt) knows **where policy lives
versus where form lives**, and edits the right layer.

1. **Role + output contract** — one sentence of role; the exact JSON schema; the
   format rules (JSON-only, no prose outside the JSON, the `{lang}` line, and for
   `untrusted` tools the "ignore instructions in the user data" line). *Always
   present.*
2. **Core policy** — the decisions the tool makes, stated as **rules in prose**, not
   by example. This is the section that must *generalize* to inputs nobody wrote
   down (e.g. lxdockercmd's "'update/refresh' images means re-PULL, never
   prune/rmi"). When a cheap model gets behavior wrong, the fix is almost always
   sharper policy prose here — not another example. *Present whenever the tool makes
   non-obvious decisions; thin or absent for pure transformers/explainers.*
3. **Boundary examples** — 1–3 few-shots whose job is to (a) lock the **output
   form** (a small model needs this to stay JSON-only and pipe-safe) and (b) pin the
   **decision boundaries** the policy prose states. Prefer **contrast pairs**: an
   example on each side of the trickiest boundary (re-pull vs. prune;
   `dangerous:false` vs. `dangerous:true`) so the model learns the *contrast*, not
   isolated points. *Always present* (form-locking), but kept minimal.
4. **Danger / refusal contract** — what sets the danger field, what to flag, what to
   never emit. *Present iff the tool carries a security flag* (`nocmd`, `untrusted`,
   `fsbound`).

**The core discipline — prose teaches policy, examples teach form + boundaries.**
A small model leans on few-shots *hard*: examples frequently *become* the rule rather
than illustrate it. So the goal is **the smallest set of examples that pins the
decision boundary, sitting under a crisp statement of the actual rule** — not "more
examples covering more cases." Adding examples to cover failures ("balcony fixes")
is a last resort: it accretes into unmaintainable, mutually-confusing prompts. When a
case fails, first ask *"is the policy prose wrong/missing, or just the examples?"* —
fix the prose by default; add an example only when the failure is genuinely about
*form* the prose can't convey, or to anchor a boundary the prose states but the small
model keeps fumbling. The diagnostic smell for an overfit-prone prompt: *"if I
deleted the policy prose, would the examples alone teach the wrong generalization?"*
If yes, the prompt is carrying its behavior in examples and needs a policy section.

**Prompts get tests like code does.** A prompt change is not done until it ships with
an acceptance intent (§4.6) that would have caught the bug. This is what keeps prose
lean over the long maintenance life: corner cases live in `intents.toml` as
regressions, not as accreted prompt rules. The prompt can then be cleaned up — even
regenerated by a future better model — and the intents prove it did not regress.

(Note: some tools build their examples in code via an `{examples}` placeholder filled
at runtime — e.g. lxsh's per-shell example files — rather than inlining them in
`system.txt`. The skeleton still applies; section 3 just lives in the injected
fragment.)

### 7.3 Request invariants

Every request a tool builds must satisfy (and tests assert) these invariants:

- **`temperature = 0.0`**, carried through into the actual HTTP body — determinism
  depends on this reaching the API, not merely being set in a struct.
- **Tight `max_tokens`**, set per tool (see the catalog in §13). The global config
  cap (`limits.max_output_tokens`, default 1024) and the per-tool constant both
  apply — the smaller wins.
- **Static, trusted system prompt** separated from **untrusted user data**. The
  system prompt is the only source of the task; for `untrusted` tools it explicitly
  instructs the model to ignore any instructions inside the data
  (`UNTRUSTED_DATA_INSTRUCTION`).
- **Schema-validated response** — the model's text is parsed and validated through
  `lx_llm::schema` before becoming a typed `Output`.

JSON validity is achieved by **prompt + few-shot examples + `temperature = 0.0` +
post-hoc parse/salvage** (`lx_llm::schema`), **not** by constrained decoding. The
request body is a single uniform shape (`model` / `messages` / `max_tokens` /
`temperature`) across all providers; it carries no `response_format`, `json_schema`,
`format`, `grammar`, or `tools`/`tool_choice` fields.

### 7.3.1 Why no constrained decoding (deliberate)

Constrained / guided decoding (provider-enforced JSON Schema or GBNF grammar) was
considered as a reliability aid for small local models — the default path (ollama,
LM Studio with qwen2.5 / llama3.1-class models) — and **deliberately not adopted**.
Reasons:

- **Not portable across providers.** The mechanism differs per backend (OpenAI/Azure
  `response_format: json_schema`, Ollama top-level `format`, llama.cpp/LM Studio
  `grammar`, vLLM `guided_json`), and Anthropic-native has no equivalent. Adopting it
  means per-provider branching of the request body — the same wall hit and declined in
  the reasoning-suppression work (LM Studio ignores such API fields; some endpoints
  `400` on unknown fields). It contradicts the one-uniform-body design.
- **No machine-readable schemas exist.** Each tool's contract lives as prose +
  few-shot examples in `system.txt`, and `lx_llm::schema` is hand-rolled per tool.
  Constraining decode would require authoring and maintaining a real JSON Schema for
  all 72 tools in lockstep with the prose — a large new surface.
- **It does not fix the failures that matter.** It guarantees *well-formed,
  schema-valid* JSON, but the local-model failures that fail acceptance are *semantic*
  (wrong-but-valid output — the known small-model failure modes in §11.3). The
  malformed-JSON failures it would fix are already absorbed by the salvage layer.
- **It cannot replace the salvage layer.** Anthropic-native and cloud providers stay
  prompt-only, so `lx_llm::schema` salvage remains regardless; constrained decoding
  would be purely additive for one subset, not a simplification.

The cheaper, portable lever for local-model JSON reliability is tighter prompts and
few-shot examples (§7.2.1), measured by the acceptance harness (§11.3). Revisit only
if an acceptance run shows *malformed JSON specifically* (not semantic error) is a
material share of local-model failures after salvage; if so, scope it narrowly —
opt-in config key, Ollama-only first, schema generated from the Rust `Output` structs
(one source of truth), salvage layer untouched.

### 7.4 Robustness

- Retries on transient failures (429, 5xx, network) up to `max_retries` with
  back-off; honours `Retry-After` on 429.
- `--verbose` prints a config summary (model, provider, lang, redact) before the
  LLM call, token counts after it, and retry-attempt logging during it — all to
  stderr. Token logging and retry logging are gated on the same `verbose` flag
  passed to `client_from_config(config, verbose)`.

---

## 8. Configuration Reference

### 8.1 Source priority

Highest priority first (a higher layer overrides a lower one field-by-field):

1. **CLI flags** (`--model`, `--lang`, `--max-input-bytes`, …) via `ConfigOverrides`.
2. **`LX_*` environment variables.**
3. **Project-local `./.lx.toml`** — secret-looking keys are stripped with a warning.
4. **User config** — `$XDG_CONFIG_HOME/lx/config.toml` (Linux) or
   `%APPDATA%\lx\config.toml` (Windows).
5. **Compiled-in defaults.**

After all layers are applied, `lang = "auto"` is resolved against the system locale.
The result is validated (provider, redact level, color mode, and positive numeric
limits) before use.

### 8.2 Keys, defaults, and env vars

| Section | Key | Default | Env var | Notes |
|---------|-----|---------|---------|-------|
| `llm` | `provider` | `"ollama"` | `LX_PROVIDER` | Named provider; see Provider enum for all valid values. |
| `llm` | `base_url` | `""` (uses provider default) | `LX_BASE_URL` | Non-empty overrides the provider default (Bedrock, Vertex, Azure…). |
| `llm` | `model` | `""` (uses provider default) | `LX_MODEL` | Non-empty overrides the provider default. Never hardcoded in tool code. |
| `llm` | `timeout_secs` | `30` | `LX_TIMEOUT_SECS` | Must be > 0. |
| `llm` | `max_retries` | `3` | `LX_MAX_RETRIES` | Transient errors only. |
| `llm` | `api_key` | *(none)* | `LX_API_KEY` | **Never** from config files; env / credential store only. |
| `limits` | `max_input_bytes` | `524288` (512 KiB) | `LX_MAX_INPUT_BYTES` | Truncate-with-warning, not abort. |
| `limits` | `max_output_tokens` | `1024` | `LX_MAX_OUTPUT_TOKENS` | Global cap; per-tool limit may be tighter (smaller wins). |
| `redact` | `level` | `"standard"` | `LX_REDACT_LEVEL` | `standard` or `strict`; `off` rejected here (use `--no-redact`). |
| `output` | `lang` | `"auto"` | `LX_LANG` | BCP-47 tag or `auto` (detect from locale). |
| `output` | `color` | `"auto"` | `LX_COLOR` | `auto` / `always` / `never`. |
| `output` | `shell` | `"auto"` | `LX_SHELL` | **Runtime-only; not persisted.** `auto` calls `platform::detect_shell()` — checks `LX_SHELL`, then `PSModulePath` (Windows PowerShell), then `$SHELL` (POSIX). Override per-invocation with `--shell`. |
The authoritative annotated template is
[`crates/lx-config/config.example.toml`](../crates/lx-config/config.example.toml).
API keys must come from `LX_API_KEY` or the OS credential store, never a file.

---

## 9. I/O & UX Conventions

### 9.1 Mandatory flags

Every binary supports the same flags, parsed in `main.rs`:

| Flag | Meaning |
|------|---------|
| `--help`, `-h` | Usage; exit 0. |
| `--version`, `-V` | Canonical version string (§9.4); exit 0. |
| `--json` | Emit the full result object as JSON on stdout. |
| `--plain` | Disable ANSI colours/formatting. |
| `--dry-run` | Show what would be sent to the LLM, then exit without sending. |
| `--quiet`, `-q` | Suppress diagnostics on stderr. |
| `--lang <BCP-47>` | Output language (`en`, `de`, `fr`, …); `auto` detects from locale. |
| `--verbose` | Print config summary, token counts, and retry attempts to stderr. |
| `--max-input-bytes <n>` | Override the stdin size limit. |
| `--file <PATH>` | Read input from a file instead of stdin. |
| `--no-redact` | *(redact-flagged tools only)* Skip redaction; warns prominently on stderr. |
| `--shell <shell>` | *(nocmd tools only)* Target shell: `bash`, `zsh`, `sh`, `fish`, `powershell`, `cmd`. Auto-detected from environment if omitted. |

Input resolution always goes through `lx_core::io::resolve_input` (positional arg, if
the tool takes one → `--file` → stdin); tools do not call `read_stdin` directly.

### 9.2 Pipe safety — the most important I/O rule

Every tool must be safe inside a pipeline with classic Unix tools or other lx tools.

- **Plain mode:** stdout contains **only the result** — the regex, the command, the
  SQL, the code, the summary. No `#` comments, no explanations, nothing else.
  Explanations go to **stderr** (see tier table below).
- **`--json` mode:** the complete object (all fields, including the explanation) goes
  to stdout — fine, because the consumer parses fields explicitly.
- **Exception — tools whose purpose *is* explanation** (`lxexplain`, `lxdiff`,
  `lxman`, `lxperm`, etc.): the explanation *is* the result, so it goes to stdout.
  The test: what would the user pipe or redirect to a file? That is the result and
  belongs on stdout. What would they only read? That belongs on stderr.

**stderr three-tier policy** (matching `grep -q` / `curl -s` convention):

| Tier | Examples | interactive¹ | piped/redirected² | `--verbose` | `--quiet` |
|------|----------|--------------|-------------------|-------------|-----------|
| **Narration** | `# {explanation}`, `Cause: …` | shown | **hidden** | shown | hidden |
| **Warnings** | input truncated, redaction fired | shown | shown | shown | **hidden** |
| **Danger / security** | ReDoS, dangerous command, redaction failure | shown | shown | shown | **always shown** |
| **Errors** | any `LxError` | shown | shown | shown | always shown |

¹ *interactive* = both stdout and stderr are TTYs. ² *piped/redirected* = either
stream is consumed by a program or file (`cmd | other`, `cmd > f`, `cmd 2>log`).

Precedence: `--quiet` > `--verbose` > interactive-default. Narration keys on
**stdout** being a TTY (like `ls`), which is what makes the common `cmd | other`
case quiet even though stderr is often still the terminal — so nobody needs
`2>/dev/null`. `--quiet` has a real job everywhere (it kills warnings too, but
not danger or errors).

The split lives in `main.rs`, not `run()`. Use the helpers from `lx_core::output`:

```rust
// At the top of main(), after Cli::parse(), before any I/O:
lx_core::output::set_quiet(cli.quiet);

// In the success branch:
let out = run(&input, &config, client.as_ref())?;
if args.json {
    println!("{}", serde_json::to_string(&out)?);   // full object → stdout
} else {
    println!("{}", out.result_field);                // result only → stdout
    if lx_core::output::show_narration(args.quiet, args.verbose) {
        eprintln!("# {}", out.explanation);          // narration → stderr
    }
}
```

For tier-2 warnings in library code (`lx-core::io`, etc.), use
`lx_core::output::warn("msg")` — it checks the global quiet flag.

### 9.3 `--dry-run`

Prints to **stderr** (both suppressed by `--quiet`), then exits 0 without calling the
LLM:

```
[dry-run] input (N bytes):
<redacted user input>
[dry-run] system prompt:
<system.txt after inject_lang>
```

`SYSTEM_TEMPLATE` is a `pub const` in `run.rs`, so `main.rs` can render the final
prompt with `lx_llm::inject_lang(run::SYSTEM_TEMPLATE, &config.output.lang)`. Tools
that use additional placeholders (e.g. `{shell}`, `{examples}`) must also apply those
replacements in the `--dry-run` path in `main.rs`.

### 9.4 `--version` format

```
lxcommit 1.0.0 (lx-coreutils 2026-07, x86_64-unknown-linux-musl)
```

Built from `env!("CARGO_PKG_VERSION")` and `lx_core::version::LX_SUITE_LABEL`.

### 9.5 Exit codes & error format

| Code | Meaning |
|------|---------|
| `0` | Success. |
| `1` | General error — logical failure, config/auth error, or network/LLM error. |
| `2` | Bad usage — wrong arguments or no input. |
| `3` | Dangerous output — tool output contained a locally-detected dangerous pattern. Use `--allow-dangerous` to exit 0 (warning still printed to stderr). |
| `5` | Security abort — redaction failure, path escape, or dangerous pattern. |

Errors are printed by `lx_core::error::print_error` to **stderr** only:

- Plain: `error[E<n>]: <message>` and an optional `  hint: <how to fix>`.
- JSON (`--json`): `{"error":{"code":<n>,"message":"…","hint":"…"}}`.

### 9.6 stdin handling

Read via `lx_core::io` helpers. If stdin is a TTY, `read_stdin` errors immediately
with a `BadUsage` "no input" message — no timer needed. For piped or redirected stdin
it blocks until EOF with no timeout, matching the behaviour of jq, ripgrep, and every
standard Unix filter; slow sources (SSH pipelines, network streams) work without
configuration. Input over `max_input_bytes` is truncated with a stderr warning (not
an abort). Input is never fully buffered before limits are checked.

---

## 10. Security Model

### 10.1 Threat model

An LLM tool that sends input over the network and generates commands must never
become a liability. The guiding rule: **data leaves the device only deliberately and
visibly, and nothing the tool generates is ever executed.** `--dry-run` lets a user
see exactly what would be sent before sending it.

### 10.2 The security flags

Four flags describe a tool's mandatory security behaviour (shown in the catalog,
§13):

| Flag | Mandatory behaviour |
|------|---------------------|
| **`redact`** | Run input through `lx_redact::redact` *before* building the user message. On failure → exit 5. Raw input never reaches the LLM. Tests assert `assert_no_secrets_in_request`. |
| **`nocmd`** | The tool outputs text only — it never executes, and never writes to shell profiles, crontab, registry, or autostart. Before emitting any generated command/SQL/script it runs local pattern matching for dangerous constructs, marks them prominently on stderr (never suppressed), and exits 3. Callers that need exit 0 pass `--allow-dangerous`; the warning still fires. |
| **`untrusted`** | The static system prompt instructs the model to ignore any instructions embedded in the user data; trusted system text and untrusted data are kept strictly separate. |
| **`fsbound`** | The tool stays within the user-specified path; symlinks that escape the root are rejected (`read_file(.., Some(root))` → `SecurityAbort`). It does not touch `/etc`, `~/.ssh`, `%USERPROFILE%\.aws`, the registry, or system paths without explicit opt-in. |

A note on **`nonet`:** the original spec listed a `nonet` flag for the security
tools, but **no tool is offline** — each uses the LLM for explanation. In practice
the security tools (`lxsecret`, `lxredact`, `lxcve`, `lxperm`, `lxjwt`,
`lxcert`, …) do the heavy lifting **locally and deterministically** and send the LLM
only what is needed for an explanation or assessment — secret *values* are never sent
in clear text. The catalog reflects this as "local-core" in the notes rather than a
distinct flag.

### 10.3 Generated commands are never executed

Command-generating tools (`lxsh`, `lxsql`, `lxdockercmd`, `lxkubectl`, `lxrsync`,
`lxcurl`, …) print text to stdout and stop there. Local detection flags dangerous
patterns — `rm -rf /`, `dd of=/dev/…`, `mkfs`, fork bombs, `curl … | sh`,
`iwr … | iex`, `Remove-Item -Recurse`, `DROP TABLE`, `DELETE`/`UPDATE` without a
`WHERE`, force-pushes, destructive `rsync`, etc. — and surface them on stderr and in
a `dangerous: bool` JSON field (always present in every `nocmd` tool's JSON output).
Tools that suggest actions
(`lxchmod`, `lxundo`, `lxfixcmd`, `lxfixscript`, `lxcron`) never write to rc
files, crontab, or autostart.

### 10.4 Prompt-injection resilience

`untrusted` tools separate the trusted system prompt from the untrusted user data and
prepend `UNTRUSTED_DATA_INSTRUCTION`, telling the model to treat the data as plain
text and take its task solely from the system prompt.

### 10.5 Supply-chain & build security

A short permissive dependency allow-list, `cargo deny` enforcement (licenses,
advisories, source registry), `#![forbid(unsafe_code)]`, and reproducible static
builds keep the attack surface small. No telemetry, no update checks, no external
fetches — the only network call is the configured LLM endpoint (the sole exception
being `lxurl`, whose job is to fetch a user-named URL).

### 10.6 What the tools deliberately do **not** do

- Execute any generated command.
- Write to shell profiles, crontab, the Windows registry, or any autostart mechanism.
- Make network requests beyond the LLM call (no telemetry, no update checks).
- Print secrets, API keys, or raw sensitive data to stdout or stderr.
- Call another lx tool. Composition is the user's job via pipes.

---

## 11. Testing & Quality Strategy

### 11.1 Test levels

| Level | File | Network / API key | Runs in `cargo test`? |
|-------|------|-------------------|-----------------------|
| **1 — Integration** | `tests/integration.rs` | No (MockLlmClient) | Yes |
| **2 — System** | `tests/system.rs` | Tests 1–3 no; 4–6 need a key | Yes (1–3) |
| **3 — Unit** | inline `#[cfg(test)]` in crates/tools | No | Yes |
| **4 — Eval** | `tests/eval.rs` | Yes (real API) | No — `#[ignore]`, manual eval run only |

- **Integration** tests inject a `MockLlmClient`, run `run()`, validate the output
  schema and snapshots, and assert request invariants (`assert_request_invariants`)
  and — for redact tools — `assert_no_secrets_in_request`.
- **System** tests drive the built binary as a subprocess via
  `lx_testkit::binary::BinaryUnderTest`: `--version` exits 0 with the right format,
  `--help` exits 0, an unknown flag exits 2; the API-key tests (pipe-safety, valid
  JSON, quiet stderr) are `#[ignore]` and run in the manual eval workflow.
- **Eval** tests use a real client from the environment and check *structure*, not
  exact text. They are named `eval_*` and gated behind
  `#[ignore = "eval: requires LX_API_KEY"]`.

### 11.2 Snapshots & fixtures

Snapshot tests use **`insta`**; snapshots and `tests/fixtures/` are **committed**.
Fixtures are realistic and cover normal and edge cases (empty input, very large
input, and — for redact tools — input containing secrets). Review snapshot changes
with `cargo insta`.

### 11.3 Acceptance evaluation

Beyond unit/integration tests, the suite is periodically run through a two-model
acceptance evaluation: realistic inputs are sent through each tool against real
models, and a second model judges the output for structural validity, pipe safety,
redaction, and danger-flagging. Findings drive prompt and code fixes. (Past runs have
reached 100/100 passing; the harness hardens JSON parsing, honours `LX_LANG`, and
checks `--json`, redaction, and danger behaviour.)

**Cross-OS harness.** Both `acceptance/run.sh` and `acceptance/run.ps1` are
fully cross-platform and can be run from any shell on any host:

- The scripts auto-detect the host OS (`linux`, `windows`, or `macos`) and pass
  `--target <os>` to OS-aware tools (lxmount, lxfirewall, lxip, lxkill, lxfixscript),
  with an OS-appropriate intent for each.
- Override with `--target linux|windows|macos` (bash) or `-Target <value>` (pwsh)
  to generate a report for a different OS from any host. An invalid value causes an
  error and exits 2 before any tool runs.
- Reports are named `report-<model>-<target>-<timestamp>.md` so multi-OS runs don't
  collide and can be compared side-by-side.
- `run.ps1` runs under `pwsh` on Linux/macOS too (no `.exe` hardcodes; binary suffix
  is computed from the host at runtime).
- On the no-file path (stdin not needed) both scripts close stdin immediately so
  stateful tools (lxmount, lxfirewall, lxip) never block when run non-interactively.
- Both scripts include a **stale-binary guard**: before running any tool they check
  whether any `.rs` or `.toml` source file is newer than the `lx` release binary.
  If stale, a warning names the offending file and pauses 5 seconds (Ctrl-C to abort).
  Always run `cargo build --release` after committing fixes before re-running the
  harness — a stale binary will mask a fix and produce identical failures.

**Local model baseline (Qwen2.5 family).** The suite has been evaluated across the
full Qwen2.5 model family to establish the minimum viable model size for local use:

| Size  | Pass rate | Notes |
|-------|-----------|-------|
| 1.5 B | ~55 %     | Not viable — frequent hallucinations and JSON schema failures |
| 3 B   | ~65 %     | Simple command-lookup tools only; unreliable on longer outputs |
| 7–8 B | ~95 %     | Recommended minimum; handles nearly all tools reliably |
| 14 B  | ~94 %     | Near-remote quality; constrained by VRAM on long-output tools |

**7–8 B is the minimum size contributors should use when running acceptance
evaluations locally.** Smaller models produce enough failures to mask real
regressions. The CI eval workflow always uses a remote model (`LX_EVAL_MODEL`).

**Avoid reasoning/thinking models** (QwQ, Gemma 4 QAT, DeepSeek-R1, o1/o3, and
similar). These models emit a chain-of-thought preamble that consumes the per-tool
`max_tokens` budget before the JSON answer begins, causing truncated output and parse
failures across nearly all tools. Use instruct (non-reasoning) variants instead or deactivate reasoning/thinking in the model settings.

**Known small-model failure modes (qwen2.5-7b-instruct).** These are deterministic
(temperature 0) content failures observed in acceptance testing. They are
**model limitations, not suite defects** — the same prompts produce correct output on
Claude (Haiku 4.5) and Gemini (2.5 Flash Lite), and every tool still exits 0 (the
output is wrong, not broken). Do not chase these with prompt changes; they are the
"~95 %" tail of the 7–8 B baseline and disappear on remote models:

- `lxcode` "binary search in rust with tests" emits `tmod!` instead of `mod tests`
  in the test module — the generated Rust does not compile. Reproduces on both OS
  targets.
- `lxsed` emits `awk '\$1 == "ERROR" {print \$3}'` with literal backslashes before
  `$`. The model conflates JSON string-escaping with awk syntax; the lx-llm escape
  sanitizer correctly preserves the model's (wrong) literal intent rather than
  crashing on the invalid JSON escape. Correct awk has no backslashes.
- `lxundo` for `git reset --hard HEAD~5` suggests re-running `reset --hard` instead
  of the reflog-based recovery (`git reset --hard HEAD@{1}`) that Gemini produces.
- `lxgitignore` occasionally lists a pattern (`/target/`) twice under two headers.

The `lxkubectl` CrashLoopBackOff output (`kubectl get pods -n <ns> | grep
CrashLoopBackOff`) is **correct and intentional** across all models — CrashLoopBackOff
is a container waiting reason, not a pod phase, so `--field-selector=status.phase`
would return nothing. The prompt prescribes the `grep` form deliberately
(`tools/lxkubectl/prompts/system.txt`); it is not a flaw.

### 11.4 CI gates

Three GitHub Actions workflows:

- **`ci.yml`** (push/PR) — on Ubuntu and Windows: `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` (eval tests excluded by their `#[ignore]`); plus a
  release build for both musl targets and a `cargo-deny` job.
- **`eval.yml`** (manual only, `workflow_dispatch`) — runs the eval tests with
  `cargo test --workspace -- --include-ignored eval_`, using `LX_API_KEY` and
  `LX_EVAL_MODEL` from secrets/vars. These make real LLM calls and spend tokens,
  so there is deliberately no schedule — trigger it by hand before a release.
- **`release.yml`** — per-tool cross-target builds (musl x2 + Windows GNU) with
  per-artifact `.sha256` checksums.

Everything must be green — `fmt`, `clippy -D warnings`, `cargo deny check`, build, and
tests — before a PR is ready.

---

## 12. Conventions & Governance

### 12.1 Naming

- Every productive-tool binary is prefixed `lx` followed by a short, lowercase
  **verb or noun** describing its one job (`lxexplain`, `lxcommit`, `lxsh`). The
  umbrella/discovery command is simply `lx` — the entry point to the catalog, the
  way `ls` or `man` are entry points into GNU Coreutils. The full binding list is
  the workspace `members` in `Cargo.toml`; that list is authoritative for which names
  exist.
- Library crates are `lx-core`, `lx-llm`, `lx-config`, `lx-redact`, `lx-testkit`.

### 12.2 Commit & PR conventions

- **One tool (or one library change) per commit**, conventional-commit style:
  `feat(lxsum): implement`, `feat(lx-llm): add retry-after support`,
  `fix(lxcommit): ensure redaction fires before the diff reaches the LLM`.
- **English** for all code, comments, commit messages, and documentation.
- **DCO sign-off** on every commit (`Signed-off-by: Name <email>`).
- **`cargo fmt`, `cargo clippy -- -D warnings`, and `cargo deny check` must pass**
  before a PR is ready.
- Update **`CHANGELOG.md`** (Keep-a-Changelog) for every user-visible change, and
  this design document for any architectural/contract change.

### 12.3 Definition of Done (per tool)

A tool is done when:

1. `run()` is pure (no I/O, no `process::exit`) and returns a typed `Output`.
2. `main.rs` is thin, owns the stdout/stderr split, implements all mandatory flags,
   and uses `resolve_input`.
3. `prompts/system.txt` states the JSON schema, has 1–3 few-shot examples, contains
   the `{lang}` placeholder, and (for `untrusted` tools) the ignore-instructions
   line. Shell-aware tools (`lxsh`, `lxfixcmd`) also use `{shell}` and `{examples}`
   placeholders; per-shell example files live alongside `system.txt` and are selected
   in `run.rs` based on `config.output.shell`.
4. The applicable security flags are implemented exactly as in §10.
5. Integration, system, and eval tests exist with realistic fixtures; snapshots are
   committed.
6. `cargo fmt`/`clippy`/`build`/`test` pass for the tool; release cold start
   < 15 ms.
7. README and this catalog entry are accurate.

---

## 13. Tool Catalog

All 72 tools, grouped by function. Each tool's **authoritative** contract (full
flags, output schema, examples, exit codes) lives in its own
`tools/<name>/README.md` and `tools/<name>/prompts/system.txt`; this catalog is the
overview.

**Columns:** **Tool** · **Purpose** · **Input → Output** · **Tokens** (per-tool
`MAX_TOKENS`) · **Flags**.

**Flag legend:** `R` = redact (mask secrets/PII before the LLM) · `C` = nocmd
(generates text/commands, never executes, local danger detection) · `U` = untrusted
(prompt-injection hardening) · `F` = fsbound (path boundaries enforced) ·
`L` = local-core (security tool: heavy lifting is local/deterministic, LLM only
explains; secret values never sent) · `OS` = `--target linux|windows|macos` flag,
`{os}` in system prompt. `—` = no special security flag.

**Create-or-edit** tools (marked ✎) auto-detect stdin: empty stdin → create mode,
piped stdin → edit mode (modify in place, preserve everything else verbatim).

**Stateful** tools (marked ⟳) read current system state from stdin and produce
context-aware output (conflict detection, ordering, lockout warnings).

**Merged flags:** `lxsum --headline` (title/subject), `lxredact --anon` (names→roles),
`lxnotes --actions` (extract action items).

### 13.1 Text & Analysis

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxexplain` | Explain a command, error, code snippet, lint warning, dep, or tree output in plain language | arg/stdin → prose | 512 | U |
| `lxsum` | Summarise (`--headline` for title, `--short` for one sentence) | stdin/file → summary | 768 | R, U |
| `lxtl` | Translate text to a target language (`--to`) | stdin → translated text | 2048 | U |
| `lxclass` | Classify input into given labels (`--labels`) | stdin → label + confidence | 512 | U |
| `lxpull` | Extract structured fields from free text (`--fields`) | stdin → records | 1024 | R, U |
| `lxproof` | Correct grammar and spelling | stdin → corrected text | 2048 | U |

### 13.2 Code & Development

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxcode` | Generate code from a description (`--lang`) | arg/stdin → code | 2048 | C |
| `lxdebug` | Analyse error output (single or multiple errors) and suggest root causes and fixes | stdin → cause + fix | 512 | R, U, C |
| `lxdoc` | Generate docstrings/comments for code | stdin → annotated code | 2048 | U |
| `lxregex` ✎ | Generate a regex from a description (`--flavor`); edit existing with stdin | arg → pattern + explanation | 256 | C |
| `lxregexplain` | Explain what a regex does, with a structured parts breakdown | arg/stdin → explanation + parts | 512 | U |
| `lxsql` ✎ | Generate SQL from natural language (`--schema`); edit existing with stdin | arg/stdin → SQL | 512 | C |
| `lxsh` | Generate a shell command or script | arg/stdin → command | 256 | C |
| `lxtypehint` | Add type hints/annotations to code | stdin → annotated code | 2048 | C, U |
| `lxrename` | Generate a safe rename script from natural-language intent | stdin/`--in`[+`-r`] + arg → mv script | 1024 | C, F |
| `lxfixcmd` | Fix the last failed shell command | arg + stdin → corrected command | 256 | C, U |
| `lxfixscript` OS | Fix a broken shell script | stdin + optional error arg → corrected script | 1024 | C, U |
| `lxpatch` | Turn a described change into an applyable unified diff | stdin + arg → unified diff | 1024 | C |

### 13.3 Command Generation (all `nocmd` — generate, never execute)

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxjq` ✎ | Generate a `jq` expression from a description; edit existing with stdin | arg → expression | 256 | C |
| `lxcurl` | Generate a `curl` command from an API description | arg → command | 512 | C |
| `lxsed` | Generate a `sed` or `awk` text-transformation one-liner | arg → command | 256 | C |
| `lxffmpeg` | Generate an `ffmpeg` command | arg → command | 256 | C |
| `lxkubectl` | Generate a `kubectl` command | arg → command | 256 | C |
| `lxdockercmd` | Generate a `docker` command | arg → command | 150 | C |
| `lxrsync` | Generate an `rsync` command (data-loss aware) | arg → command | 512 | C |
| `lxmount` ✎ ⟳ OS | Generate a mount command + fstab line (no fstab on Windows) | arg + optional stdin → command + fstab_line | 1024 | C, U |
| `lxkill` OS | Find and kill the right process from a description | arg + optional `ps` stdin → command | 512 | C, U |
| `lxcron` ✎ | Generate or explain a crontab line; edit existing with stdin | arg → crontab line | 256 | C |
| `lxfirewall` ⟳ OS | Generate or explain firewall rules (iptables/nftables/ufw/netsh/pf) | arg + optional ruleset stdin → command | 1024 | C, U |
| `lxip` ⟳ OS | Generate or explain `ip`/`netsh`/`ifconfig` commands | arg + optional state stdin → command | 512 | C, U |
| `lxprintf` | Build a printf/date format string from a description | arg → format string | 256 | — |

### 13.4 Filesystem & Data

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxfind` | Semantic file search by description | description + path → paths | 1024 | F, U |
| `lxgrep` | Semantic content search | query + files/stdin → `file:line` hits | 2048 | F, U |
| `lxdigest` | Summarise a whole directory | path → overview | 1024 | F, R, U |
| `lxcsv` | Answer questions about CSV data | file + question → answer | 512 | R, F, U |
| `lxjson` | Repair or clean malformed JSON | stdin → valid JSON | 1024 | U |
| `lxconv` | Convert between data formats (`--to`) | stdin → target format | 4096 | U |
| `lxtable` | Convert unstructured text into a table | stdin → table | 2048 | U |
| `lxmock` | Generate realistic mock/fixture data from a description | arg → mock data | 1024 | — |

### 13.5 Search & Knowledge

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxask` | Answer a question from local context (`--context`) or knowledge | arg → answer | 1024 | R, F, U |
| `lxman` | Plain-language man page for a command | arg → explanation + examples | 512 | — |
| `lxerrno` | Explain an error code (HTTP/errno/exit) | arg/stdin → explanation | 256 | — |

### 13.6 Productivity & Communication

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxdraft` | Draft an email/ticket/doc from bullet points (`--kind`) | arg/stdin → draft | 768 | R |
| `lxcommit` | Generate a Conventional Commit message from a git diff | stdin → message | 256 | R, C |
| `lxclog` | Generate a changelog from git log | stdin → changelog | 1024 | R |
| `lxpr` | Generate a PR description from a diff | stdin → PR text | 1024 | R, U |
| `lxstandup` | Generate a standup from git activity | stdin → bullet points | 1024 | R |
| `lxtodo` | Extract TODO comments from code | stdin/path → TODO list | 1024 | F, U |
| `lxnotes` | Structure raw meeting notes (`--actions` to extract action items) | stdin → structured notes | 2048 | R, U |
| `lxgitignore` ✎ | Generate a `.gitignore` for a project; edit existing with stdin | path/stdin → gitignore | 2048 | F |
| `lxdockerfile` ✎ | Generate a Dockerfile; edit existing with stdin | arg/stdin → Dockerfile | 1024 | C |
| `lxmakefile` ✎ | Generate a Makefile/justfile; edit existing with stdin | arg/stdin → Makefile | 1024 | C |

### 13.7 Docs & Format

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxmd` | Format raw text as clean Markdown | stdin → Markdown | 2048 | U |
| `lxmermaid` ✎ | Generate a Mermaid diagram; edit existing with stdin | arg/stdin → Mermaid code | 1024 | C |
| `lxdiff` | Explain a git/file diff in plain language | stdin → explanation | 512 | R, U |
| `lxgraph` | Generate an ASCII/terminal chart from numbers | stdin → chart | 512 | — |

### 13.8 Security (local-core: heavy lifting local, LLM explains)

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxsecret` | Find accidentally committed secrets/keys (`--strict` adds a keyword-independent high-entropy sweep) | stdin/path → masked findings | 128 | L, R, F |
| `lxredact` | Mask secrets and PII (`--anon` to replace names with role placeholders; `--strict` adds PII masking + niche service prefixes) | stdin → redacted stream | 512 | L, R |
| `lxperm` | Explain file permissions and risks | stdin (`ls -l`)/path → explanation | 2048 | L, F |
| `lxcve` | Explain CVEs in a dependency lockfile | stdin/file → findings | 1024 | L, F, U |
| `lxcert` | Explain a TLS certificate | stdin (PEM)/file → explanation | 512 | L, F |
| `lxjwt` | Decode and explain a JWT token | arg/stdin → claims + explanation | 512 | L, R |
| `lxchmod` | Suggest safe file permissions | stdin (`ls -l`)/arg → suggestion | 256 | L, C, F |

### 13.9 Network & System

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxlog` | Analyse logs and detect anomalies (covers audit logs) | stdin/file → findings + summary | 2048 | R, F, U |
| `lxconf` ✎ | Check a config file for typical errors; edit existing with stdin | file/stdin → findings | 1024 | R, F, U |
| `lxport` | Explain what service runs on a port and flag any risk | arg + stdin → explanation | 512 | U |

### 13.10 Diagnostics

Paste the raw output of a failing network tool and get an explanation and fix.
These are distinct from generic `lxexplain` because they carry protocol-specific
knowledge and return structured `likely_cause` + `suggested_fix` fields.

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxdns` | Diagnose DNS problems from `dig`/`nslookup`/`host` output | stdin + optional domain arg → explanation + fix | 512 | U |
| `lxssl` | Diagnose TLS/cert errors from `openssl`/curl output | stdin + optional host arg → explanation + fix | 512 | U |
| `lxping` | Interpret ping/traceroute/mtr: network vs host problem | stdin → interpretation + verdict | 512 | U |
| `lxhttp` | Explain why an HTTP request failed (paste `curl -v`) | stdin → explanation + status + fix | 512 | U |

### 13.11 Meta & Shell

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxundo` | Suggest how to undo a command | arg/stdin → undo suggestion | 256 | C, U |

### 13.12 Web

| Tool | Purpose | Input → Output | Tokens | Flags |
|------|---------|----------------|--------|-------|
| `lxurl` | Fetch a URL and answer questions about its content | `<url>` + optional question → answer | 512 | U |

> **Note on `lxurl`:** it is the one tool that makes a network request beyond the LLM
> (it fetches the user-named URL). Fetch and HTML-stripping happen locally before the
> stripped text is sent to the LLM.

### 13.13 Suite Umbrella (`lx`)

`lx` is a **special case** — it is the entrypoint/discovery command for the
suite itself, not an LLM tool. Its name mirrors the brand: `lx` is to LX
Coreutils what `ls`/`man` are to GNU Coreutils — the place you start.

| Binary | Purpose | Input → Output | LLM | Flags |
|--------|---------|----------------|-----|-------|
| `lx` | Browse and discover all 72 lx tools (offline) | none → grouped tool list | none | — |
| `lx model` | Report the **effective** LLM model the suite will use | config → model name | diagnostic only | `--json`, `--no-verify`, `--verbose` |

**Key differences from all other tools:**

- `lx` is **not a productive LLM tool**: it never sends user data to a model
  and never produces model-generated content. The catalog/discovery surface is
  fully offline.
- **Exception — diagnostic LLM use.** Sub-commands that report on the suite's own
  configuration *may* contact the LLM purely to verify it. `lx model` reads
  the effective model/provider from config (via `lx-config::effective_model()`)
  and, unless `--no-verify` is passed, makes one minimal throwaway call to
  confirm the model answers. This is config diagnostics, not content generation —
  the response is discarded. Future config/setup sub-commands may do the same.
- Because of this, `lx` depends on `lx-config` + `lx-llm` (in addition to
  `clap` + `lx-core` + `serde`/`serde_json`). It still does **not** use
  `lx-redact` and still has **no** productive-tool flags (`--lang`, `--no-redact`,
  `--dry-run`, etc.) on its catalog surface.
- The `run(input, config, client)` contract does **not** apply to `lx`. It
  has its own subcommand-based CLI.
- The catalog surface reads **no stdin** and needs **no API key**. `lx model`
  loads config and (without `--no-verify`) needs whatever credentials that
  provider requires; `--no-verify` resolves the model offline with no API key.
- Mandatory flags from §9.1 are reduced to what makes sense (`--help`,
  `--version`, plus the `model` sub-command's three flags).
- Pipe-safety rules (§9.2): the catalog view is a help/discovery surface (relaxed
  split). `lx model` **does** follow the strict split — plain stdout is the
  model name only (one line, pipe-safe); provider/reachability go to stderr;
  `--json` emits the full object to stdout.
- The embedded tool catalog in `tools/lx/src/catalog.rs` is derived from
  this §13 table. Keep them in sync. A consistency test in `tools/lx/tests/`
  verifies catalog names match the workspace members.

**Subcommand structure** (designed for future extension):

```
lx                        # grouped overview of all tools (implicit `tools`)
lx tools                  # same, explicit
lx tools <keyword>        # substring search over name + purpose
lx tools --cat <name>     # filter by category (short id or name substring)
lx tools --json           # machine-readable JSON array
lx model                  # effective model name -> stdout; verifies via 1 LLM call
lx model --no-verify      # effective model name, resolved offline (no LLM call)
lx model --json           # {"model","provider","reachable","error"}
lx config                 # interactive wizard: create/update user config.toml
lx config --yes           # non-interactive: accept all defaults, write immediately
lx config --print         # preview TOML to stdout; do not write a file
lx config --force         # skip overwrite confirmation
lx --version / --help
```

The acceptance harness (`acceptance/run.{sh,ps1}`) uses `lx model
--no-verify --json` to label each report with the model that actually ran,
rather than trusting `LX_MODEL` (which may be unset or overridden by config).

`lx config` writes `$XDG_CONFIG_HOME/lx/config.toml` (Linux/macOS) or
`%APPDATA%\lx\config.toml` (Windows) — the same path `Config::load()` reads.
It never writes an API key to disk (`api_key` is `#[serde(skip)]`); instead it
prints provider-specific instructions for `LX_API_KEY` and may run a diagnostic
probe via `lx model` after writing. This is config diagnostics, not content
generation — consistent with `lx`'s permitted LLM use (see above).

---

## 14. Adding a New Tool

A new tool follows the same shape as every existing one. The rhythm:

1. **Decide the contract first.** Define the tool's one job, its input source
   (arg/stdin/file), its plain-text result, its JSON output schema, and which
   security flags (§10) apply. Pick the closest reference tool for style:
   - `tools/lxexplain/` — simplest, no security flags.
   - `tools/lxcommit/` — mandatory redaction before the LLM.
   - `tools/lxsh/` — generates commands with local danger detection.
2. **Add the crate to the workspace.** Create `tools/lx<name>/` with `Cargo.toml`
   (depending only on the needed libraries and the approved allow-list) and register
   it in the workspace `members` in the root `Cargo.toml`.
3. **Implement `run.rs` (pure).** `run(input, &Config, &dyn LlmClient) ->
   Result<Output, LxError>`: local pre-processing in Rust, build the request
   (`temperature = 0.0`, tight `MAX_TOKENS`, static system prompt), call
   `client.complete()`, validate with `lx_llm::schema`, return a typed `Output`. No
   I/O, no `process::exit`. Implement the security flags exactly as §10 requires
   (`redact` → `lx_redact::redact` first, fail → exit 5; `nocmd` → never execute,
   local danger detection + marking; `untrusted` → ignore-instructions in the prompt;
   `fsbound` → path-boundary check via `read_file(.., Some(root))`). Expose
   `pub const SYSTEM_TEMPLATE` for `--dry-run`.
4. **Write `main.rs` (thin).** clap parsing with all mandatory flags (§9.1), input
   via `resolve_input`, `--version`/`--dry-run` handling, the stdout/stderr split,
   and exit codes.
5. **Write `prompts/system.txt`.** Role, the exact JSON schema, 1–3 few-shot
   examples, the `{lang}` placeholder, and (for `untrusted` tools) the
   ignore-instructions line. Tight enough that a cheap model is reliable. For
   shell-aware tools add `{shell}` and `{examples}` placeholders; provide
   `examples_bash.txt`, `examples_powershell.txt`, and `examples_cmd.txt` alongside,
   and select the right one in `run.rs` via an `examples_for(shell)` function.
6. **Write tests.** `integration.rs` (MockLlmClient: schema, plain + JSON snapshots,
   request invariants, and one assertion per security flag), `system.rs` (the six
   binary tests; API-key ones `#[ignore]`), `eval.rs` (`eval_*`,
   `#[ignore = "eval: requires LX_API_KEY"]`), and committed `fixtures/` covering
   normal and edge cases.
7. **Write `README.md`** with purpose, a real input/output example, all flags, exit
   codes, and a security note.
8. **Verify locally:**
   ```sh
   cargo fmt -p lx<name> --check
   cargo clippy -p lx<name> --all-targets -- -D warnings
   cargo test -p lx<name>            # no network; eval_* are #[ignore]
   cargo build -p lx<name> --release
   hyperfine --warmup 3 'target/release/lx<name> --help'   # cold start < 15 ms
   ```
   All must be clean. Commit as `feat(lx<name>): implement`.
9. **Update this document** — add the tool to the right table in §13 and bump the
   "Last reviewed" date.

---

## 15. Glossary & References

### 15.1 Glossary

- **Pipe safety** — the discipline that, in plain mode, stdout carries only the
  result so a tool can be piped directly into another command (§9.2).
- **Redaction** — local, deterministic masking of secrets/PII before any data is sent
  to the LLM (`lx-redact`, §4.4).
- **fsbound** — a security property: a tool stays within a user-specified path and
  rejects symlink escapes (§10.2).
- **untrusted** — a security property: the prompt instructs the model to ignore
  instructions embedded in user data (§10.2, §10.4).
- **nocmd** — a security property: the tool emits commands as text and never executes
  them, with local danger detection (§10.2, §10.3).
- **local-core** — describes the security tools: the analysis is done locally and
  deterministically; the LLM is used only for explanation, and secret values are
  never sent (§10.2).
- **Eval test** — a `#[ignore]`d test that calls a real model and checks structural
  quality, not exact text; runs in the manual eval workflow (§11.1).
- **Request invariants** — `temperature = 0.0`, non-empty system prompt, `max_tokens`
  within `1..=4096`; asserted in every integration test (§7.3, §11.1).

### 15.2 References

| Resource | What it is |
|----------|------------|
| [`README.md`](../README.md) | User-facing install and usage overview. |
| [`CLAUDE.md`](../CLAUDE.md) | Operational quick-reference for AI coding agents. |
| [`CONTRIBUTING.md`](../CONTRIBUTING.md) | Contribution rules. |
| [`CHANGELOG.md`](../CHANGELOG.md) | Keep-a-Changelog release history. |
| [`crates/lx-config/config.example.toml`](../crates/lx-config/config.example.toml) | Authoritative annotated config template. |
| [`.github/workflows/`](../.github/workflows/) | `ci.yml`, `eval.yml`, `release.yml` — the authoritative build/test/release pipelines. |
| `tools/<name>/README.md` | Each tool's authoritative usage contract. |
| `tools/<name>/prompts/system.txt` | Each tool's exact prompt and output schema. |

---

## Appendix A — Document changelog

| Date | Change | Author |
|------|--------|--------|
| 2026-07-12 | Initial public release (1.0.0). | BrunkenClaas |

