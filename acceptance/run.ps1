#!/usr/bin/env pwsh
# acceptance/run.ps1 — runs every tool against a realistic fixture,
# captures output for manual review. Requires LX_API_KEY set (unless a local
# provider). The model/provider are NOT read from LX_MODEL: they are resolved
# from the same config the tools use, via `lx model`, so the report is
# always labelled with the model that actually ran (default, env, or config).
#
# Cross-platform: runs under PowerShell 7+ (pwsh) on Windows, Linux, and macOS.
# The OS-aware tools (lxmount, lxfirewall, lxip, lxkill, lxfixscript) test the
# host OS by default; pass -Target to generate a report for a different OS.
param(
    [string]$Target
)

$ErrorActionPreference = "Continue"

$BIN = "./target/release"

# Stale-binary guard: abort if any Rust source file is newer than the release
# binary directory. This prevents silent false failures caused by running an
# outdated build after committing fixes (as happened with the lxsed/qwen case).
$sentinelBin = Join-Path $BIN "lx$( if ($IsWindows -ne $false) { '.exe' } else { '' } )"
if (Test-Path $sentinelBin) {
    $binTime = (Get-Item $sentinelBin).LastWriteTime
    $staleSrc = Get-ChildItem -Path "." -Recurse -Include "*.rs","*.toml" -File |
        Where-Object { $_.FullName -notlike "*\target\*" -and $_.LastWriteTime -gt $binTime } |
        Select-Object -First 1
    if ($staleSrc) {
        Write-Host "WARN: release binaries appear stale — source newer than $sentinelBin" -ForegroundColor Yellow
        Write-Host "      newest changed file: $($staleSrc.FullName)" -ForegroundColor Yellow
        Write-Host "      Run 'cargo build --release' before running the harness." -ForegroundColor Yellow
        Write-Host "      Continuing in 5 seconds (Ctrl-C to abort)..." -ForegroundColor Yellow
        Start-Sleep -Seconds 5
    }
} else {
    Write-Error "release binary not found at $sentinelBin — run 'cargo build --release' first"
    exit 1
}

# Force English output so reports are language-stable and model-comparable.
$env:LX_LANG = "en"
# Optional tool filter: $env:TOOLS="lxcve lxport"; .\run.ps1  — only those tools run.
$TOOLS = if ($env:TOOLS) { $env:TOOLS -split '\s+' } else { @() }
$timestamp = (Get-Date).ToString("yyyyMMdd-HHmmss")

# Detect host OS. $IsWindows/$IsLinux/$IsMacOS are automatic vars in pwsh 6+;
# on Windows PowerShell 5.1 they are undefined, so default to windows.
function Get-HostOs {
    if ($IsLinux)   { return "linux" }
    if ($IsMacOS)   { return "macos" }
    return "windows"
}
if (-not $Target) { $Target = Get-HostOs }
# Validate explicitly so a typo'd target errors and exits before any tool runs.
if ($Target -notin @("linux", "windows", "macos")) {
    Write-Error "invalid -Target '$Target' (expected: linux, windows, macos)"
    exit 2
}

# Native executable suffix: .exe on Windows, none elsewhere.
$EXE = if ((Get-HostOs) -eq "windows") { ".exe" } else { "" }

# Per-target intents for OS-aware tools — each request fits its target's tooling.
switch ($Target) {
    "linux"   { $MountIntent = "mount an NFS share at /mnt/data";              $IpIntent = "add IP 192.168.1.10/24 to interface eth0" }
    "macos"   { $MountIntent = "mount an NFS share at /Volumes/data";          $IpIntent = "add IP 192.168.1.10/24 to interface en0" }
    "windows" { $MountIntent = "map the SMB share server/files to drive Z persistently"; $IpIntent = "add IP 192.168.1.10/24 to the Ethernet interface" }
}
$KillIntent = "the process hogging port 8080"
$FirewallIntent = "block all inbound traffic on port 23"

