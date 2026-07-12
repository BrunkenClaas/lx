use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxman::run::run;

fn mock_response() -> &'static str {
    r#"{"summary":"grep searches for lines matching a pattern in files or stdin.","examples":["grep 'error' app.log — find all lines containing 'error'","grep -r 'TODO' ./src — search recursively","grep -n 'pattern' file.txt — show line numbers"]}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run("grep", &config, &client).unwrap();
    assert!(!out.summary.is_empty(), "summary must not be empty");
    assert!(!out.examples.is_empty(), "examples must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_tool_name_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("curl", &config, &client);
    let req = client.last_request();
    assert!(req.max_tokens <= 512, "lxman max_tokens should be ≤ 512");
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("git", &config, &client);
    let req = client.last_request();
    assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
}

#[test]
fn system_prompt_not_empty() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("ls", &config, &client);
    let req = client.last_request();
    assert!(!req.system.is_empty(), "system prompt must not be empty");
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run("grep", &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run("grep", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
