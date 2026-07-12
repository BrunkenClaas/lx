# Changelog

All notable changes to LX Coreutils are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning: each tool has independent versions; the suite release label is `YYYY-MM`.

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
