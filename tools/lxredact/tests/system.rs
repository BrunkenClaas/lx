// System tests — run the compiled binary as a black box.
// Requires: cargo build -p lxredact before running.
// Tests 1-3 run without network. Tests 4-6 require LX_API_KEY.

use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxredact")
}

// ── Tests 1–3: no network required ───────────────────────────────────────────

#[test]
fn version_flag_exits_0_and_format_correct() {
    let out = binary().run(&["--version"]);
    out.assert_exit(0);
    assert!(
        out.stdout.contains("lx-coreutils"),
        "--version must contain 'lx-coreutils': {}",
        out.stdout
    );
    assert!(
        out.stdout.trim().starts_with("lxredact"),
        "--version must start with tool name: {}",
        out.stdout
    );
}

#[test]
fn help_flag_exits_0() {
    let out = binary().run(&["--help"]);
    out.assert_exit(0);
    assert!(!out.stdout.is_empty(), "--help must produce output");
}

#[test]
fn unknown_flag_exits_2() {
    let out = binary().run(&["--this-flag-does-not-exist-xyz"]);
    out.assert_exit(2);
}

// ── Tests 4–6: require LX_API_KEY (explain mode) ─────────────────────────────

#[test]
#[ignore = "system: set LX_API_KEY"]
fn stdout_is_pipe_safe() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let input = "This text has no secrets and is safe to redact.";
    let out = binary().run_with_stdin(&[], input);
    out.assert_success();
    out.assert_stdout_pipe_safe();
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json_on_stdout() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let input = "Hello world. No secrets here.";
    let out = binary().run_with_stdin(&["--json"], input);
    out.assert_success();
    out.assert_stdout_valid_json();
    out.assert_json_field("redacted_count");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let input = "Hello world. No secrets here.";
    let out = binary().run_with_stdin(&["--quiet"], input);
    out.assert_success();
    let stderr_lines: Vec<&str> = out
        .stderr
        .lines()
        .filter(|l| !l.contains("DANGER") && !l.contains("WARNING"))
        .collect();
    assert!(
        stderr_lines.is_empty(),
        "--quiet must suppress stderr diagnostic lines, got: {:?}",
        stderr_lines
    );
}

// ── Additional no-network tests ───────────────────────────────────────────────

#[test]
fn redacts_secret_locally_without_api_key() {
    // This test verifies the core feature: local redaction works without any
    // API key (no --explain, so no LLM call is made).
    let out = binary().run_with_stdin(&[], "DATABASE_URL=postgres://user:s3cr3t@localhost/db");
    // Even without an API key, local redaction must succeed.
    out.assert_success();
    assert!(
        !out.stdout.contains("s3cr3t"),
        "secret password must not appear in stdout: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("[REDACTED]"),
        "redaction placeholder must appear in stdout: {}",
        out.stdout
    );
}

#[test]
fn no_redact_flag_prints_warning_on_stderr() {
    let out = binary().run_with_stdin(&["--no-redact"], "token=sk-abcdefghijklmnopqrstu");
    out.assert_success();
    assert!(
        out.stderr.contains("WARNING"),
        "--no-redact must print a WARNING to stderr: {}",
        out.stderr
    );
}

#[test]
fn strict_flag_accepted() {
    let out = binary().run_with_stdin(&["--strict"], "server at 10.0.0.1");
    // --strict bundles PII masking, so it must redact the IP address.
    out.assert_success();
    assert!(
        !out.stdout.contains("10.0.0.1"),
        "strict mode must redact IP addresses: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("[IP]"),
        "strict mode must insert [IP] placeholder: {}",
        out.stdout
    );
}

#[test]
fn removed_level_flag_exits_2() {
    // --level was removed in favour of --strict; it must now be a usage error.
    let out = binary().run_with_stdin(&["--level", "strict"], "x");
    out.assert_exit(2);
}
