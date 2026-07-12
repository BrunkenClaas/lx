// System tests — run the compiled binary as a black box.
// Requires: cargo build -p lxgrep before running.
// Tests 1-3 need no network. Tests 4-6 require LX_API_KEY.

use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxgrep")
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
        out.stdout.trim().starts_with("lxgrep"),
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
    let content = "fn connect(host: &str, port: u16) -> Connection {\n    Connection::new(host, port)\n}\n\nfn add(a: i32, b: i32) -> i32 { a + b }\n";
    let out = binary().run_with_stdin(&["database connection"], content);
    out.assert_success();
    // stdout should be either empty (no matches) or grep-compatible lines.
    for line in out.stdout.lines() {
        assert!(
            !line.starts_with('#'),
            "comment line on stdout (pipe unsafe): {line:?}"
        );
        assert!(
            !line.starts_with("//"),
            "comment line on stdout (pipe unsafe): {line:?}"
        );
    }
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json_on_stdout() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let content =
        "fn connect(host: &str, port: u16) -> Connection {\n    Connection::new(host, port)\n}\n";
    let out = binary().run_with_stdin(&["--json", "connection"], content);
    out.assert_success();
    out.assert_stdout_valid_json();
    out.assert_json_field("matches");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let content = "fn error_handler(e: Error) {\n    eprintln!(\"error: {e}\");\n}\n";
    let out = binary().run_with_stdin(&["--quiet", "error handling"], content);
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
