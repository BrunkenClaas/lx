// System tests — run the compiled binary as a black box.
// Requires: cargo build -p lxkill before running.
// Tests 1-3 need no network. Tests 4-6 require LX_API_KEY.

use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxkill")
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
        out.stdout.trim().starts_with("lxkill"),
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
fn target_flag_appears_in_help() {
    let out = binary().run(&["--help"]);
    out.assert_exit(0);
    assert!(
        out.stdout.contains("--target"),
        "--help must document --target flag"
    );
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn stdout_is_pipe_safe() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&["process listening on port 8080"], "");
    out.assert_success();
    out.assert_stdout_pipe_safe();
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json_on_stdout() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&["--json", "process listening on port 8080"], "");
    out.assert_success();
    out.assert_stdout_valid_json();
    out.assert_json_field("command");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&["--quiet", "process on port 9090"], "");
    out.assert_success();
    let stderr_lines: Vec<&str> = out
        .stderr
        .lines()
        .filter(|l| !l.contains("DANGER"))
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
    let out = binary().run_with_stdin(&["kill the init process PID 1"], "");
    out.assert_exit(3);
    assert!(
        out.stderr.contains("DANGER"),
        "stderr must contain DANGER warning: {}",
        out.stderr
    );
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn allow_dangerous_flag_exits_0_on_dangerous_output() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&["--allow-dangerous", "kill the init process PID 1"], "");
    out.assert_exit(0);
    assert!(
        out.stderr.contains("DANGER"),
        "DANGER warning must still appear on stderr with --allow-dangerous: {}",
        out.stderr
    );
}
