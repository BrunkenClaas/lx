use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxcommit::run::run;

fn mock_response() -> &'static str {
    r#"{"type":"feat","scope":"auth","subject":"add token refresh method","body":"Allows callers to exchange a refresh token for a new access token."}"#
}

fn sample_diff() -> &'static str {
    include_str!("fixtures/sample_add_function.diff")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, warnings) = run(sample_diff(), &config, &client).unwrap();
    assert!(!out.commit_type.is_empty());
    assert!(!out.subject.is_empty());
    assert!(out.subject.len() <= 72, "subject too long");
    assert!(warnings.is_empty(), "small diff must not warn");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn secrets_never_reach_llm() {
    let diff_with_secret = include_str!("fixtures/diff_with_secret.diff");
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    // Run should succeed (redaction replaces the secret)
    let _ = run(diff_with_secret, &config, &client);
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn empty_diff_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_diff(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_diff(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