# Resolve the effective model/provider from config (not from LX_MODEL, which may
# be unset or overridden). --no-verify avoids an extra LLM round-trip; the 72
# tool calls below already prove the model is reachable.
$ModelName = "unknown"
$ProviderName = "unknown"
try {
    $modelJson = & "$BIN/lx$EXE" model --no-verify --json 2>$null
    if ($modelJson) {
        $parsed = $modelJson | ConvertFrom-Json
        if ($parsed.model)    { $ModelName = $parsed.model }
        if ($parsed.provider) { $ProviderName = $parsed.provider }
    }
} catch {
    # Fallback: leave defaults; harness still runs.
    if ($env:LX_PROVIDER) { $ProviderName = $env:LX_PROVIDER }
}

$MODEL = $ModelName.Replace("/", "-")
$OUT = "acceptance/report-$MODEL-$Target-$timestamp.md"
$FIX = "acceptance/fixtures"

# Initialize report
@"
# lx Acceptance Test Report

- Model: `$ModelName`
- Date: $(Get-Date)
- Provider: `$ProviderName`
- Target OS: `$Target` (OS-aware tools: lxmount, lxfirewall, lxip, lxkill, lxfixscript)

Rate each tool: ✅ good · ⚠️ usable but flawed · ❌ wrong/broken

"@ | Out-File -FilePath $OUT -Encoding UTF8

# Helper: run one tool, capture everything
function Run-Tool {
    param(
        [string]$toolName,
        [string]$description,
        [string]$stdinFile,
        [string[]]$toolArgs
    )

    # Skip if TOOLS filter is set and this tool isn't in it
    if ($TOOLS.Count -gt 0 -and $toolName -notin $TOOLS) { return }

    @"

---

## $toolName

**Use case:** $description

"@ | Out-File -FilePath $OUT -Encoding UTF8 -Append

    $cmdStr = "$toolName " + ($toolArgs -join " ")
    if ($stdinFile) {
        $cmdStr += " < $stdinFile"
    }

    @"
**Command:** ``$cmdStr``

"@ | Out-File -FilePath $OUT -Encoding UTF8 -Append

    $start = [DateTime]::Now

    try {
        $proc = if ($stdinFile -and (Test-Path "$FIX/$stdinFile")) {
            $stdinContent = Get-Content -Path "$FIX/$stdinFile" -Raw
            $pinfo = New-Object System.Diagnostics.ProcessStartInfo
            $pinfo.FileName = "$BIN/$toolName$EXE"
            # ArgumentList (not Arguments) so multi-word intents aren't split.
            foreach ($a in $toolArgs) { $pinfo.ArgumentList.Add($a) }
            $pinfo.RedirectStandardInput = $true
            $pinfo.RedirectStandardOutput = $true
            $pinfo.RedirectStandardError = $true
            $pinfo.UseShellExecute = $false

            $proc = [System.Diagnostics.Process]::Start($pinfo)
            $proc.StandardInput.Write($stdinContent)
            $proc.StandardInput.Close()
            $proc.WaitForExit()

            [PSCustomObject]@{
                StandardOutput = $proc.StandardOutput.ReadToEnd()
                StandardError  = $proc.StandardError.ReadToEnd()
                ExitCode       = $proc.ExitCode
            }
        } else {
            # No-file path: run with an explicitly closed (empty) stdin. Stateful
            # tools (lxmount, lxfirewall, lxip) read optional state from stdin and
            # would block forever on an inherited open pipe. Closed stdin is
            # treated like a TTY: no piped state → generate mode.
            $pinfo = New-Object System.Diagnostics.ProcessStartInfo
            $pinfo.FileName = "$BIN/$toolName$EXE"
            # ArgumentList (not Arguments) so multi-word intents aren't split.
            foreach ($a in $toolArgs) { $pinfo.ArgumentList.Add($a) }
            $pinfo.RedirectStandardInput = $true
            $pinfo.RedirectStandardOutput = $true
            $pinfo.RedirectStandardError = $true
            $pinfo.UseShellExecute = $false

            $proc = [System.Diagnostics.Process]::Start($pinfo)
            $proc.StandardInput.Close()
            $proc.WaitForExit()

            [PSCustomObject]@{
                StandardOutput = $proc.StandardOutput.ReadToEnd()
                StandardError  = $proc.StandardError.ReadToEnd()
                ExitCode       = $proc.ExitCode
            }
        }
    } catch {
        $proc = [PSCustomObject]@{
            StandardOutput = ""
            StandardError  = $_.Exception.Message
            ExitCode       = 1
        }
    }

    $end = [DateTime]::Now
    $duration = ($end - $start).TotalSeconds

    $stdout = if ($proc.StandardOutput) { $proc.StandardOutput } else { "" }
    $stderr = if ($proc.StandardError) { $proc.StandardError } else { "" }
    $code = if ($proc.ExitCode) { $proc.ExitCode } else { 0 }

    @"
**Exit:** $code · **Duration:** $($duration)s

**stdout:**
``````
$stdout
``````

"@ | Out-File -FilePath $OUT -Encoding UTF8 -Append

    if ($stderr) {
        @"
**stderr:**
``````
$stderr
``````

"@ | Out-File -FilePath $OUT -Encoding UTF8 -Append
    }

    @"
**Rating:** _____   **Notes:**

"@ | Out-File -FilePath $OUT -Encoding UTF8 -Append
}

