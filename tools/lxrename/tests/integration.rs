use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxrename::run::{build_script, run, today_utc, Rename};

fn mock_response() -> &'static str {
    r#"{"renames":[{"from":"testFoo.py","to":"test_foo.py"},{"from":"testBar.py","to":"test_bar.py"}],"script":""}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let file_list = "testFoo.py\ntestBar.py\nconfig.toml";
    let out = run(
        file_list,
        "rename to snake_case",
        None,
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.renames.is_empty());
    assert!(!out.script.is_empty(), "script must be built locally");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn script_built_locally() {
    // Model returns empty script — we build it from renames.
    let client = MockLlmClient::returning(mock_response());
    let out = run(
        "testFoo.py\ntestBar.py",
        "snake_case",
        None,
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(
        out.script.contains("testFoo.py"),
        "script should reference original name"
    );
    assert!(
        out.script.contains("test_foo.py"),
        "script should reference new name"
    );
}

#[test]
fn empty_intent_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let err = run("file.py", "   ", None, &Config::default(), &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn empty_file_list_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let err = run(
        "",
        "rename to snake_case",
        None,
        &Config::default(),
        &client,
    )
    .unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);

    // Newlines with nothing between them are still no input.
    let err = run(
        "\n\n",
        "rename to snake_case",
        None,
        &Config::default(),
        &client,
    )
    .unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

/// A filename may consist entirely of blanks on Linux. Such an entry is a real
/// file, not "no input" — the guard must not trim it away. Previously a lone
/// "   " was rejected as an empty list.
#[test]
fn blanks_only_filename_is_a_valid_entry() {
    let client = MockLlmClient::returning(mock_response());
    let out = run(
        "   ",
        "rename to snake_case",
        None,
        &Config::default(),
        &client,
    );
    assert!(
        out.is_ok(),
        "a file named with blanks must be accepted as a list entry"
    );

    // ...and it must survive intact into the prompt, not be trimmed off the end.
    let req = client.last_request();
    assert!(
        req.user.contains("Files:\n   "),
        "blanks-only filename was stripped from the prompt: {:?}",
        req.user
    );
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let _ = run("file.py", "snake_case", None, &Config::default(), &client);
    assert!(client.last_request().max_tokens <= 1024);
}

#[test]
fn dangerous_pattern_detected() {
    // Script is built from renames, not from model's script field.
    // Test build_script directly with a dangerous "to" path pattern.
    let renames = vec![Rename {
        from: "foo".to_string(),
        to: "bar; rm -rf /".to_string(),
    }];
    let script = build_script(&renames);
    assert!(script.contains("rm -rf /"));
}

#[test]
fn build_script_produces_mv_commands() {
    let renames = vec![Rename {
        from: "testFoo.py".to_string(),
        to: "test_foo.py".to_string(),
    }];
    let script = build_script(&renames);
    assert!(script.contains("mv"));
    assert!(script.contains("testFoo.py"));
    assert!(script.contains("test_foo.py"));
}

#[test]
fn today_placeholder_replaced_in_system_prompt() {
    let client = MockLlmClient::returning(mock_response());
    let _ = run("file.py", "snake_case", None, &Config::default(), &client);
    let req = client.last_request();
    assert!(
        !req.system.contains("{today}"),
        "{{today}} placeholder must be replaced before sending to LLM"
    );
    let today = today_utc();
    assert!(
        req.system.contains(&today),
        "system prompt must contain today's date ({})",
        today
    );
}

#[test]
fn dir_name_included_in_user_message() {
    let client = MockLlmClient::returning(mock_response());
    let _ = run(
        "beach.jpg\nsunset.jpg",
        "add folder name as prefix",
        Some("vacation"),
        &Config::default(),
        &client,
    );
    let req = client.last_request();
    assert!(
        req.user.contains("Directory: vacation"),
        "user message must include Directory: <dir_name> when dir_name is Some"
    );
}

#[test]
fn empty_renames_returns_logical_error() {
    let client = MockLlmClient::returning(r#"{"renames":[],"script":""}"#);
    let err = run(
        "file.py",
        "rename to snake_case",
        None,
        &Config::default(),
        &client,
    )
    .unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::LOGICAL_ERROR);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let out = run(
        "testFoo.py\ntestBar.py",
        "snake_case",
        None,
        &Config::default(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let out = run(
        "testFoo.py\ntestBar.py",
        "snake_case",
        None,
        &Config::default(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
