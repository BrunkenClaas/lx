#Requires -Version 7
<#
.SYNOPSIS
    Builds a local release ZIP of the full LX Coreutils suite for the current host platform.

.PARAMETER Version
    Version string to embed in the ZIP name, e.g. "1.0.0". Defaults to "dev".

.EXAMPLE
    .\scripts\build-release-zip.ps1
    .\scripts\build-release-zip.ps1 -Version 1.0.0
#>
param(
    [string]$Version = "dev"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path $PSScriptRoot -Parent
Set-Location $RepoRoot

# Detect host target triple
$Target = "x86_64-pc-windows-msvc"
$Ext    = ".exe"

Write-Host "Building LX Coreutils $Version for $Target ..."
cargo build --workspace --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$ZipName = "lx-coreutils-$Version-$Target"
$Staging = Join-Path $RepoRoot "dist" $ZipName

# Clean and recreate staging dir
if (Test-Path $Staging) { Remove-Item $Staging -Recurse -Force }
New-Item -ItemType Directory -Path $Staging | Out-Null
New-Item -ItemType Directory -Path "$Staging\shell-integration" | Out-Null

# Binaries
$excluded = @("lx-acceptance$Ext")
Get-ChildItem "target\release\lx*$Ext" | Where-Object { -not $_.PSIsContainer -and $_.Name -notin $excluded } | ForEach-Object {
    Copy-Item $_.FullName $Staging
}

# Docs
Copy-Item "README.md", "CHANGELOG.md", "LICENSE-APACHE", "LICENSE-MIT" $Staging
Copy-Item "crates\lx-config\config.example.toml" $Staging
Copy-Item "shell-integration\lx.bash",
          "shell-integration\lx.zsh",
          "shell-integration\lx.fish",
          "shell-integration\lx.ps1",
          "shell-integration\README.md" "$Staging\shell-integration\"

# ZIP
$ZipPath = Join-Path $RepoRoot "dist" "$ZipName.zip"
Compress-Archive -Path $Staging -DestinationPath $ZipPath -Force

# Checksum
$Hash = (Get-FileHash $ZipPath -Algorithm SHA256).Hash.ToLower()
"$Hash  $ZipName.zip" | Set-Content (Join-Path $RepoRoot "dist" "$ZipName.zip.sha256")

Write-Host ""
Write-Host "Done: dist\$ZipName.zip"
Write-Host "SHA256: $Hash"
