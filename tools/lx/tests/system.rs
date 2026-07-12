use lx_testkit::binary::BinaryUnderTest;

fn binary() -> BinaryUnderTest {
    BinaryUnderTest::for_tool("lx")
}

#[test]
fn version_flag_exits_0_and_format_correct() {
    let out = binary().run(&["--version"]);
    out.assert_exit(0);
    assert!(
        out.stdout.contains("lx-coreutils"),
        "version string must contain 'lx-coreutils', got: {}",
        out.stdout
    );
    assert!(
        out.stdout.starts_with("lx "),
        "version must start with 'lx ', got: {}",
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
fn no_args_exits_0_and_shows_help() {
    let out = binary().run(&[]);
    out.assert_exit(0);
    assert!(
        out.stdout.contains("Usage") || out.stdout.contains("usage"),
        "no-arg output must show help/usage, got: {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("tools")
            && out.stdout.contains("model")
            && out.stdout.contains("config"),
        "help must mention the subcommands, got: {}",
        out.stdout
    );
}

#[test]
fn tools_subcommand_exits_0() {
    let out = binary().run(&["tools"]);
    out.assert_exit(0);
    assert!(out.stdout.contains("lxcommit"));
}

#[test]
fn tools_json_is_valid_json_array() {
    let out = binary().run(&["tools", "--json"]);
    out.assert_exit(0);
    out.assert_stdout_valid_json();
    let parsed: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 72);
}

#[test]
fn tools_keyword_commit_shows_lxcommit() {
    let out = binary().run(&["tools", "commit"]);
    out.assert_exit(0);
    assert!(
        out.stdout.contains("lxcommit"),
        "keyword 'commit' must show lxcommit"
    );
}

#[test]
fn model_no_verify_exits_0_and_prints_model() {
    // --no-verify makes no network call and needs no API key: it must always
    // resolve and print the effective model name as a single stdout line.
    let out = binary().run(&["model", "--no-verify"]);
    out.assert_exit(0);
    assert!(
        !out.stdout.trim().is_empty(),
        "model name must be printed to stdout, got empty"
    );
    assert!(
        out.stdout.trim().lines().count() == 1,
        "plain model output must be a single pipe-safe line, got: {:?}",
        out.stdout
    );
    assert!(
        out.stderr.contains("provider:"),
        "provider diagnostic must go to stderr, got: {}",
        out.stderr
    );
}

#[test]
fn model_no_verify_json_is_valid() {
    let out = binary().run(&["model", "--no-verify", "--json"]);
    out.assert_exit(0);
    out.assert_stdout_valid_json();
    let parsed: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert!(parsed.get("model").is_some(), "json must have 'model'");
    assert!(
        parsed.get("provider").is_some(),
        "json must have 'provider'"
    );
    assert!(
        parsed.get("reachable").unwrap().is_null(),
        "reachable must be null with --no-verify"
    );
}

#[test]
fn model_help_exits_0() {
    let out = binary().run(&["model", "--help"]);
    out.assert_exit(0);
}

#[test]
fn tools_no_match_exits_0() {
    let out = binary().run(&["tools", "zzz_no_such_tool_99999"]);
    out.assert_exit(0);
    // Output should be empty (hint goes to stderr).
    assert!(
        out.stdout.trim().is_empty(),
        "no-match stdout must be empty"
    );
}

// ── lx config ─────────────────────────────────────────────────────────────────

#[test]
fn config_help_exits_0() {
    let out = binary().run(&["config", "--help"]);
    out.assert_exit(0);
}

#[test]
fn config_yes_print_exits_0_and_stdout_is_valid_toml() {
    // --yes --print: non-interactive, output only to stdout, no file written.
    let out = binary().run(&["config", "--yes", "--print"]);
    out.assert_exit(0);
    assert!(
        !out.stdout.trim().is_empty(),
        "stdout must contain TOML output"
    );
    // Must contain a provider line.
    assert!(
        out.stdout.contains("provider"),
        "TOML output must contain provider key"
    );
    // Must NOT contain api_key (secrets must never appear).
    assert!(
        !out.stdout.contains("api_key"),
        "api_key must never appear in TOML output"
    );
    // Must NOT contain shell (runtime-only, skipped in serialization).
    assert!(
        !out.stdout.contains("shell ="),
        "shell must never appear in TOML output"
    );
}

#[test]
fn config_yes_print_default_provider_is_ollama() {
    let out = binary().run(&["config", "--yes", "--print"]);
    out.assert_exit(0);
    assert!(
        out.stdout.contains("ollama"),
        "default non-interactive config must use ollama provider"
    );
}

#[test]
fn config_non_tty_stdin_without_yes_exits_nonzero() {
    // Pipe empty input — stdin is not a TTY and --yes is not passed.
    let out = binary().run_with_stdin(&["config"], "");
    assert_ne!(
        out.exit_code, 0,
        "non-TTY stdin without --yes must exit non-zero"
    );
    assert!(
        out.stderr.contains("--yes") || out.stderr.contains("terminal"),
        "error message must mention --yes or terminal, got: {}",
        out.stderr
    );
}
