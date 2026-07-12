use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxprintf::run::{run, Output};

fn mock_response() -> &'static str {
    r#"{"format":"%Y-%m-%d %H:%M:%S","explanation":"%Y=4-digit year, %m=2-digit month, %d=2-digit day, %H=24h hour, %M=minutes, %S=seconds."}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run("ISO date and time", &config, &client).unwrap();
    assert!(!out.format.is_empty(), "format must not be empty");
    assert!(!out.explanation.is_empty(), "explanation must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("ISO date and time", &config, &client);
    let req = client.last_request();
    assert!(
        req.max_tokens <= 256,
        "lxprintf max_tokens should be <= 256, got {}",
        req.max_tokens
    );
}

#[test]
fn snapshot_plain_output() {
    let out: Output = serde_json::from_str(mock_response()).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let out: Output = serde_json::from_str(mock_response()).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
