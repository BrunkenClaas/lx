use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxport::run::run;

fn mock_response() -> &'static str {
    r#"{"port":22,"likely_service":"SSH","explanation":"Port 22 is the standard SSH port used for remote login. It is a frequent target for brute-force attacks.","risk":"medium"}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let out = run(22, "", &Config::default(), &client).unwrap();
    assert_eq!(out.port, 22);
    assert!(!out.likely_service.is_empty());
    assert!(!out.explanation.is_empty());
    assert!(["low", "medium", "high"].contains(&out.risk.as_str()));
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn port_set_locally() {
    // Model returns port=9999 but we override with 22.
    let resp = r#"{"port":9999,"likely_service":"SSH","explanation":"SSH port","risk":"medium"}"#;
    let client = MockLlmClient::returning(resp);
    let out = run(22, "", &Config::default(), &client).unwrap();
    assert_eq!(out.port, 22, "port must be set locally, not from model");
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let _ = run(80, "", &Config::default(), &client);
    assert!(client.last_request().max_tokens <= 512);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let out = run(22, "", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let out = run(22, "", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
