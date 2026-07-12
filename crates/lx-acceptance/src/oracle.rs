//! Execution oracles — grade a tool's output by *using* it, where that can be
//! done with a pure (side-effect-free) function.
//!
//! Hard rule (matches the suite's own security model): command-shaped output of
//! dangerous tools (shell/docker/kubectl/firewall/...) is NEVER executed. Only
//! pure-function oracles run here:
//!   * `regex` — compile the generated pattern with the same `regex` crate the
//!     tools target and test it against should/should-not-match strings.
//!   * `json`  — the generated artifact must parse as JSON.
//!   * `jq`/`sed` — run an external, side-effect-free binary on fixture data fed
//!     on stdin. Probe-or-SKIP: absent binary => skipped, never failed.
//!
//! Future work: a SQLite execution oracle for lxsql (run generated SQL against
//! an in-memory throwaway DB). Dropped for now — no SQLite crate is on the
//! allow-list; lxsql is graded structurally instead.

use once_cell::sync::Lazy;
use std::process::{Command, Stdio};

use crate::intents::{Intent, Oracle};

/// Outcome of running an oracle.
pub enum OracleResult {
    /// Oracle not applicable (Oracle::None) — nothing to do.
    NotApplicable,
    /// Oracle ran and the output satisfied it.
    Pass,
    /// Oracle ran and found problems (one message per failure).
    Fail(Vec<String>),
    /// Oracle skipped with a visible reason (e.g. external binary missing).
    Skipped(String),
}

/// Runs the configured oracle against the tool's extracted field value.
/// `field_value` is the string content of the checked JSON field (e.g. the
/// generated regex pattern, the generated JSON document, the jq expression).
pub fn run(intent: &Intent, field_value: &str) -> OracleResult {
    match intent.oracle {
        Oracle::None => OracleResult::NotApplicable,
        Oracle::Regex => regex_oracle(intent, field_value),
        Oracle::Json => json_oracle(field_value),
        Oracle::Jq => external_oracle("jq", intent, field_value),
        Oracle::Sed => external_oracle("sed", intent, field_value),
    }
}

/// Compiles the generated pattern and checks should/should-not-match strings.
fn regex_oracle(intent: &Intent, pattern: &str) -> OracleResult {
    let re = match regex::Regex::new(pattern) {
        Ok(re) => re,
        Err(e) => {
            return OracleResult::Fail(vec![format!("pattern does not compile: {e}")]);
        }
    };
    let mut fails = Vec::new();
    for s in &intent.should_match {
        if !re.is_match(s) {
            fails.push(format!("should match {s:?} but did not"));
        }
    }
    for s in &intent.should_not_match {
        if re.is_match(s) {
            fails.push(format!("should NOT match {s:?} but did"));
        }
    }
    if fails.is_empty() {
        OracleResult::Pass
    } else {
        OracleResult::Fail(fails)
    }
}

/// The generated artifact must parse as JSON.
fn json_oracle(value: &str) -> OracleResult {
    match serde_json::from_str::<serde_json::Value>(value) {
        Ok(_) => OracleResult::Pass,
        Err(e) => OracleResult::Fail(vec![format!("generated content is not valid JSON: {e}")]),
    }
}

/// Has an external binary been found on PATH? Probed once and cached.
fn binary_available(bin: &str) -> bool {
    static JQ: Lazy<bool> = Lazy::new(|| probe("jq"));
    static SED: Lazy<bool> = Lazy::new(|| probe("sed"));
    match bin {
        "jq" => *JQ,
        "sed" => *SED,
        _ => probe(bin),
    }
}

fn probe(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Runs `jq`/`sed` with the generated expression on fixture data fed via stdin.
/// Never uses `-i` or a file argument; refuses to run a generated command that
/// looks like it writes in place.
fn external_oracle(bin: &str, intent: &Intent, expr: &str) -> OracleResult {
    if !binary_available(bin) {
        return OracleResult::Skipped(format!("{bin} not installed"));
    }
    // Defense in depth: never run an in-place / redirecting transform.
    if bin == "sed" && (expr.contains(" -i") || expr.starts_with("-i") || expr.contains('>')) {
        return OracleResult::Skipped(format!(
            "generated {bin} looks in-place/redirecting; skipped"
        ));
    }
    let stdin_data = match &intent.oracle_stdin {
        Some(rel) => match read_fixture(rel) {
            Ok(d) => d,
            Err(e) => return OracleResult::Fail(vec![format!("oracle_stdin unreadable: {e}")]),
        },
        None => String::new(),
    };

    use std::io::Write;
    // sed/awk tools emit the full shell command (e.g. "sed '/^$/d'"), not just
    // the expression. Run via `sh -c` so the whole command is interpreted by the
    // shell rather than passing the expression as a single positional argument.
    // jq outputs a bare expression (".users[].name"), so it still gets its own arg.
    let mut child = if bin == "sed" {
        match Command::new("sh")
            .args(["-c", expr])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return OracleResult::Skipped(format!("{bin} failed to spawn: {e}")),
        }
    } else {
        match Command::new(bin)
            .arg(expr)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return OracleResult::Skipped(format!("{bin} failed to spawn: {e}")),
        }
    };
    if let Some(mut sin) = child.stdin.take() {
        let _ = sin.write_all(stdin_data.as_bytes());
    }
    let out = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => return OracleResult::Fail(vec![format!("{bin} run failed: {e}")]),
    };
    let mut fails = Vec::new();
    if !out.status.success() {
        fails.push(format!(
            "{bin} exited {}: {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    if out.status.success() && stdout.trim().is_empty() {
        fails.push(format!("{bin} produced empty output"));
    }
    for s in &intent.should_match {
        if !stdout.contains(s.as_str()) {
            fails.push(format!("{bin} output missing {s:?}"));
        }
    }
    if fails.is_empty() {
        OracleResult::Pass
    } else {
        OracleResult::Fail(fails)
    }
}

/// Reads a fixture relative to `acceptance/fixtures`.
pub fn read_fixture(rel: &str) -> std::io::Result<String> {
    let path = crate::fixtures_dir().join(rel);
    std::fs::read_to_string(path)
}
