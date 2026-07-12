use lx_config::Config;
use lx_testkit::mock::MockLlmClient;
use lxhttp::run::run;

#[test]
fn output_schema_is_valid() {
    let json = r#"{"explanation":"401 due to bad token.","status":401,"likely_cause":"Expired token.","suggested_fix":"Re-authenticate."}"#;
    let client = MockLlmClient::returning(json);
    let out = run("HTTP 401 output", &Config::default(), &client).unwrap();
    assert!(!out.explanation.is_empty());
    assert_eq!(out.status, 401);
    lx_testkit::assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_error() {
    let client = MockLlmClient::returning("{}");
    let result = run("", &Config::default(), &client);
    assert!(result.is_err());
}

#[test]
fn snapshot_plain_output() {
    let json = r#"{"explanation":"401 due to bad token.","status":401,"likely_cause":"Expired token.","suggested_fix":"Re-authenticate."}"#;
    let client = MockLlmClient::returning(json);
    let out = run("HTTP output", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let json = r#"{"explanation":"401 due to bad token.","status":401,"likely_cause":"Expired token.","suggested_fix":"Re-authenticate."}"#;
    let client = MockLlmClient::returning(json);
    let out = run("HTTP output", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
