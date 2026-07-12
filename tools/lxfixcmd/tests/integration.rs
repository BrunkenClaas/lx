use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxfixcmd::run::run;

fn mock_response() -> &'static str {
    r#"{"command":"git push origin main","reason":"'psh' is a typo for 'push'","dangerous":false}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let out = run("git psh origin main", "", &Config::default(), &client).unwrap();
    assert!(!out.command.is_empty());
    assert!(!out.reason.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let err = run("   ", "", &Config::default(), &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let _ = run("git psh", "", &Config::default(), &client);
    assert!(client.last_request().max_tokens <= 256);
}

#[test]
fn dangerous_command_flagged() {
    let dangerous = r#"{"command":"rm -rf /","reason":"fixed","dangerous":false}"#;
    let client = MockLlmClient::returning(dangerous);
    let out = run("rm -rf /", "", &Config::default(), &client).unwrap();
    assert!(out.dangerous, "should detect rm -rf / as dangerous");
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let out = run("git psh origin main", "", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let out = run("git psh origin main", "", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
