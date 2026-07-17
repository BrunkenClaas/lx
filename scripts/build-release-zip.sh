#!/usr/bin/env bash
# Builds a local release ZIP of the full LX Coreutils suite for the current host platform.
#
# Usage:
#   ./scripts/build-release-zip.sh [VERSION]
#
# VERSION defaults to "dev".

set -euo pipefail

VERSION="${1:-dev}"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Detect host target triple
TARGET="$(rustc -vV | awk '/^host:/ { print $2 }')"

echo "Building LX Coreutils $VERSION for $TARGET ..."
cargo build --workspace --release

ZIPNAME="lx-coreutils-${VERSION}-${TARGET}"
STAGING="dist/${ZIPNAME}"

rm -rf "$STAGING"
mkdir -p "$STAGING/shell-integration"

# Binaries
find target/release -maxdepth 1 -name 'lx*' -type f -executable \
    ! -name 'lx-acceptance' | while read -r bin; do
    cp "$bin" "$STAGING/"
done

# Docs
cp README.md CHANGELOG.md LICENSE-APACHE LICENSE-MIT "$STAGING/"
cp crates/lx-config/config.example.toml "$STAGING/"
cp shell-integration/lx.bash \
   shell-integration/lx.zsh \
   shell-integration/lx.fish \
   shell-integration/lx.ps1 \
   shell-integration/README.md \
   "$STAGING/shell-integration/"

# ZIP
ZIPPATH="dist/${ZIPNAME}.zip"
rm -f "$ZIPPATH"
if command -v zip &>/dev/null; then
    cd dist && zip -r "${ZIPNAME}.zip" "${ZIPNAME}" && cd "$REPO_ROOT"
elif command -v 7z &>/dev/null; then
    cd dist && 7z a "${ZIPNAME}.zip" "${ZIPNAME}" && cd "$REPO_ROOT"
else
    # Fallback: PowerShell (available on Windows/Git Bash)
    powershell.exe -NoProfile -Command \
        "Compress-Archive -Path 'dist/${ZIPNAME}' -DestinationPath '${ZIPPATH}' -Force"
fi

# Checksum
if command -v sha256sum &>/dev/null; then
    sha256sum "${ZIPPATH}" > "${ZIPPATH}.sha256"
elif command -v shasum &>/dev/null; then
    shasum -a 256 "${ZIPPATH}" > "${ZIPPATH}.sha256"
else
    powershell.exe -NoProfile -Command \
        "(Get-FileHash '${ZIPPATH}' -Algorithm SHA256).Hash.ToLower() + '  ${ZIPNAME}.zip'" \
        | Set-Content "${ZIPPATH}.sha256"
fi

echo ""
echo "Done: dist/${ZIPNAME}.zip"
echo "SHA256: $(cut -d' ' -f1 "dist/${ZIPNAME}.zip.sha256")"