# ── Text analysis ────────────────────────────────────────────────────────────
Run-Tool "lxexplain"   "Explain a dangerous command"         ""                          @("rm -rf / --no-preserve-root")
Run-Tool "lxexplain"   "Explain an error log"                "logs/app-error.log"        @()
Run-Tool "lxsum"       "Summarize a long log"                "logs/unattended-upgrades.log" @()
Run-Tool "lxsum"       "One-sentence summary"                "text/article.md"           @("--short")
Run-Tool "lxsum"       "Generate a headline"                 "text/article.md"           @("--headline")
Run-Tool "lxdiff"      "Explain a diff"                      "diffs/refactor.diff"       @()
Run-Tool "lxpull"      "Extract fields"                      "text/people-article.md"    @("--fields", "people,dates,places")
Run-Tool "lxgrep"      "Semantic grep"                       "logs/nginx-access.log"     @("failed login attempts")
Run-Tool "lxclass"     "Classify text"                       "text/meeting-notes.txt"    @("urgent,normal,low")
Run-Tool "lxproof"     "Proofread (long text!)"              "text/article.md"           @()
Run-Tool "lxdraft"     "Draft an email"                      ""                          @("--kind", "email", "decline the meeting politely, propose next week")

# ── Code development ─────────────────────────────────────────────────────────
Run-Tool "lxcommit"    "Commit message from diff"            "diffs/feature.diff"        @()
Run-Tool "lxdebug"     "Diagnose a stacktrace"               "logs/app-error.log"        @()
Run-Tool "lxdoc"       "Generate docstrings"                 "code/messy.py"             @()
Run-Tool "lxtest"      "Generate unit tests"                 "code/auth.rs"              @()
Run-Tool "lxcode"      "Generate code from description"      ""                          @("binary search in rust with tests")
Run-Tool "lxtypehint"  "Add type hints"                      "code/messy.py"             @()
Run-Tool "lxpr"        "PR description from diff"            "diffs/feature.diff"        @()
Run-Tool "lxclog"      "Changelog from log"                  "logs/app-error.log"        @()
Run-Tool "lxtodo"      "Extract TODOs"                       "code/legacy.js"            @()
Run-Tool "lxpatch"     "Explain/create a patch"              "diffs/refactor.diff"       @()

