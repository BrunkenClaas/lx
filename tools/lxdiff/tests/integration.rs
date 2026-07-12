use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxdiff::run::run;

fn mock_response() -> &'static str {
    r#"{"summary":"Adds a bounded cache with eviction support.","changes":["A max_entries field is added to the Cache struct to limit its size.","A with_capacity constructor allows callers to set the entry limit.","The insert method evicts the oldest entry when the cache is full."]}"#
}

fn sample_diff() -> &'static str {
    include_str!("fixtures/sample.diff")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_diff(), &config, &client).unwrap();
    assert!(!out.summary.is_empty(), "summary must not be empty");
    assert!(!out.changes.is_empty(), "changes must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn secrets_never_reach_llm() {
    let diff_with_secret = include_str!("fixtures/sample_with_secret.diff");
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    // run() always redacts; the raw secret must not appear in either system or user fields.
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
fn whitespace_only_diff_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   \n\t\n", &config, &client).unwrap_err();
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
