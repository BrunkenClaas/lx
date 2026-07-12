use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxask")
}

#[test]
fn version_flag_exits_0_and_format_correct() {
    let out = binary().run(&["--version"]);
    out.assert_exit(0);
    assert!(
        out.stdout.contains("lxask"),
        "stdout missing tool name: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("lx-coreutils"),
        "stdout missing suite label: {}",
        out.stdout
    );
}

#[test]
fn help_flag_exits_0() {
    let out = binary().run(&["--help"]);
    out.assert_exit(0);
}

#[test]
fn unknown_flag_exits_2() {
    let out = binary().run(&["--this-flag-does-not-exist"]);
    out.assert_exit(2);
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn stdout_is_pipe_safe() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run(&["What is 2 plus 2?"]);
    out.assert_success();
    out.assert_stdout_pipe_safe();
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run(&["--json", "What is 2 plus 2?"]);
    out.assert_success();
    out.assert_stdout_valid_json();
    out.assert_json_field("answer");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run(&["--quiet", "What is 2 plus 2?"]);
    out.assert_success();
    assert!(
        out.stderr.is_empty(),
        "stderr should be empty with --quiet, got: {}",
        out.stderr
    );
}
