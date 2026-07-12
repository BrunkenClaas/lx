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

1. `cargo fmt --all`
2. `cargo clippy --workspace --all-targets -- -D warnings` — must be clean
3. `cargo test --workspace` — must pass (eval tests with `#[ignore]` are fine)
4. Update `CHANGELOG.md` for any user-visible change
5. One tool per PR; one PR per tool

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

Tag with `suite-vX.Y.Z` and push. GitHub Actions (`release-coreutils.yml`) builds
the entire workspace, assembles one ZIP per target, and creates a GitHub
Release with all ZIPs and `.sha256` checksums.

```sh
git tag suite-v1.0.0
git push origin suite-v1.0.0
```

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