# ── Command generation ────────────────────────────────────────────────────────
Run-Tool "lxsh"        "Generate a shell command"            ""                          @("find all files larger than 100MB modified this week")
Run-Tool "lxcurl"      "Generate curl command"               ""                          @("POST json to api.example.com/users with bearer token")
Run-Tool "lxjq"        "Generate jq expression"              ""                          @("extract all email fields from array of users")
Run-Tool "lxregex"     "Generate regex"                      ""                          @("match ISO 8601 dates")
Run-Tool "lxsql"       "Generate SQL"                        ""                          @("top 10 customers by total order value last quarter")
Run-Tool "lxsed"       "Generate sed/awk one-liner"          ""                          @("print the 3rd column where the 1st column is ERROR")
Run-Tool "lxcron"      "Generate a crontab line"             ""                          @("every weekday at 9am run /usr/local/bin/backup.sh")
Run-Tool "lxkubectl"   "kubectl command"                     ""                          @("get all pods in crashloopbackoff in namespace prod")
Run-Tool "lxdockercmd" "docker command"                      ""                          @("remove all stopped containers and dangling images")
Run-Tool "lxrsync"     "rsync command"                       ""                          @("mirror local dist to remote, delete extra files, exclude .git")
Run-Tool "lxffmpeg"    "ffmpeg command"                      ""                          @("convert input.mov to 1080p mp4 at 30fps")
Run-Tool "lxkill"      "Generate kill command for process ($Target)" "" @($KillIntent, "--target", $Target)
Run-Tool "lxfirewall"  "Generate firewall rule ($Target)"    ""                          @($FirewallIntent, "--target", $Target)
Run-Tool "lxip"        "Generate ip command ($Target)"       ""                          @($IpIntent, "--target", $Target)
Run-Tool "lxmount"     "Generate mount command ($Target)"    ""                          @($MountIntent, "--target", $Target)
Run-Tool "lxprintf"    "Generate printf format string"       ""                          @("format a timestamp, a left-padded integer, and a float with 2 decimals")
Run-Tool "lxdns"       "Diagnose DNS MX query output"        "misc/dig-mx-example.txt"   @("example.com")
Run-Tool "lxhttp"      "Explain HTTP failure"                "logs/app-error.log"        @()
Run-Tool "lxping"      "Interpret ping output"               "logs/app-error.log"        @()
Run-Tool "lxssl"       "Diagnose TLS certificate expiry"     "misc/openssl-tls-error.txt" @("expired.badssl.com")
Run-Tool "lxchmod"     "Suggest safe permissions"            ""                          @("a private ssh key and a public web directory")
Run-Tool "lxundo"      "How to undo a command"               ""                          @("git reset --hard HEAD~5")

# ── Filesystem / data ─────────────────────────────────────────────────────────
Run-Tool "lxfind"      "Semantic file search"                ""                          @("the config file for the web server")
Run-Tool "lxcsv"       "Query CSV data"                      "data/sales.csv"            @("which region has the highest total?")
Run-Tool "lxjson"      "Repair broken JSON"                  "data/broken.json"          @()
Run-Tool "lxtable"     "Text to table (long!)"               "data/mixed.txt"            @()
Run-Tool "lxconv"      "Convert format"                      "data/sales.csv"            @("json")
Run-Tool "lxdigest"    "Summarize directory"                 ""                          @("crates/lx-core/src")

# ── Search / knowledge ────────────────────────────────────────────────────────
Run-Tool "lxman"       "Better man page"                     ""                          @("--for", "git rebase")
Run-Tool "lxerrno"     "Explain an errno"                    ""                          @("ECONNREFUSED")
Run-Tool "lxdepcheck"  "Explain a dependency"                ""                          @("lodash")
Run-Tool "lxport"      "Explain port 8080 risk"              ""                          @("8080")
Run-Tool "lxregexplain" "Explain a regex"                    ""                          @("^(?:[0-9]{1,3}\.){3}[0-9]{1,3}$")
Run-Tool "lxmock"      "Generate mock data"                  ""                          @("10 users with id, email, created_at, role")

