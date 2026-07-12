<!--
Thanks for contributing. Keep PRs small: one tool per PR, one PR per tool.
New tools need a proposal issue with maintainer sign-off first (CONTRIBUTING.md).
-->

## What this changes

<!-- one or two sentences. Link the issue it closes, if any: "Closes #123" -->

## Checklist

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean
- [ ] `cargo test --workspace` — passing (`#[ignore]` eval/system tests may stay ignored)
- [ ] `cargo deny check` — passing (if dependencies changed)
- [ ] `CHANGELOG.md` updated for any user-visible change
- [ ] `docs/design_document.md` updated for any architectural, security, config, flag, or catalog change
- [ ] Every commit is DCO-signed (`git commit -s`)

## Notes for the reviewer

<!-- anything non-obvious: a design trade-off, a deliberate deviation, a
follow-up you're leaving for later. Delete if there's nothing to add. -->
