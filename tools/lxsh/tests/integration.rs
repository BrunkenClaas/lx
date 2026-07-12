use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxsh::run::run;

fn mock_safe() -> &'static str {
    r#"{"command":"ls -lat","shell":"bash","dangerous":false}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"command":"rm -rf /","shell":"bash","dangerous":true}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, findings) = run("list files by modification time", &config, &client).unwrap();
    assert!(!out.command.is_empty());
    assert!(!out.shell.is_empty());
    assert!(findings.is_empty(), "safe command must yield no findings");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_pattern_is_detected_locally() {
    // Even if the model says dangerous:false, our local check overrides it.
    let client =
        MockLlmClient::returning(r#"{"command":"rm -rf /","shell":"bash","dangerous":false}"#);
    let config = Config::default();
    let (out, findings) = run("delete everything", &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local danger detection must override model's dangerous:false"
    );
    assert!(
        !findings.is_empty(),
        "danger check must return the matched findings for main.rs to emit"
    );
}

#[test]
fn model_dangerous_flag_preserved() {
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    let (out, _findings) = run("delete root filesystem", &config, &client).unwrap();
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
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("list files", &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("list files", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