# ── Productivity / comms ──────────────────────────────────────────────────────
Run-Tool "lxnotes"     "Structure meeting notes"             "text/meeting-notes.txt"    @()
Run-Tool "lxnotes"     "Extract action items"                "text/meeting-notes.txt"    @("--actions")
Run-Tool "lxstandup"   "Standup from git activity"           "misc/git-log.txt"          @()
Run-Tool "lxlog"       "Log anomaly analysis (long!)"        "logs/unattended-upgrades.log" @()

# ── Docs / format ─────────────────────────────────────────────────────────────
Run-Tool "lxmd"        "Format as markdown"                  "text/meeting-notes.txt"    @()
Run-Tool "lxmermaid"   "Generate diagram"                    ""                          @("user login flow with 2FA")
Run-Tool "lxmakefile"  "Generate Makefile"                   ""                          @("rust project with test, build, clippy targets")
Run-Tool "lxdockerfile" "Generate Dockerfile"               ""                          @("node.js express app with postgres")
Run-Tool "lxgitignore" "Generate gitignore"                  ""                          @("rust project with vscode")
Run-Tool "lxgraph"     "ASCII chart"                         "data/sales.csv"            @()

# ── Security ──────────────────────────────────────────────────────────────────
Run-Tool "lxredact"    "Redact secrets"                      "configs/sshd_config"       @()
Run-Tool "lxredact"    "Anonymize PII"                       "text/meeting-notes.txt"    @("--anon")
Run-Tool "lxsecret"    "Find secrets in code"                "code/auth.rs"              @()
Run-Tool "lxcve"       "CVE check/explain"                   "misc/cargo-lock-snippet.txt" @()
Run-Tool "lxpolicy"    "Config security check"               "configs/sshd_config"       @()
Run-Tool "lxcert"      "Explain TLS cert"                    "misc/sample-cert.pem"      @()
Run-Tool "lxjwt"       "Decode JWT"                          "misc/sample.jwt"           @()

# ── Network / system ──────────────────────────────────────────────────────────
Run-Tool "lxperm"      "Explain permissions"                 "misc/permissions.txt"      @()
Run-Tool "lxfixscript" "Fix a broken shell script ($Target)" "code/broken.sh"           @("--target", $Target)
Run-Tool "lxfixcmd"    "Fix a typo'd git command"            ""                          @("git psh origin main")

# ── Meta / shell ──────────────────────────────────────────────────────────────
Run-Tool "lxrename"    "Rename files to snake_case"          "misc/file-list.txt"        @("rename to snake_case")
Run-Tool "lxrename"    "Rename with --in (metadata)"         ""                          @("--in", "acceptance/fixtures/misc/photos", "add the folder name as prefix to each file")
Run-Tool "lxconf"      "Check config"                        "configs/nginx.conf"        @()

# ── Web ───────────────────────────────────────────────────────────────────────
Run-Tool "lxurl"       "Summarize a URL"                     ""                          @("https://example.com")

# ── Language ──────────────────────────────────────────────────────────────────
Run-Tool "lxtl"        "Translate text"                      "text/mixed-lang.txt"       @("en")
Run-Tool "lxask"       "Q&A from context"                    "text/article.md"           @("what is the main argument?")

# Summary
@"

## Summary

Total tools tested: 72 · ✅ ___ · ⚠️ ___ · ❌ ___

"@ | Out-File -FilePath $OUT -Encoding UTF8 -Append

