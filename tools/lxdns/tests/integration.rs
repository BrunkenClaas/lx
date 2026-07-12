use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxdns::run::run;

fn mock_nxdomain() -> &'static str {
    r#"{"explanation":"NXDOMAIN returned.","likely_cause":"Domain does not exist.","suggested_fix":"Check spelling."}"#
}

fn mock_healthy() -> &'static str {
    r#"{"explanation":"DNS resolution succeeded with a valid A record.","likely_cause":"none","suggested_fix":"none"}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_nxdomain());
    let out = run("dig output here", "", &Config::default(), &client).unwrap();
    assert!(!out.explanation.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_error() {
    let client = MockLlmClient::returning("{}");
    let result = run("", "", &Config::default(), &client);
    assert!(result.is_err());
}

#[test]
fn domain_arg_included_in_request() {
    let client = MockLlmClient::returning(mock_healthy());
    let _ = run("dig output", "api.example.com", &Config::default(), &client);
    let req = client.last_request();
    assert!(req.user.contains("api.example.com"));
}

#[test]
fn no_domain_omits_domain_prefix() {
    let client = MockLlmClient::returning(mock_nxdomain());
    let _ = run("dig output here", "", &Config::default(), &client);
    let req = client.last_request();
    assert!(!req.user.contains("Domain:"));
}

#[test]
fn snapshot_plain_output() {
    let json = r#"{"explanation":"NXDOMAIN returned.","likely_cause":"Domain not found.","suggested_fix":"Check the domain name."}"#;
    let client = MockLlmClient::returning(json);
    let out = run("dig output", "", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let json = r#"{"explanation":"NXDOMAIN returned.","likely_cause":"Domain not found.","suggested_fix":"Check the domain name."}"#;
    let client = MockLlmClient::returning(json);
    let out = run("dig output", "", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
