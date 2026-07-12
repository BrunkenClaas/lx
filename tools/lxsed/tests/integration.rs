use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxsed::run::run;

fn mock_awk() -> &'static str {
    r#"{"command":"awk '{print $2}'","tool":"awk","dangerous":false}"#
}

fn mock_sed() -> &'static str {
    r#"{"command":"sed 's/foo/bar/g'","tool":"sed","dangerous":false}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_awk());
    let config = Config::default();
    let (out, _findings) = run("print the second column of each line", &config, &client).unwrap();
    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(!out.tool.is_empty(), "tool must not be empty");
    assert!(
        out.tool == "awk" || out.tool == "sed",
        "tool must be 'awk' or 'sed', got: {}",
        out.tool
    );
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_awk());
    let config = Config::default();
    let (out, _findings) = run("print the second column", &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_awk());
    let config = Config::default();
    let (out, _findings) = run("print the second column", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn sed_tool_accepted() {
    let client = MockLlmClient::returning(mock_sed());
    let config = Config::default();
    let (out, _findings) = run("replace foo with bar", &config, &client).unwrap();
    assert_eq!(out.tool, "sed");
    assert!(!out.command.is_empty());
}

#[test]
fn dangerous_command_flagged() {
    // Mock returns a command with a dangerous pipe-to-shell pattern.
    let client = MockLlmClient::returning(
        r#"{"command":"awk '{print $0}' file | sh","tool":"awk","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run("run each line as a shell command", &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local danger detection must flag pipe-to-shell patterns"
    );
}

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_awk());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn invalid_tool_field_returns_error() {
    let client =
        MockLlmClient::returning(r#"{"command":"grep something","tool":"grep","dangerous":false}"#);
    let config = Config::default();
    let err = run("find lines containing something", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::LOGICAL_ERROR);
}
