# Changelog

All notable changes to LX Coreutils are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning: each tool has independent versions; the suite release label is `YYYY-MM`.

## [Unreleased]

### Fixed

- **Local models no longer silently truncate input.** Requests to local providers
  (Ollama, LM Studio) now send `num_ctx` (default 32768, configurable via
  `llm.num_ctx` / `LX_NUM_CTX`), so the model receives the full prompt instead of
  being cut off at Ollama's small default context (~2–4k) — the cause of malformed
  or truncated output on larger inputs. Hosted providers are unaffected: the field
  is omitted from their request bodies.
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
