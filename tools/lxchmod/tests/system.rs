// System tests — run the compiled binary as a black box.
// Requires: cargo build -p lxchmod before running.
// Tests 1-3 need no network. Tests 4-6 require LX_API_KEY.

use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxchmod")
}

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
        out.stdout.trim().starts_with("lxchmod"),
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

#[test]
#[ignore = "system: set LX_API_KEY"]
fn stdout_is_pipe_safe() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&[], "-rw-rw-rw- 1 user group 1234 Jan 01 12:00 data.csv");
    out.assert_success();
    out.assert_stdout_pipe_safe();
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json_on_stdout() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(
        &["--json"],
        "-rw-rw-rw- 1 user group 1234 Jan 01 12:00 data.csv",
    );
    out.assert_success();
    out.assert_stdout_valid_json();
    out.assert_json_field("suggestion");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(
        &["--quiet"],
        "-rw-rw-rw- 1 user group 1234 Jan 01 12:00 data.csv",
    );
    out.assert_success();
    let stderr_lines: Vec<&str> = out
        .stderr
        .lines()
        .filter(|l| !l.contains("DANGER") && !l.contains("WARNING"))
        .collect();
    assert!(
        stderr_lines.is_empty(),
        "--quiet must suppress stderr, got: {:?}",
        stderr_lines
    );
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn dangerous_output_exits_3() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&[], "-rwxrwxrwx 1 root root 4096 Jan 01 12:00 /etc/shadow");
    out.assert_exit(3);
    assert!(
        out.stderr.contains("DANGER") || out.stderr.contains("WARNING"),
        "stderr must contain danger warning: {}",
        out.stderr
    );
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn allow_dangerous_flag_exits_0_on_dangerous_output() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(
        &["--allow-dangerous"],
        "-rwxrwxrwx 1 root root 4096 Jan 01 12:00 /etc/shadow",
    );
    out.assert_exit(0);
    assert!(
        out.stderr.contains("DANGER") || out.stderr.contains("WARNING"),
        "danger warning must still appear on stderr with --allow-dangerous: {}",
        out.stderr
    );
}
