# Changelog

All notable changes to LX Coreutils are documented here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning: each tool has independent versions; the suite release label is `YYYY-MM`.

## [Unreleased]

### Fixed

- **A truncated model response no longer prints a warning that bypasses
  `--quiet` and names a remedy it never checked.** When a reply was cut at the
  token limit mid-JSON, `lx-llm` salvaged the valid prefix and printed *"Raise
  the tool's limit or narrow the input"* straight to stderr from inside `run()`
  — ignoring `--quiet`, and prescribing a fix that is often impossible: the cut
  can happen well below the tool's own cap, and a tool's `max_tokens` is a
  compile-time constant with no user-facing flag. The library is now silent and
  reports the fact in its return value, so each tool can surface it through the
  normal warning channel with a remedy it can justify. Providers' stop reasons
  (`finish_reason`, `stop_reason`, `done_reason`) are now read as well, which
  catches a reply cut on a token boundary that still parses as valid JSON — a
  case the previous JSON-only detection missed entirely.

- **`lxgrep` no longer strips the leading `/` from absolute paths.** Searching
  from the filesystem root printed matches as `var/log/syslog:12:` instead of
  `/var/log/syslog:12:`. Result paths are shown relative to the search root, and
  when that root is `/` (or a drive root on Windows) the relative form drops the
  separator, leaving a path that no longer resolves from anywhere else. Absolute
  paths now stay absolute; the short relative form is unchanged everywhere else.

- **`lxgrep` no longer warns that small results are incomplete.** A search over
  input far below the budget could still print "results are INCOMPLETE: input
  exceeded the search budget" — a 5 KB, 134-line directory listing was enough.
  The even-coverage sampler walks the input in fixed steps from the first line,
  so it stopped short of the last one whenever the step size did not divide the
  input exactly. That left a single uncovered line, and the completeness check
  ("did every line reach the model?") correctly reported it as a gap — but the
  warning told the user their input was too large, which it was not. The sampler
  now always includes the final line. Genuine capping is still reported.

## [1.1.0] - 2026-08-09

### Changed

- **Suite label moved to `2026-08`.** `--version` now reports
  `lxsum 1.1.0 (lx-coreutils 2026-08, …)`. The label marks the suite generation
  and moves on a minor release, not with the calendar.
- **`limits.max_input_bytes` raised from 512 KiB to 2 MiB.** The old default cut
  a mid-size repository's file listing in half: `find . | lxgrep "…"` on this
  repo read 512 KiB of a 1.4 MB listing, so nothing past the first third could
  match however good the sampling was. It now reads the whole thing.

  This is a **read** budget, not a send budget, so the raise does not make
  requests larger or more expensive. Tools that sample keep their fixed
  candidate budgets — a larger read just lets the sampler choose from the whole
  input instead of its first slice. Tools that send their input keep their own
  per-tool ceilings, which bind long before this limit does. Measured on
  `lxgrep`: growing the input 54× (33 KB → 1.8 MB) grows the prompt by 8%
  (14.0 KB → 15.1 KB). The cost of the raise is memory and local scan time, not
  tokens.

### Added

- **Twelve more tools report truncated input in `--json`.** `lxclog`,
  `lxcommit`, `lxconf`, `lxcsv`, `lxdebug`, `lxdiff`, `lxgrep`, `lxlog`,
  `lxnotes`, `lxpr`, `lxpull`, and `lxsecret` now emit `"input_truncated"`
  alongside their result. On `lxgrep`, `lxlog`, and `lxpull` this is a *separate*
  field from the existing `capped`/`truncated` sampling flag, because the two
  have different remedies: sampling means "narrow the query", input truncation
  means "raise `--max-input-bytes`". Both now warn distinctly on stderr.

- **Truncated input is now visible in `--json`.** When input exceeded
  `--max-input-bytes` the tool warned on stderr and carried on — invisible to
  anything parsing stdout, so a script consuming `--json` could not tell a
  summary of a whole document from a summary of its first 512 KiB. The readers
  now return that fact alongside the text (`resolve_input_checked` and friends
  in `lx-core`), and `lxsum` reports it as `"input_truncated": true`. Tools
  whose result is a claim about the *whole* input adopt this; generators, where
  the stderr warning is the right treatment, keep the existing readers.

### Fixed

- **Seven tools no longer send unbounded input to the model.** `lxclass`,
  `lxconv`, `lxgraph`, `lxnotes`, `lxpatch`, `lxpull`, and `lxtl` passed whatever
  `limits.max_input_bytes` allowed straight into the request — 512 KiB by
  default, which is already about four times the default context window
  (`llm.num_ctx`, 32k tokens). On a local model the provider silently cut the
  request short; on a hosted one it was billed in full. Each now has an explicit
  per-tool ceiling sized to what it does — 24 KB where the reply is as long as
  the input (`lxconv`, `lxtl`), 64 KB where every record must stay visible
  (`lxpull`) — and warns on stderr when it fires. For `lxconv` and `lxgraph`,
  which parse their input as records locally, the cut is trimmed back to the
  last complete line: half a CSV row does not parse and would have failed the
  whole conversion.

- **`lxcsv` now samples rows across the whole file, not just the first 50.** A
  question about a date-sorted export was answered from its oldest rows only,
  and the `used_rows` note ("50 of 20000 rows sampled") read as if the sample
  were representative. Rows are now spread evenly across the file and the note
  says so; the computed aggregates always covered every row and still do.
