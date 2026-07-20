<#
.SYNOPSIS
    LX Coreutils installer for Windows — downloads the latest prebuilt release,
    verifies its checksum, and installs the binaries to a bin directory on your PATH.
    No Rust toolchain, no compilation.

.DESCRIPTION
    Usage (from PowerShell):
      irm https://raw.githubusercontent.com/BrunkenClaas/lx/main/scripts/install.ps1 | iex

    Options (as environment variables, set before running):
      $env:LX_INSTALL_DIR   install location (default: %USERPROFILE%\bin)
      $env:LX_VERSION       version to install, e.g. 1.0.2 (default: latest release)
#>

$ErrorActionPreference = 'Stop'

$Repo       = 'BrunkenClaas/lx'
$InstallDir = if ($env:LX_INSTALL_DIR) { $env:LX_INSTALL_DIR } else { Join-Path $env:USERPROFILE 'bin' }

function Write-Info { param($m) Write-Host $m }
function Die        { param($m) Write-Host "error: $m" -ForegroundColor Red; exit 1 }

# ── detect architecture → release target ─────────────────────────────────────
# Only x86_64-pc-windows-gnu is published today.
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    'AMD64' { $target = 'x86_64-pc-windows-gnu' }
    'ARM64' { Die "no prebuilt Windows-on-ARM64 binary is published yet. Build from source, or run the x64 build under emulation." }
    default { Die "unsupported architecture '$arch'." }
}

# ── resolve version ──────────────────────────────────────────────────────────
if ($env:LX_VERSION) {
    $version = $env:LX_VERSION
} else {
    Write-Info "resolving latest release ..."
    try {
        $rel = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" `
                                 -Headers @{ 'User-Agent' = 'lx-installer' }
    } catch {
        Die "could not reach the GitHub API to determine the latest version: $($_.Exception.Message)"
    }
    # Tags are suite-vX.Y.Z; assets are versioned X.Y.Z.
    $version = $rel.tag_name -replace '^suite-v', ''
    if (-not $version) { Die "could not parse the latest release tag." }
}

$zipname = "lx-coreutils-$version-$target"
$base    = "https://github.com/$Repo/releases/download/suite-v$version"

Write-Info "Installing LX Coreutils $version ($target) -> $InstallDir"

# ── download to a temp dir ───────────────────────────────────────────────────
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("lx-install-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    $zipPath = Join-Path $tmp "$zipname.zip"
    $sumPath = Join-Path $tmp "$zipname.zip.sha256"

    Write-Info "downloading $zipname.zip ..."
    try {
        Invoke-WebRequest -Uri "$base/$zipname.zip"        -OutFile $zipPath -UseBasicParsing
        Invoke-WebRequest -Uri "$base/$zipname.zip.sha256" -OutFile $sumPath -UseBasicParsing
    } catch {
        Die "download failed: $($_.Exception.Message)"
    }

    # ── verify checksum ──────────────────────────────────────────────────────
    # The .sha256 asset is "<hash>  <zipname>" (GNU sha256sum format).
    $want = (Get-Content $sumPath -Raw).Trim().Split()[0].ToLower()
    $got  = (Get-FileHash $zipPath -Algorithm SHA256).Hash.ToLower()
    if ($want -ne $got) {
        Die "checksum mismatch - refusing to install.`n  expected: $want`n  got:      $got"
    }
    Write-Info "checksum ok"

    # ── extract ──────────────────────────────────────────────────────────────
    Write-Info "extracting ..."
    Expand-Archive -Path $zipPath -DestinationPath $tmp -Force

    # ZIP extracts to a top-level dir named exactly $zipname, containing lx*.exe
    # plus a shell-integration\ subdir and docs. Install only the .exe files.
    $srcDir = Join-Path $tmp $zipname
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

    $bins = Get-ChildItem -Path $srcDir -Filter 'lx*.exe' -File
    if ($bins.Count -eq 0) { Die "no binaries found in the archive - the release may be malformed." }
    foreach ($b in $bins) {
        Copy-Item -Path $b.FullName -Destination $InstallDir -Force
    }
    Write-Info ""
    Write-Info "installed $($bins.Count) binaries to $InstallDir"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

# ── PATH check ───────────────────────────────────────────────────────────────
# The binaries are useless off-PATH, so surface this first.
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if (($userPath -split ';') -notcontains $InstallDir) {
    Write-Info ""
    Write-Info "note: $InstallDir is not on your PATH. Add it permanently, then restart your terminal:"
    Write-Info "  [Environment]::SetEnvironmentVariable('PATH', `"$InstallDir;`$([Environment]::GetEnvironmentVariable('PATH','User'))`", 'User')"
}

# ── Next steps ───────────────────────────────────────────────────────────────
# With the default (local Ollama) the only thing left is to pull a model — no
# config file is needed, lx works out of the box. 'lx config' is OPTIONAL, only
# for changing the defaults. Keep that framing so the tool reads as "just works".
Write-Info ""
if (Get-Command ollama -ErrorAction SilentlyContinue) {
    Write-Info "Almost ready - Ollama is installed. Pull a model and go:"
    Write-Info "  ollama pull llama3.1:8b"
} else {
    Write-Info "One more step - lx uses a local Ollama model by default:"
    Write-Info "  1. install Ollama:  https://ollama.com"
    Write-Info "  2. pull a model:    ollama pull llama3.1:8b"
}
Write-Info ""
Write-Info "Then try it:"
Write-Info "  lxexplain `"tar -xzf archive.tar.gz`""
Write-Info "  lx                                   # browse all 72 tools (offline)"
Write-Info ""
Write-Info "Want another provider (local or hosted), use another model, or configure"
Write-Info "advanced settings?  ->  run 'lx config'"
