#!/usr/bin/env sh
# LX Coreutils installer — downloads the latest prebuilt release for this
# platform, verifies its checksum, and installs the binaries to a bin directory
# on your PATH. No Rust toolchain, no compilation.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/BrunkenClaas/lx/main/scripts/install.sh | sh
#
# Options (as environment variables):
#   LX_INSTALL_DIR   install location (default: ~/.local/bin)
#   LX_VERSION       version to install, e.g. 1.0.2 (default: latest release)
#
# POSIX sh — no bashisms. Works with dash, busybox ash (Raspberry Pi OS), etc.

set -eu

REPO="BrunkenClaas/lx"
INSTALL_DIR="${LX_INSTALL_DIR:-$HOME/.local/bin}"

# ── helpers ──────────────────────────────────────────────────────────────────
err()  { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }
info() { printf '%s\n' "$*" >&2; }
have() { command -v "$1" >/dev/null 2>&1; }

# ── detect platform → release target triple ──────────────────────────────────
# Must match the targets built by .github/workflows/release-coreutils.yml:
#   x86_64-unknown-linux-musl · aarch64-unknown-linux-musl · x86_64-pc-windows-gnu
detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux) ;;
        Darwin)
            err "no prebuilt macOS binary is published yet. Build from source:
  git clone https://github.com/$REPO && cd lx && cargo build --release --workspace" ;;
        *)
            err "unsupported OS '$os'. On Windows use the PowerShell installer (scripts/install.ps1)." ;;
    esac

    case "$arch" in
        x86_64 | amd64)          echo "x86_64-unknown-linux-musl" ;;
        aarch64 | arm64)         echo "aarch64-unknown-linux-musl" ;;
        armv7l | armv6l | arm)
            err "32-bit ARM ('$arch') is not published. On a 32-bit Raspberry Pi OS,
either switch to 64-bit Raspberry Pi OS, or build from source." ;;
        *)
            err "unsupported architecture '$arch'." ;;
    esac
}

# ── resolve version ──────────────────────────────────────────────────────────
# The releases are tagged suite-vX.Y.Z; assets are versioned X.Y.Z.
latest_version() {
    # Ask the GitHub API for the latest release tag, strip the suite-v prefix.
    # No jq dependency — a small grep/sed does it.
    api="https://api.github.com/repos/$REPO/releases/latest"
    tag="$(download_stdout "$api" \
        | grep '"tag_name"' \
        | head -n1 \
        | sed 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/')"
    [ -n "$tag" ] || err "could not determine the latest release version from the GitHub API."
    echo "${tag#suite-v}"
}

# ── download primitives (curl or wget) ───────────────────────────────────────
download_stdout() { # url → stdout
    if have curl;   then curl -fsSL "$1"
    elif have wget; then wget -qO- "$1"
    else err "need either curl or wget installed."; fi
}
download_file() { # url dest
    if have curl;   then curl -fsSL "$1" -o "$2"
    elif have wget; then wget -qO "$2" "$1"
    else err "need either curl or wget installed."; fi
}

# ── checksum verification ────────────────────────────────────────────────────
verify_sha256() { # file expected_sums_file
    file="$1"; sums="$2"
    if have sha256sum; then
        # The .sha256 asset is "<hash>  <zipname>"; check against our local file.
        want="$(awk '{print $1}' "$sums")"
        got="$(sha256sum "$file" | awk '{print $1}')"
    elif have shasum; then
        want="$(awk '{print $1}' "$sums")"
        got="$(shasum -a 256 "$file" | awk '{print $1}')"
    else
        info "warning: no sha256sum/shasum found — skipping checksum verification."
        return 0
    fi
    [ "$want" = "$got" ] || err "checksum mismatch — refusing to install.
  expected: $want
  got:      $got"
    info "checksum ok"
}

# ── main ─────────────────────────────────────────────────────────────────────
main() {
    have unzip || err "need 'unzip' installed (e.g. apt-get install unzip)."

    target="$(detect_target)"
    version="${LX_VERSION:-$(latest_version)}"
    zipname="lx-coreutils-${version}-${target}"
    base="https://github.com/$REPO/releases/download/suite-v${version}"

    info "Installing LX Coreutils ${version} (${target}) → ${INSTALL_DIR}"

    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT INT TERM

    info "downloading ${zipname}.zip ..."
    download_file "${base}/${zipname}.zip"        "${tmp}/${zipname}.zip"
    download_file "${base}/${zipname}.zip.sha256" "${tmp}/${zipname}.zip.sha256"

    verify_sha256 "${tmp}/${zipname}.zip" "${tmp}/${zipname}.zip.sha256"

    info "extracting ..."
    unzip -q "${tmp}/${zipname}.zip" -d "$tmp"

    mkdir -p "$INSTALL_DIR"
    # The ZIP extracts to a top-level dir named exactly $zipname, containing the
    # lx* binaries plus a shell-integration/ subdir and docs. Install only the
    # executables (lx and lx<tool>), skipping the subdir and doc files.
    count=0
    for f in "${tmp}/${zipname}"/lx*; do
        [ -f "$f" ] || continue          # skip the shell-integration/ directory
        install -m 0755 "$f" "$INSTALL_DIR/" 2>/dev/null || {
            cp "$f" "$INSTALL_DIR/" && chmod 0755 "$INSTALL_DIR/$(basename "$f")"
        }
        count=$((count + 1))
    done
    [ "$count" -gt 0 ] || err "no binaries found in the archive — the release may be malformed."

    info ""
    info "installed ${count} binaries to ${INSTALL_DIR}"

    # PATH check — the binaries are useless off-PATH, so surface this first.
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)  info ""
            info "note: ${INSTALL_DIR} is not on your PATH. Add it, then restart your shell:"
            info "  export PATH=\"${INSTALL_DIR}:\$PATH\"   # add to ~/.bashrc or ~/.profile"
            ;;
    esac

    # Next steps. With the default (local Ollama) the only thing left is to pull a
    # model — no config file is needed, lx works out of the box. `lx config` is
    # OPTIONAL, only for changing the defaults. Keep that framing so the tool reads
    # as "just works", not "set me up".
    info ""
    if have ollama; then
        info "Almost ready — Ollama is installed. Pull a model and go:"
        info "  ollama pull llama3.1:8b"
    else
        info "One more step — lx uses a local Ollama model by default:"
        info "  1. install Ollama:  https://ollama.com"
        info "  2. pull a model:    ollama pull llama3.1:8b"
    fi
    info ""
    info "Then try it:"
    info "  lxexplain \"tar -xzf archive.tar.gz\""
    info "  lx                                   # browse all 72 tools (offline)"
    info ""
    info "Want another provider (local or hosted), use another model, or configure"
    info "advanced settings?  ->  run 'lx config'"
}

main "$@"
