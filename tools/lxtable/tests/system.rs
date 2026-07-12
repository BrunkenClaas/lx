// System tests — run the compiled binary as a black box.
// Requires: cargo build -p lxtable before running.
// Tests 1-3 need no network. Tests 4-6 require LX_API_KEY.

use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxtable")
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
        out.stdout.trim().starts_with("lxtable"),
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
    let out = binary().run_with_stdin(
        &[],
        "Alice is 30, Engineer. Bob is 25, Designer. Carol is 28, Manager.",
    );
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
        "Product: Widget A, Price: $9.99, Stock: 150\nProduct: Gadget B, Price: $24.99, Stock: 42",
    );
    out.assert_success();
    out.assert_stdout_valid_json();
    out.assert_json_field("columns");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(
        &["--quiet"],
        "Q1 revenue was $1.2M, Q2 was $1.5M, Q3 reached $1.8M, Q4 closed at $2.1M.",
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
