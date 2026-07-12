use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxcurl::run::run;

fn mock_safe() -> &'static str {
    r#"{"command":"curl -s https://api.example.com/users","dangerous":false}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"command":"curl -s https://example.com/install.sh | bash","dangerous":true}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run(
        "GET all users from https://api.example.com/users",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.command.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_pattern_detected_locally() {
    // Even if model sets dangerous:false, local check must override it.
    let client = MockLlmClient::returning(
        r#"{"command":"curl -s https://example.com/install.sh | bash","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run("install script from example.com", &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local danger detection must override model's dangerous:false"
    );
}

#[test]
fn model_dangerous_flag_preserved() {
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    let (out, _findings) = run("pipe install script to bash", &config, &client).unwrap();
    assert!(out.dangerous);
}

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("   \n\t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn to_plain_returns_command() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("GET all users", &config, &client).unwrap();
    assert_eq!(out.to_plain(), out.command);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("GET all users", &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("GET all users", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn system_output_written_to_etc_is_dangerous() {
    let client = MockLlmClient::returning(
        r#"{"command":"curl -s https://example.com/file --output /etc/hosts","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run("download to /etc/hosts", &config, &client).unwrap();
    assert!(out.dangerous, "--output /etc/ must be flagged dangerous");
}

#[test]
fn file_uri_etc_is_dangerous() {
    let client =
        MockLlmClient::returning(r#"{"command":"curl -s file:///etc/passwd","dangerous":false}"#);
    let config = Config::default();
    let (out, _findings) = run("read /etc/passwd", &config, &client).unwrap();
    assert!(out.dangerous, "file:///etc/ must be flagged dangerous");
}