- **`lxgrep` now samples the whole input instead of only its head.** When a file
  produced more candidates than the budget allowed, the sampler sorted them by
  line number and truncated — which keeps the *lowest* line numbers, so the
  model only ever saw the beginning of the file. On a 200,000-line list with a
  match every tenth line it reached line 3,991: **two percent of the input**,
  presented as a complete answer. Candidates are now thinned evenly across the
  whole file, and a quarter of the budget is reserved for coverage so records
  that share no keyword with the query stay reachable even when matches alone
  could fill it. The same defect existed in three places — the line sampler, the
  block sampler, and the global cap across files — and all three are fixed.
  Coverage on the same input goes from ~2% to 100% at an unchanged budget.
- **`lxgrep` now bounds total memory when searching a directory.**
  `--max-input-bytes` is a per-file limit, and a directory walk multiplied it by
  the file count with no aggregate ceiling. Searching a large tree could pull
  hundreds of megabytes into memory. There is now a 64 MiB aggregate ceiling;
  exceeding it is a clear error naming the remedy, rather than silently
  searching whichever files sorted first.
- **`lxjson` no longer rejects input of exactly `--max-input-bytes`.** It
  inferred "input too large" from `raw.len() >= max`, which also fired when the
  input landed precisely on the limit with nothing dropped. It now uses the
  reader's truncation flag, so only genuinely oversized input is refused.
- **`lxdigest` no longer writes to stderr from inside `run()`.** Its
  listing-truncation warning bypassed `--quiet` and broke the rule that `run()`
  is pure and does no I/O. It now returns the warning to `main.rs` like every
  other tool, so `--quiet` suppresses it.
- **`lxlog` and `lxpull` no longer hide an incomplete result when piped.** Both
  reported their capping flag as narration, which is suppressed as soon as
  stdout is not a terminal — exactly the case where a silently partial answer
  does the most damage. It is now a warning (tier 2), shown unless `--quiet`.
- **Input of exactly `--max-input-bytes` is no longer reported as truncated.**
  The read loop treated "the buffer is exactly full" as overflow, so an input
  landing precisely on the limit produced a spurious truncation warning even
  though nothing had been dropped. It now distinguishes the two cases.
- **A truncated read no longer ends in a stray replacement character.** Cutting
  the byte stream at the limit can split a multi-byte character, and the lossy
  UTF-8 conversion turned that fragment into a `U+FFFD` the user never wrote —
  which was then sent to the model. The incomplete trailing sequence is now
  dropped instead.
- **The truncation warning no longer reads "0 KiB"** when `--max-input-bytes`
  is set below 1024; it reports bytes at that scale. The warning now also names
  the remedy (`raise --max-input-bytes to see more`).
- **Tools no longer crash on oversized non-ASCII input.** Eight tools cap their
  input before the LLM call (`lxclog`, `lxcommit`, `lxconf`, `lxdebug`,
  `lxdiff`, `lxdigest`, `lxpr`, `lxsum`), and each cut the text at a raw byte
  offset. When that offset fell inside a multi-byte character — an umlaut, an
  accent, a CJK glyph, an emoji — the tool panicked instead of producing a
  result. A 32 KB German log piped into `lxsum` was enough to trigger it. The
  cap now snaps back to the nearest character boundary, so the text stays valid
  UTF-8 and is at most three bytes shorter. Behaviour is otherwise unchanged;
  input that previously worked produces the same output.

## [1.0.6] - 2026-08-04

### Fixed

- **`lxgrep` now searches an order of magnitude more records when the input is a
  list.** Piping a path list (`find . | lxgrep "build related stuff"`) sampled the
  input with the strategy meant for file content: every candidate carried two
  lines of context either side, which on a list of self-contained records is
  meaningless — the neighbours of a path are unrelated paths. Four fifths of the
  budget went to that context, only ~40 records ever reached the model, and the
  budget effectively decided which records were eligible at all. `lxgrep` now
  detects line-oriented input (many short, unindented, blank-line-free lines) and
  samples it one record at a time against a larger budget: ~390 records instead of
  ~40 on the same input, with even coverage so relevant entries late in a long
  list are reachable. Source code, logs and prose are unaffected — the heuristic
  is deliberately conservative and keeps context blocks for anything structured.

- **`lxgrep` now states plainly that capped results are incomplete.** When the
  input exceeded the search budget, the warning read "a sampled subset was
  analysed", which is easy to read as a performance note rather than a
  correctness one — results that were missing matches looked authoritative. The
  warning now leads with "results are INCOMPLETE", says matching lines may be
  missing, and suggests narrowing the input. Behaviour is unchanged; only the
  wording is clearer.

### Documentation

- **Documented what the output-token caps are for.** `limits.max_output_tokens`
  and the per-tool `max_tokens` budgets are for latency on local models, cost on
  hosted ones, and pipe safety — *not* for task adherence: the cap truncates at
  the sampler, the model never sees it, so a tight cap cannot make a model
  terser or more schema-obedient (that comes from the prompt). Also records why
  the cap is not split per provider, that a truncated reply is a correctness
  event rather than a budget one, and that `lxconv` converts roughly 7–8 KB
  before hitting the ceiling. New design-doc §7.3.2 plus expanded guidance in
  `config.example.toml`. No behaviour change.

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
