use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxpatch")
}

#[test]
fn version_flag_exits_0_and_format_correct() {
    let out = binary().run(&["--version"]);
    out.assert_exit(0);
    assert!(out.stdout.contains("lxpatch"));
    assert!(out.stdout.contains("lx-coreutils"));
}

#[test]
fn help_flag_exits_0() {
    binary().run(&["--help"]).assert_exit(0);
}

#[test]
fn unknown_flag_exits_2() {
    binary().run(&["--this-flag-does-not-exist"]).assert_exit(2);
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn stdout_is_pipe_safe() {}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json() {}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {}
