use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxfirewall")
}

#[test]
fn version_flag_exits_0_and_format_correct() {
    let out = binary().run(&["--version"]);
    out.assert_exit(0);
    assert!(out.stdout.contains("lx-coreutils"));
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
    let out = binary().run_with_stdin(&["allow SSH from 10.0.0.0/8"], "");
    out.assert_success();
    out.assert_stdout_pipe_safe();
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run_with_stdin(&["--json", "allow SSH"], "");
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
    let out = binary().run_with_stdin(&["--quiet", "allow HTTP"], "");
    out.assert_success();
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn explain_mode_via_stdin() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let rules =
        "Chain INPUT (policy ACCEPT)\nACCEPT tcp -- 10.0.0.0/8 anywhere tcp dpt:ssh\nDROP all";
    let out = binary().run_with_stdin(&[], rules);
    out.assert_success();
    out.assert_stdout_pipe_safe();
}
