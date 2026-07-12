use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxexplain::run::run;

fn mock_response() -> &'static str {
    r#"{"summary":"Extracts files from a tar archive.","details":["'-x' extracts","'-z' decompresses with gzip","'-f' names the archive"]}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run("tar -xzf foo.tar.gz", &config, &client).unwrap();
    assert!(!out.summary.is_empty(), "summary must not be empty");
    assert!(!out.details.is_empty(), "details must not be empty");
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
    let _ = run("ls -la", &config, &client);
    let req = client.last_request();
    assert!(
        req.max_tokens <= 512,
        "lxexplain max_tokens should be ≤ 512"
    );
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run("tar -xzf foo.tar.gz", &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run("tar -xzf foo.tar.gz", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
