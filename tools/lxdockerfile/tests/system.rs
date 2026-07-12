// System tests — run the compiled binary as a black box.
// Requires: cargo build -p lxdockerfile before running.
// Tests 1-3 need no network. Tests 4-6 require LX_API_KEY.

use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxdockerfile")
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
        out.stdout.trim().starts_with("lxdockerfile"),
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
fn create_mode_stdout_is_pipe_safe() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&[], "Node.js 18 app with npm, exposes port 3000");
    out.assert_success();
    out.assert_stdout_pipe_safe();
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn edit_mode_produces_output() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let existing = "FROM node:18-alpine\nWORKDIR /app\nEXPOSE 3000\nCMD [\"node\", \"index.js\"]";
    let out = binary().run_with_stdin(&["switch to node:20-alpine"], existing);
    out.assert_success();
    assert!(!out.stdout.is_empty(), "edit mode must produce output");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&["--json"], "Node.js 18 app with npm, exposes port 3000");
    out.assert_success();
    out.assert_stdout_valid_json();
    out.assert_json_field("content");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&["--quiet"], "Node.js 18 app with npm, exposes port 3000");
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
