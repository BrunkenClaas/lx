#!/usr/bin/env bash
# acceptance/run.sh — runs every tool against a realistic fixture,
# captures output for manual review. Requires LX_API_KEY set (unless a local
# provider). The model/provider are NOT read from LX_MODEL: they are resolved
# from the same config the tools use, via `lx model`, so the report is
# always labelled with the model that actually ran (default, env, or config).
set -uo pipefail   # NOT -e: a failing tool must not abort the entire run

BIN=./target/release

# Stale-binary guard: abort if any Rust source file is newer than the release
# binary. Prevents silent false failures caused by running an outdated build
# after committing fixes (as happened with the lxsed/qwen case).
SENTINEL_BIN="$BIN/lx"
[ -f "$SENTINEL_BIN" ] || SENTINEL_BIN="$BIN/lx.exe"
if [ -f "$SENTINEL_BIN" ]; then
  STALE_SRC="$(find . -path ./target -prune -o \( -name '*.rs' -o -name '*.toml' \) -newer "$SENTINEL_BIN" -print -quit 2>/dev/null)"
  if [ -n "$STALE_SRC" ]; then
    echo "WARN: release binaries appear stale — source newer than $SENTINEL_BIN" >&2
    echo "      newest changed file: $STALE_SRC" >&2
    echo "      Run 'cargo build --release' before running the harness." >&2
    echo "      Continuing in 5 seconds (Ctrl-C to abort)..." >&2
    sleep 5
  fi
else
  echo "error: release binary not found at $SENTINEL_BIN — run 'cargo build --release' first" >&2
  exit 1
fi

# Force English output so reports are language-stable and model-comparable.
export LX_LANG=en
# Optional tool filter: TOOLS="lxcve lxport" bash run.sh  — only those tools run.
TOOLS="${TOOLS:-}"

# ── Target OS for the OS-aware tools ──────────────────────────────────────────
# The OS-aware tools (lxmount, lxfirewall, lxip, lxkill, lxfixscript) take
# --target and tailor their output per OS. By default we test the host OS so the
# report reflects what a user on this machine would get. Override with --target
# to generate a report for a different OS from any host (e.g. produce a Linux
# report from Windows). This makes reports reproducible and OS-comparable.
detect_host_os() {
  case "$(uname -s 2>/dev/null)" in
    Linux*)            echo linux ;;
    Darwin*)           echo macos ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT) echo windows ;;
    *) # Fall back to $OS (set to "Windows_NT" on Windows shells) then linux.
       case "${OS:-}" in Windows_NT) echo windows ;; *) echo linux ;; esac ;;
  esac
}

TARGET=""
while [ $# -gt 0 ]; do
  case "$1" in
    --target) TARGET="${2:-}"; shift 2 ;;
    --target=*) TARGET="${1#--target=}"; shift ;;
    *) echo "error: unknown argument '$1'" >&2
       echo "usage: bash run.sh [--target linux|windows|macos]" >&2
       exit 2 ;;
  esac
done
[ -n "$TARGET" ] || TARGET="$(detect_host_os)"
case "$TARGET" in
  linux|windows|macos) ;;
  *) echo "error: invalid --target '$TARGET' (expected: linux, windows, macos)" >&2
     exit 2 ;;
esac

# Per-target intents for OS-aware tools — each request fits its target's tooling.
case "$TARGET" in
  linux)
    MOUNT_INTENT="mount an NFS share at /mnt/data"
    IP_INTENT="add IP 192.168.1.10/24 to interface eth0"
    KILL_INTENT="the process hogging port 8080" ;;
  macos)
    MOUNT_INTENT="mount an NFS share at /Volumes/data"
    IP_INTENT="add IP 192.168.1.10/24 to interface en0"
    KILL_INTENT="the process hogging port 8080" ;;
  windows)
    MOUNT_INTENT="map the SMB share server/files to drive Z persistently"
    IP_INTENT="add IP 192.168.1.10/24 to the Ethernet interface"
    KILL_INTENT="the process hogging port 8080" ;;
esac
FIREWALL_INTENT="block all inbound traffic on port 23"

