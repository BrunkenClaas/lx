use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxundo::run;

fn mock_safe() -> &'static str {
    r#"{"undo_command":"git checkout HEAD -- file.txt","caution":"Restores the file from HEAD"}"#
}

fn mock_empty_caution() -> &'static str {
    r#"{"undo_command":"mv /tmp/backup.txt .","caution":""}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let out = run("rm file.txt", &config, &client).unwrap();
    assert!(!out.undo_command.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("   \n  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn caution_field_can_be_empty() {
    let client = MockLlmClient::returning(mock_empty_caution());
    let config = Config::default();
    let out = run("mv file.txt /tmp/backup.txt", &config, &client).unwrap();
    assert!(!out.undo_command.is_empty());
    assert!(out.caution.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let out = run("rm file.txt", &config, &client).unwrap();
    insta::assert_snapshot!(out.undo_command.clone());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let out = run("rm file.txt", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
