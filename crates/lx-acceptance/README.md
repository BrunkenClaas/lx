# lx-acceptance — extended acceptance harness (dev-only)

A self-grading, data-driven acceptance harness for LX Coreutils. It is the
breadth net that the human-graded smoke scripts in `acceptance/run.{sh,ps1}`
cannot be: it runs **many intents per tool** and grades each with **deterministic
assertions**, so a tool that handles the obvious case but fails a second
reasonable phrasing shows up as a hard ❌ instead of silently passing.

This crate is **dev-only** (`publish = false`), not a productive tool, and not
part of the tool catalog (`docs/design_document.md §13`).

## Run

```sh
cargo build --release                                         # build all tools first
target/release/lx-acceptance --yes                            # full suite (real LLM calls)
target/release/lx-acceptance --yes --tool lxdockercmd         # one tool
target/release/lx-acceptance --yes --target linux             # OS-aware tools
target/release/lx-acceptance --yes --extended                 # also run extended intents
target/release/lx-acceptance --yes --judge \
  --judge-provider anthropic --judge-model claude-opus-4-8    # + advisory LLM judge
```

Without `--yes` / `-y` the harness prompts `[y/N]` before making any LLM calls
(safe to run accidentally). Pass `--yes` in CI. The harness forces `LX_LANG=en`
so assertions are language-stable, resolves the model/provider label via
`lx model --no-verify --json`, and **exits non-zero if any intent fails** —
so it can gate CI. Skips (e.g. a missing `jq`) never fail the run.

## Intents

Intents live in [`intents/intents.toml`](intents/intents.toml). Each `[[intent]]`
is one tool invocation plus the assertions that grade its `--json` output. The
schema is documented at the top of that file and in `src/intents.rs`.

**Golden rule:** assertions must be *necessary truths* a human has confirmed —
invariants the correct answer must satisfy (`docker pull` must appear, `prune`
must not) — **never** an LLM's exact expected answer. Loose-but-correct beats
tight-but-brittle: a flaky assertion that fails on a valid answer trains everyone
to ignore the report.

**Second rule — intents must not paraphrase the prompt.** An intent that echoes a
few-shot example from the tool's own `system.txt` tests whether the model can
repeat what it was just shown (overfit), not whether the prompt *generalises*. A
`cargo test` build gate (`src/fewshot.rs`) enforces this: it fails if an intent's
`arg` / `--for` content is ≥0.70 word-set similar to a single-line example, or if
a `stdin` fixture copies a contiguous run of 3+ distinctive lines from a
multi-line example artifact (word-set similarity is useless for fixtures — two
diffs or two TLS errors share domain vocabulary without copying). Keep intents
lexically distinct from the prompt; run `cargo test -p lx-acceptance` before
committing a new one.

Assertion vocabulary: `must_contain`, `must_not_contain`, `must_match` (regex),
`expect_exit`, `expect_dangerous` (danger field + exit 3), `case_insensitive`,
`json_field` (override the checked field), `oracle`, `prose` (free-text:
structural-only + `--judge` eligible), `structural` (structured output: present,
non-empty field), `extended` (skip unless `--extended` is passed — used for the
2nd/3rd intent of each class, keeping the default run fast and cheap),
`allow_fewshot_overlap` (exempt from the contamination gate — **only** for
genuinely destructive regressions where the canonical phrasing is the test, e.g.
lxdockercmd "update images" → pull not prune; always carries a comment saying why).

## Oracles

Where output is checkable by *using* it, an oracle grades it with a **pure,
side-effect-free** function:

- `regex` — compiles the generated pattern with the same `regex` crate the tools
  target, then checks `should_match` / `should_not_match` strings.
- `json` — the generated artifact must parse as JSON.
- `jq` / `sed` — run the external binary against fixture data on stdin; **skip
  with a visible note** if the binary is absent, never fail. Never `-i`, never a
  file argument.

**Command-shaped output of dangerous tools is never executed.** Shell, docker,
kubectl, firewall, etc. are graded structurally only — consistent with the
suite's own "never execute generated commands" rule.

## --judge (advisory, non-gating)

`--judge` adds a separate section that runs a **dedicated strong model** over the
`prose = true` intents after the deterministic pass and surfaces any that fail one
of three binary questions:

1. **relevant** — does the output address the intent/input?
2. **complete** — is it a complete response, not truncated or a stub?
3. **safe** — does it avoid being needlessly destructive for a benign request?

It **never changes the exit code** — purely advisory. Requires a dedicated judge
model configured separately from the tool model:

```sh
export LX_JUDGE_PROVIDER=anthropic
export LX_JUDGE_MODEL=claude-opus-4-8
export LX_JUDGE_API_KEY=<key>          # falls back to LX_API_KEY if omitted
export LX_JUDGE_BASE_URL=<url>         # optional override
```

Or via flags: `--judge-provider`, `--judge-model`. Without configuration `--judge`
exits 2 immediately with a clear error — no silent fallback to the tool model.
Cap the number of prose intents judged with `--judge-limit N`.

## Relationship to `eval.rs`

The harness is breadth; each tool's `tests/eval.rs` is the durable per-tool
guardrail. When an intent here catches a real regression worth permanent
protection (e.g. the lxdockercmd "update images → prune" bug), promote it by hand
into that tool's `eval.rs` as a `#[ignore = "eval"]` test with the same intent and
Rust assertions. No automation does this — it is a deliberate curation step.