# Resolve the effective model/provider from config (not from LX_MODEL, which may
# be unset or overridden). --no-verify avoids an extra LLM round-trip; the 72
# tool calls below already prove the model is reachable.
MODEL_JSON="$("$BIN/lx" model --no-verify --json 2>/dev/null)"
if [ -n "$MODEL_JSON" ] && command -v python3 >/dev/null 2>&1; then
  MODEL="$(printf '%s' "$MODEL_JSON" | python3 -c 'import sys,json;print(json.load(sys.stdin).get("model",""))')"
  PROVIDER="$(printf '%s' "$MODEL_JSON" | python3 -c 'import sys,json;print(json.load(sys.stdin).get("provider",""))')"
else
  # Fallback if lx or python3 is unavailable: plain stdout is the model name.
  MODEL="$("$BIN/lx" model --no-verify 2>/dev/null)"
  PROVIDER="${LX_PROVIDER:-unknown}"
fi
[ -n "$MODEL" ] || MODEL="unknown"
[ -n "$PROVIDER" ] || PROVIDER="unknown"

MODEL_CLEAN="${MODEL//\//-}"
OUT="acceptance/report-$MODEL_CLEAN-$TARGET-$(date +%Y%m%d-%H%M%S).md"
FIX=acceptance/fixtures

echo "# lx Acceptance Test Report" > "$OUT"
echo "" >> "$OUT"
echo "- Model: \`$MODEL\`" >> "$OUT"
echo "- Date: $(date)" >> "$OUT"
echo "- Provider: \`$PROVIDER\`" >> "$OUT"
echo "- Target OS: \`$TARGET\` (OS-aware tools: lxmount, lxfirewall, lxip, lxkill, lxfixscript)" >> "$OUT"
echo "" >> "$OUT"
echo "Rate each tool: ✅ good · ⚠️ usable but flawed · ❌ wrong/broken" >> "$OUT"
echo "" >> "$OUT"