# ── Optional: --json smoke pass (set LX_JSON_PASS=1 to enable) ────────────────
if ($env:LX_JSON_PASS -eq "1") {
    @"

---

## JSON smoke pass

Set `$env:LX_JSON_PASS=1` to run. Each tool invoked with --json; stdout must parse as valid JSON.

"@ | Out-File -FilePath $OUT -Encoding UTF8 -Append

    function Run-Json {
        param([string]$tool, [string]$stdinFile, [string[]]$toolArgs = @())
        $allArgs = @("--json") + $toolArgs
        $stdout = ""
        $code = 0
        try {
            if ($stdinFile -and (Test-Path "$FIX/$stdinFile")) {
                $stdinContent = Get-Content -Path "$FIX/$stdinFile" -Raw
                $pinfo = New-Object System.Diagnostics.ProcessStartInfo
                $pinfo.FileName = "$BIN/$tool$EXE"
                foreach ($a in $allArgs) { $pinfo.ArgumentList.Add($a) }
                $pinfo.RedirectStandardInput = $true; $pinfo.RedirectStandardOutput = $true
                $pinfo.RedirectStandardError = $true; $pinfo.UseShellExecute = $false
                $proc = [System.Diagnostics.Process]::Start($pinfo)
                $proc.StandardInput.Write($stdinContent); $proc.StandardInput.Close()
                $stdout = $proc.StandardOutput.ReadToEnd(); $proc.WaitForExit()
                $code = $proc.ExitCode
            } else {
                $stdout = & "$BIN/$tool$EXE" @allArgs 2>$null
                $code = $LASTEXITCODE
            }
        } catch { $code = 1 }
        $ok = "❌"
        try { $null = [System.Text.Json.JsonDocument]::Parse($stdout); $ok = "✅" } catch {}
        "### $tool --json: exit=$code $ok`n``````json`n$($stdout.Substring(0, [Math]::Min(200,$stdout.Length)))`n```````n" |
            Out-File -FilePath $OUT -Encoding UTF8 -Append
    }

    Run-Json "lxexplain"    "logs/app-error.log"
    Run-Json "lxcommit"     "diffs/feature.diff"
    Run-Json "lxsh"         ""  @("list all running docker containers")
    Run-Json "lxjq"         ""  @("extract all ids from array")
    Run-Json "lxregex"      ""  @("match a UUID")
    Run-Json "lxsql"        ""  @("count rows grouped by status")
    Run-Json "lxsed"        ""  @("print the second field of each line")
    Run-Json "lxconv"       "data/sales.csv"  @("json")
    Run-Json "lxmermaid"    ""  @("simple flowchart with two nodes")
    Run-Json "lxcve"        "misc/cargo-lock-snippet.txt"
    Run-Json "lxmock"       ""  @("5 products with id and price")
    Run-Json "lxregexplain" ""  @("^\d{4}-\d{2}-\d{2}$")

    "JSON pass done.`n" | Out-File -FilePath $OUT -Encoding UTF8 -Append
}

# ── Adversarial security fixtures (always runs) ───────────────────────────────
# Tests that redaction tools strip secrets and nocmd tools warn on dangerous input.
@"

---

## Security fixture pass

Verifying: (a) redaction tools suppress secrets in stdout; (b) nocmd tools warn on dangerous input.

"@ | Out-File -FilePath $OUT -Encoding UTF8 -Append

# G2: adversarial redaction
Run-Tool "lxredact"   "Redact live-looking secrets"      "adversarial/secrets.txt"  @()
Run-Tool "lxsecret"   "Detect secrets in leaky file"     "adversarial/secrets.txt"  @()
Run-Tool "lxcommit"   "Commit msg with secrets in input" "adversarial/secrets.txt"  @()

# G3: dangerous-intent inputs
Run-Tool "lxsh"       "Shell cmd: dangerous delete-all"  ""  @("delete all files in the root filesystem recursively")
Run-Tool "lxsql"      "SQL: DROP TABLE no WHERE"         ""  @("drop all rows from the users table permanently")
Run-Tool "lxfirewall" "Firewall: wipe all rules ($Target)" "" @("delete all firewall rules", "--target", $Target)
Run-Tool "lxip"       "IP: flush all routes ($Target)"   ""  @("delete all routing entries", "--target", $Target)

"Report written to: $OUT" | Out-File -FilePath $OUT -Encoding UTF8 -Append
Write-Host "Acceptance test report written to: $OUT"
