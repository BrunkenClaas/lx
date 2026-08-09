# Contributing to LX Coreutils

This suite was designed and written to a hand-authored specification
([`docs/design_document.md`](docs/design_document.md)), which is authoritative on
architecture, security rules, and every tool's contract. AI was used as a tool
in the implementation, directed against that spec. Contributions follow the same
model: match the spec, use an existing tool as a style reference, keep it green.

## Language

All code, comments, commit messages, and documentation must be in **English**.

## DCO — Developer Certificate of Origin

This project uses DCO instead of a CLA. Sign every commit with:

```
Signed-off-by: Your Name <your@email.com>
```

Add it automatically with `git commit -s`.

## Development environment

This repo pins an **exact** Rust toolchain (`rust-toolchain.toml`), so a slightly
different local `rustc` will produce different `rustfmt`/`clippy` output and can
turn CI red on an otherwise-fine PR. To avoid that, a dev container is provided:

- **One click:** open the repo in GitHub Codespaces, or "Reopen in Container" in
  VS Code. It builds from [`.devcontainer/`](.devcontainer/) with the pinned
  toolchain and every CI tool (`clippy`, `rustfmt`, `cargo-deny`, musl target)
  preinstalled.
- **Manual:** install the toolchain in `rust-toolchain.toml` (rustup reads it
  automatically) plus `cargo-deny` and the `x86_64-unknown-linux-musl` target.

## Opening a PR

The authoritative checklist is
[`.github/PULL_REQUEST_TEMPLATE.md`](.github/PULL_REQUEST_TEMPLATE.md) — it is
filled in automatically when you open a PR, and it is the list CI enforces. Work
through it rather than a copy here; this file deliberately does not duplicate it,
because a duplicated checklist is one that drifts.

Before you open the PR:

```sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                  # #[ignore] eval/system tests may stay ignored
cargo deny check                        # only if dependencies changed
cargo build --release -p lx<name>       # the binary users actually run
```

Note the scope: these are **workspace-wide** (`--all` / `--workspace`). The
per-tool commands in §14 of the design document (`-p lx<name>`) are for finishing
a single tool, not for clearing a PR — a change can be clean in its own crate and
still break another.

Then:

- **One tool per PR; one PR per tool.** Keep unrelated changes out, even small ones.
- **Update `CHANGELOG.md`** for any user-visible change (see the table below).
- **Update `docs/design_document.md`** in the *same* PR for any architectural,
  security, config, flag, or catalog change — plus an Appendix A row.

### Which document to update when

`CHANGELOG.md` records what **users** experience. `docs/design_document.md`
records what **maintainers** must know. Appendix A records that the design
document moved. "Last reviewed" records when someone checked the document still
matches the code.

| Your change | CHANGELOG | Design doc | Appendix A row | "Last reviewed" |
|---|---|---|---|---|
| User-visible behaviour (message, flag, output, exit code) | yes | only if architectural | — | — |
| Architecture / security / config / flag / catalog | yes | yes | yes, same commit | yes |
| New tool | yes | yes (§13 catalog) | yes | yes |
| Docs-only clarification | if user-visible | yes | yes | judgement |
| Internal refactor, no user-visible change | no | no | — | — |
| Toolchain / dependency bump | yes | Appendix A only | yes | no |

"Last reviewed" is **not** "last edited" — bump it when you have verified the
document still describes the code, not for a typo fix.

### After your PR is merged

The head branch is deleted automatically on GitHub. Clean up locally:

```sh
git switch main
git pull
git branch -d <your-branch>    # lowercase -d refuses if unmerged — keep that safety
git fetch --prune
```

Always cut the next branch from a freshly pulled `main`. Branches cut in parallel
from an older `main` collide in `CHANGELOG.md`, since every PR inserts under the
same `## [Unreleased]` heading.

## Adding a new tool

1. Open an issue with: tool name, purpose, and input/output contract.
2. Get maintainer approval in the issue.
3. PR: implement following the architecture and per-tool contracts in
   [`docs/design_document.md`](docs/design_document.md) (§13 is the tool
   catalog; §10 the security flags). Use the closest existing tool as a style
   reference — `tools/lxexplain/` (simplest), `tools/lxcommit/` (mandatory
   redaction), `tools/lxsh/` (command generation with danger detection).
4. Maintainer review; merge on green CI.

## Releasing

### Single-tool release

Tag with `lx<tool>-vX.Y.Z` and push. GitHub Actions (`release.yml`) builds
the tool for all three targets and creates a GitHub Release automatically.

```sh
git tag lxcommit-v1.2.0
git push origin lxcommit-v1.2.0
```

### Full suite release

All 79 crates share one version, bumped in lockstep. Between releases `main` carries
a `-dev` suffix (e.g. `1.0.6-dev`) so a self-built `main` binary's `--version` is
distinguishable from a real release. The ritual, in strict order:

1. On a `release/X.Y.Z` branch: bump every crate `X.Y.Z-dev` → plain `X.Y.Z`,
   regenerate `Cargo.lock` (`cargo build`), move `CHANGELOG.md` `[Unreleased]` →
   `[X.Y.Z] - <date>`, and update the version refs in `README.md` and
   `scripts/install.{sh,ps1}`. Open a PR, label it `no-release-note` (so the
   next release's auto-generated notes don't list this version bump — see
   `.github/release.yml`), let full CI pass, merge.
