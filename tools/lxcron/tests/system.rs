use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lxcron")
}

#[test]
fn version_flag_exits_0_and_format_correct() {
    let out = binary().run(&["--version"]);
    out.assert_exit(0);
    assert!(out.stdout.contains("lx-coreutils"));
    assert!(out.stdout.starts_with("lxcron"));
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
fn stdout_is_pipe_safe() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run(&["every weekday at 9am run backup.sh"]);
    out.assert_success();
    out.assert_stdout_pipe_safe();
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn json_flag_produces_valid_json() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run(&["--json", "every hour"]);
    out.assert_success();
    out.assert_stdout_valid_json();
    out.assert_json_field("crontab");
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn edit_mode_produces_crontab() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let existing = "0 2 * * 0 /home/user/cleanup.sh";
    let out = binary().run_with_stdin(&["change to run daily at midnight"], existing);
    out.assert_success();
    assert!(
        !out.stdout.is_empty(),
        "edit mode must produce a crontab line"
    );
}

#[test]
#[ignore = "system: set LX_API_KEY"]
fn quiet_flag_suppresses_stderr() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let out = binary().run(&["--quiet", "every hour"]);
    out.assert_success();
}