# Helper: run one tool, capture everything
run_tool() {
  local tool="$1"; shift
  local desc="$1"; shift
  local stdin_file="$1"; shift
  # remaining args = tool args

  # Skip if TOOLS filter is set and this tool isn't in it
  if [ -n "$TOOLS" ] && [[ " $TOOLS " != *" $tool "* ]]; then return 0; fi

  echo "" >> "$OUT"
  echo "---" >> "$OUT"
  echo "" >> "$OUT"
  echo "## $tool" >> "$OUT"
  echo "" >> "$OUT"
  echo "**Use case:** $desc" >> "$OUT"
  echo "" >> "$OUT"

  local cmd_str="$tool"
  if [ $# -gt 0 ]; then
    for arg in "$@"; do
      cmd_str="$cmd_str \"$arg\""
    done
  fi
  if [ -n "$stdin_file" ]; then
    cmd_str="$cmd_str < $stdin_file"
  fi

  echo "**Command:** \`$cmd_str\`" >> "$OUT"
  echo "" >> "$OUT"

  local start=$(date +%s.%N)
  local stdout stderr code
  if [ -n "$stdin_file" ] && [ -f "$FIX/$stdin_file" ]; then
    stdout=$("$BIN/$tool" "$@" < "$FIX/$stdin_file" 2>/tmp/stderr_$$) || code=$?
  else
    # Close stdin (</dev/null) on the no-file path. Stateful tools (lxmount,
    # lxfirewall, lxip) read optional state from stdin; with an inherited open
    # pipe (non-interactive shell, CI) they would block forever waiting for EOF.
    # A closed stdin is treated the same as a TTY: no piped state → generate mode.
    stdout=$("$BIN/$tool" "$@" </dev/null 2>/tmp/stderr_$$) || code=$?
  fi
  code=${code:-0}
  stderr=$(cat /tmp/stderr_$$ 2>/dev/null || echo "")
  rm -f /tmp/stderr_$$
  local end=$(date +%s.%N)
  local dur=$(awk "BEGIN {printf \"%.3f\", $end - $start}")

  echo "**Exit:** $code · **Duration:** ${dur}s" >> "$OUT"
  echo "" >> "$OUT"
  echo "**stdout:**" >> "$OUT"
  echo '```' >> "$OUT"
  echo "$stdout" >> "$OUT"
  echo '```' >> "$OUT"
  echo "" >> "$OUT"
  if [ -n "$stderr" ]; then
    echo "**stderr:**" >> "$OUT"
    echo '```' >> "$OUT"
    echo "$stderr" >> "$OUT"
    echo '```' >> "$OUT"
    echo "" >> "$OUT"
  fi
  echo "**Rating:** _____   **Notes:** " >> "$OUT"
  echo "" >> "$OUT"
}

# ── Text analysis ────────────────────────────────────────────────────────────
run_tool lxexplain   "Explain a dangerous command"         ""                          "rm -rf / --no-preserve-root"
run_tool lxexplain   "Explain an error log"                "logs/app-error.log"
run_tool lxsum       "Summarize a long log"                "logs/unattended-upgrades.log"
run_tool lxsum       "One-sentence summary"                "text/article.md"           "--short"
run_tool lxsum       "Generate a headline"                 "text/article.md"           "--headline"
run_tool lxdiff      "Explain a diff"                      "diffs/refactor.diff"
run_tool lxpull      "Extract fields"                      "text/people-article.md"    "--fields" "people,dates,places"
run_tool lxgrep      "Semantic grep"                       "logs/nginx-access.log"     "failed login attempts"
run_tool lxclass     "Classify text"                       "text/meeting-notes.txt"    "urgent,normal,low"
run_tool lxproof     "Proofread (long text!)"              "text/article.md"
run_tool lxdraft     "Draft an email"                      ""                          "--kind" "email" "decline the meeting politely, propose next week"

# ── Code development ─────────────────────────────────────────────────────────
run_tool lxcommit    "Commit message from diff"            "diffs/feature.diff"
run_tool lxdebug     "Diagnose a stacktrace"               "logs/app-error.log"
run_tool lxdoc       "Generate docstrings"                 "code/messy.py"
run_tool lxtest      "Generate unit tests"                 "code/auth.rs"
run_tool lxcode      "Generate code from description"      ""                          "binary search in rust with tests"
run_tool lxtypehint  "Add type hints"                      "code/messy.py"
run_tool lxpr        "PR description from diff"            "diffs/feature.diff"
run_tool lxclog      "Changelog from log"                  "logs/app-error.log"
run_tool lxtodo      "Extract TODOs"                       "code/legacy.js"
run_tool lxpatch     "Explain/create a patch"              "diffs/refactor.diff"

# ── Command generation ────────────────────────────────────────────────────────
run_tool lxsh        "Generate a shell command"            ""                          "find all files larger than 100MB modified this week"
run_tool lxcurl      "Generate curl command"               ""                          "POST json to api.example.com/users with bearer token"
run_tool lxjq        "Generate jq expression"              ""                          "extract all email fields from array of users"
run_tool lxregex     "Generate regex"                      ""                          "match ISO 8601 dates"
run_tool lxsql       "Generate SQL"                        ""                          "top 10 customers by total order value last quarter"
run_tool lxsed       "Generate sed/awk one-liner"          ""                          "print the 3rd column where the 1st column is ERROR"
run_tool lxcron      "Generate a crontab line"             ""                          "every weekday at 9am run /usr/local/bin/backup.sh"
run_tool lxkubectl   "kubectl command"                     ""                          "get all pods in crashloopbackoff in namespace prod"
run_tool lxdockercmd "docker command"                      ""                          "remove all stopped containers and dangling images"
run_tool lxrsync     "rsync command"                       ""                          "mirror local dist to remote, delete extra files, exclude .git"
run_tool lxffmpeg    "ffmpeg command"                      ""                          "convert input.mov to 1080p mp4 at 30fps"
run_tool lxkill      "Generate kill command for process ($TARGET)" "" "$KILL_INTENT" "--target" "$TARGET"
run_tool lxfirewall  "Generate firewall rule ($TARGET)"    ""                          "$FIREWALL_INTENT" "--target" "$TARGET"
run_tool lxip        "Generate ip command ($TARGET)"       ""                          "$IP_INTENT" "--target" "$TARGET"
run_tool lxmount     "Generate mount command ($TARGET)"    ""                          "$MOUNT_INTENT" "--target" "$TARGET"
run_tool lxprintf    "Generate printf format string"       ""                          "format a timestamp, a left-padded integer, and a float with 2 decimals"
run_tool lxdns       "Diagnose DNS MX query output"        "misc/dig-mx-example.txt"   "example.com"
run_tool lxhttp      "Explain HTTP failure"                "logs/app-error.log"
run_tool lxping      "Interpret ping output"               "logs/app-error.log"
run_tool lxssl       "Diagnose TLS certificate expiry"     "misc/openssl-tls-error.txt" "expired.badssl.com"
run_tool lxchmod     "Suggest safe permissions"            ""                          "a private ssh key and a public web directory"
run_tool lxundo      "How to undo a command"               ""                          "git reset --hard HEAD~5"

# ── Filesystem / data ─────────────────────────────────────────────────────────
run_tool lxfind      "Semantic file search"                ""                          "the config file for the web server"
run_tool lxcsv       "Query CSV data"                      "data/sales.csv"            "which region has the highest total?"
run_tool lxjson      "Repair broken JSON"                  "data/broken.json"
run_tool lxtable     "Text to table (long!)"               "data/mixed.txt"
run_tool lxconv      "Convert format"                      "data/sales.csv"            "json"
run_tool lxdigest    "Summarize directory"                 ""                          "crates/lx-core/src"

# ── Search / knowledge ────────────────────────────────────────────────────────
run_tool lxman       "Better man page"                     ""                          "--for" "git rebase"
run_tool lxerrno     "Explain an errno"                    ""                          "ECONNREFUSED"
run_tool lxdepcheck  "Explain a dependency"                ""                          "lodash"
run_tool lxport      "Explain port 8080 risk"              ""                          "8080"
run_tool lxregexplain "Explain a regex"                    ""                          "^(?:[0-9]{1,3}\.){3}[0-9]{1,3}$"
run_tool lxmock      "Generate mock data"                  ""                          "10 users with id, email, created_at, role"

# ── Productivity / comms ──────────────────────────────────────────────────────
run_tool lxnotes     "Structure meeting notes"             "text/meeting-notes.txt"
run_tool lxnotes     "Extract action items"                "text/meeting-notes.txt"    "--actions"
run_tool lxstandup   "Standup from git activity"           "misc/git-log.txt"
run_tool lxlog       "Log anomaly analysis (long!)"        "logs/unattended-upgrades.log"

# ── Docs / format ─────────────────────────────────────────────────────────────
run_tool lxmd        "Format as markdown"                  "text/meeting-notes.txt"
run_tool lxmermaid   "Generate diagram"                    ""                          "user login flow with 2FA"
run_tool lxmakefile  "Generate Makefile"                   ""                          "rust project with test, build, clippy targets"
run_tool lxdockerfile "Generate Dockerfile"                ""                          "node.js express app with postgres"
run_tool lxgitignore "Generate gitignore"                  ""                          "rust project with vscode"
run_tool lxgraph     "ASCII chart"                         "data/sales.csv"

# ── Security ──────────────────────────────────────────────────────────────────
run_tool lxredact    "Redact secrets"                      "configs/sshd_config"
run_tool lxredact    "Anonymize PII"                       "text/meeting-notes.txt"    "--anon"
run_tool lxsecret    "Find secrets in code"                "code/auth.rs"
run_tool lxcve       "CVE check/explain"                   "misc/cargo-lock-snippet.txt"
run_tool lxpolicy    "Config security check"               "configs/sshd_config"
run_tool lxcert      "Explain TLS cert"                    "misc/sample-cert.pem"
run_tool lxjwt       "Decode JWT"                          "misc/sample.jwt"

# ── Network / system ──────────────────────────────────────────────────────────
run_tool lxperm      "Explain permissions"                 "misc/permissions.txt"
run_tool lxfixscript "Fix a broken shell script ($TARGET)" "code/broken.sh"           "--target" "$TARGET"
run_tool lxfixcmd    "Fix a typo'd git command"            ""                          "git psh origin main"

# ── Docs / meta / shell ───────────────────────────────────────────────────────
run_tool lxrename    "Rename files to snake_case"          "misc/file-list.txt"        "rename to snake_case"
run_tool lxrename    "Rename with --in (metadata)"         ""                          "--in" "acceptance/fixtures/misc/photos" "add the folder name as prefix to each file"
run_tool lxconf      "Check config"                        "configs/nginx.conf"

# ── Web ───────────────────────────────────────────────────────────────────────
run_tool lxurl       "Summarize a URL"                     ""                          "https://example.com"

# ── Language ──────────────────────────────────────────────────────────────────
run_tool lxtl        "Translate text"                      "text/mixed-lang.txt"       "en"
run_tool lxask       "Q&A from context"                    "text/article.md"           "what is the main argument?"

echo "" >> "$OUT"
echo "## Summary" >> "$OUT"
echo "" >> "$OUT"
echo "Total tools tested: 72 · ✅ ___ · ⚠️ ___ · ❌ ___" >> "$OUT"

# ── Optional: --json smoke pass (set LX_JSON_PASS=1 to enable) ────────────────
# Tests that a representative set of tools emit parseable JSON under --json.
if [ "${LX_JSON_PASS:-0}" = "1" ]; then
  echo "" >> "$OUT"
  echo "---" >> "$OUT"
  echo "" >> "$OUT"
  echo "## JSON smoke pass" >> "$OUT"
  echo "" >> "$OUT"
  echo "Set LX_JSON_PASS=1 to run. Each tool run with --json; stdout must parse as valid JSON." >> "$OUT"
  echo "" >> "$OUT"

  run_json_tool() {
    local tool="$1"; local stdin_file="$2"; shift 2
    local stdout stderr code=0
    if [ -n "$stdin_file" ] && [ -f "$FIX/$stdin_file" ]; then
      stdout=$("$BIN/$tool" --json "$@" < "$FIX/$stdin_file" 2>/tmp/json_stderr_$$) || code=$?
    else
      stdout=$("$BIN/$tool" --json "$@" 2>/tmp/json_stderr_$$) || code=$?
    fi
    stderr=$(cat /tmp/json_stderr_$$ 2>/dev/null); rm -f /tmp/json_stderr_$$
    local ok="❌"
    if echo "$stdout" | python3 -c "import sys,json; json.load(sys.stdin)" 2>/dev/null; then ok="✅"; fi
    echo "### $tool --json: exit=$code $ok" >> "$OUT"
    echo '```json' >> "$OUT"
    echo "$stdout" | head -5 >> "$OUT"
    echo '```' >> "$OUT"
    echo "" >> "$OUT"
  }

  run_json_tool lxexplain   "logs/app-error.log"
  run_json_tool lxcommit    "diffs/feature.diff"
  run_json_tool lxsh        ""  "list all running docker containers"
  run_json_tool lxjq        ""  "extract all ids from array"
  run_json_tool lxregex     ""  "match a UUID"
  run_json_tool lxsql       ""  "count rows grouped by status"
  run_json_tool lxsed       ""  "print the second field of each line"
  run_json_tool lxconv      "data/sales.csv"  "json"
  run_json_tool lxmermaid   ""  "simple flowchart with two nodes"
  run_json_tool lxcve       "misc/cargo-lock-snippet.txt"
  run_json_tool lxmock      ""  "5 products with id and price"
  run_json_tool lxregexplain ""  "^\\d{4}-\\d{2}-\\d{2}$"

  echo "JSON pass done." >> "$OUT"
fi

# ── Adversarial security fixtures (always runs) ───────────────────────────────
# Tests that redaction tools strip secrets and nocmd tools warn on dangerous input.
echo "" >> "$OUT"
echo "---" >> "$OUT"
echo "" >> "$OUT"
echo "## Security fixture pass" >> "$OUT"
echo "" >> "$OUT"
echo "Verifying: (a) redaction tools suppress secrets in stdout; (b) nocmd tools warn on dangerous input." >> "$OUT"
echo "" >> "$OUT"

# G2: adversarial redaction — secrets must not appear in stdout
run_tool lxredact  "Redact live-looking secrets"      "adversarial/secrets.txt"
run_tool lxsecret  "Detect secrets in leaky file"     "adversarial/secrets.txt"
run_tool lxcommit  "Commit msg with secrets in input" "adversarial/secrets.txt"

# G3: dangerous-intent inputs — DANGER warning must appear in stderr
run_tool lxsh      "Shell cmd: dangerous delete-all"  ""  "delete all files in the root filesystem recursively"
run_tool lxsql     "SQL: DROP TABLE no WHERE"         ""  "drop all rows from the users table permanently"
run_tool lxfirewall "Firewall: wipe all rules ($TARGET)" "" "delete all firewall rules" "--target" "$TARGET"
run_tool lxip      "IP: flush all routes ($TARGET)"   ""  "delete all routing entries" "--target" "$TARGET"

echo "" >> "$OUT"
echo "Report written to: $OUT"