2. Get onto the merged commit, clean up, and tag it — never before merge, so the
   release builds from a CI-verified, plain-version commit. Chain the steps so a
   failure stops the run instead of letting the tag land on the wrong commit:
   ```sh
   git checkout main && git pull && \
     git branch -d release/X.Y.Z && git fetch --prune && \
     git tag suite-vX.Y.Z && git push origin suite-vX.Y.Z
   ```
   GitHub Actions (`release-coreutils.yml`) builds the entire workspace, assembles
   one ZIP per target, and creates a GitHub Release with all ZIPs and `.sha256`
   checksums.
3. **Final step — the release isn't finished without it.** Bump `main` to the next
   `-dev` (e.g. `1.0.7-dev`) and commit it **straight to `main`, no PR**:
   ```sh
   # bump all 79 Cargo.toml, then
   cargo build                     # regenerate Cargo.lock
   git commit -sam "chore: mark main as X.Y.Z-dev between releases"
   git push
   ```
   This is the one carve-out from "code goes via PR". It must land immediately
   after tagging, before any other commit reaches `main`, and it must be
   version-only — the 79 `Cargo.toml` plus `Cargo.lock`, nothing else. Any other
   edit in the commit voids the carve-out and it goes via PR. CI still runs on the
   push, so mistakes are caught, just after the fact rather than before.
   `README.md`/install scripts stay at the just-released version.

`LX_SUITE_LABEL` (`YYYY-MM`, in `lx-core/src/version.rs`) marks the suite generation,
not the release — bump it only on a minor/major, not on patches.

### Local suite ZIP (for testing before tagging)

```sh
# Linux / macOS
./scripts/build-release-zip.sh 1.0.0

# Windows (PowerShell 7+)
.\scripts\build-release-zip.ps1 -Version 1.0.0

# Windows (CMD)
scripts\build-release-zip.bat 1.0.0
```

The ZIP lands in `dist/` and contains all binaries plus the user-facing
documents (`README.md`, `CHANGELOG.md`, licences, `config.example.toml`,
`shell-integration/`).

## Deprecation policy

- Tool deprecation: 2 minor-version advance notice via stderr warning, then removal in next major.
- Breaking library API change: semver Major bump + `CHANGELOG.md` entry.

## Code style

- `cargo fmt` is mandatory (enforced by CI).
- `clippy -- -D warnings` must be clean.
- Comments explain **why**, not what.
- `unwrap()`/`expect()` only in tests, with a reason string.
- No `println!` in library code.
- `CHANGELOG.md` in [Keep-a-Changelog](https://keepachangelog.com) format.

## Toolchain & dependency policy

This project is built to be maintainable for ~20 years. The guiding rule:

> **Anything that determines a reproducible build or a CI pass/fail is pinned to
> an exact version. Every upgrade is a deliberate, dated, reviewed commit — never
> ambient drift.** Manifests express *intent* (version ranges); lockfiles and the
> toolchain express *reproducibility* (exact versions).

Concretely, by layer:

| Layer | File | Policy |
|-------|------|--------|
| Rust toolchain | `rust-toolchain.toml` | **Exact** version (`channel = "1.95.0"`), not `"stable"`. `"stable"` rolls forward on any `rustup update` and silently changes rustfmt/clippy output — it caused a 70-file reformat once. |
| CI toolchain | `.github/workflows/*.yml` | `dtolnay/rust-toolchain@<exact-version>` matching `rust-toolchain.toml`. The action does **not** read `rust-toolchain.toml`, so the version is duplicated here on purpose — keep the two in lock-step. |
| Direct deps | `Cargo.toml` | Caret ranges with a **lower bound = the minor actually tested** (e.g. `clap = "4.6"`). Never `=exact` in the manifest (it fights the lockfile and makes security bumps painful). |
| Locked deps | `Cargo.lock` | **Committed.** Pins exact transitive versions. The source of build reproducibility for dependencies. |
| GitHub Actions | `.github/workflows/*.yml` | Major tags (`@v4`). Auto-receives security patches within the major; revisit if supply-chain hardening (SHA pinning) is ever required. |

### Upgrade ritual (do this deliberately, ~quarterly or when a security fix needs it)

**Rust toolchain bump:**
1. Pick the new exact version (`rustc --version` after `rustup update`, or a chosen release).
2. Edit `rust-toolchain.toml` (`channel = "X.Y.Z"`) **and** every
   `dtolnay/rust-toolchain@X.Y.Z` ref in `.github/workflows/*.yml` — they must match.
3. `cargo fmt --all` (new rustfmt may reflow — commit that **separately** as a
   `style:` commit so logical diffs stay clean).
4. `cargo clippy --workspace --all-targets -- -D warnings` and fix any new lints.
5. `cargo test --workspace`.
6. Commit as `chore: bump Rust toolchain to X.Y.Z` + `CHANGELOG.md` entry +
   an Appendix A row in `docs/design_document.md`.

**Dependency bump:**
1. `cargo update -p <crate>` (single crate) or review `cargo update` output.
2. If you now rely on a newer minor, raise its lower bound in `Cargo.toml`.
3. `cargo deny check` (licenses/advisories/bans/sources), `clippy`, `test`.
4. Commit the `Cargo.lock` change with a `chore(deps):` message and the reason.

**Never** run `rustup update` or loosen a pin as a side effect of unrelated work —
that is the exact ambient drift this policy exists to prevent.
