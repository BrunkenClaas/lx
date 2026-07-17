#!/usr/bin/env bash
# One-time devcontainer setup: install the extra tooling the CI pipeline uses,
# so a contributor can reproduce every CI check locally without hunting for it.
#
# The Rust toolchain itself is pinned by rust-toolchain.toml and installed by
# rustup on demand — this script only adds what the base image lacks.
set -euo pipefail

# Trigger rustup to install the exact pinned toolchain + its components
# (rustfmt, clippy) declared in rust-toolchain.toml, so the first real build
# isn't the one that pays for it.
rustup show

# musl target for the release/CI build target.
rustup target add x86_64-unknown-linux-musl
sudo apt-get update -y
sudo apt-get install -y musl-tools

# cargo-deny: the CI licence/advisory/bans gate. `cargo deny check` must pass
# before a PR is ready, so contributors need it available here.
cargo install cargo-deny --locked

echo
echo "Dev environment ready. The CI checks, reproduced locally:"
echo "  cargo fmt --all --check"
echo "  cargo clippy --workspace --all-targets -- -D warnings"
echo "  cargo test --workspace"
echo "  cargo deny check"
