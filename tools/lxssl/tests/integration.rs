use lx_config::Config;
use lx_testkit::mock::MockLlmClient;
use lxssl::run::run;

#[test]
fn output_schema_is_valid() {
    let json = r#"{"explanation":"Certificate expired.","likely_cause":"Expired cert.","suggested_fix":"Renew it."}"#;
    let client = MockLlmClient::returning(json);
    let out = run("openssl output here", "", &Config::default(), &client).unwrap();
    assert!(!out.explanation.is_empty());
    lx_testkit::assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_error() {
    let client = MockLlmClient::returning("{}");
    let result = run("", "", &Config::default(), &client);
    assert!(result.is_err());
}

#[test]
fn host_arg_included_in_request() {
    let json = r#"{"explanation":"ok","likely_cause":"none","suggested_fix":"none"}"#;
    let client = MockLlmClient::returning(json);
    let _ = run(
        "openssl output",
        "api.example.com",
        &Config::default(),
        &client,
    );
    let req = client.last_request();
    assert!(req.user.contains("api.example.com"));
}

#[test]
fn snapshot_plain_output() {
    let json = r#"{"explanation":"Certificate has expired.","likely_cause":"Expired cert.","suggested_fix":"Run certbot renew."}"#;
    let client = MockLlmClient::returning(json);
    let out = run("openssl output", "", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let json = r#"{"explanation":"Certificate has expired.","likely_cause":"Expired cert.","suggested_fix":"Run certbot renew."}"#;
    let client = MockLlmClient::returning(json);
    let out = run("openssl output", "", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
