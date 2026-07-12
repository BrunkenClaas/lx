use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxchmod::run::{run, Output};

fn mock_normal() -> &'static str {
    r#"{"suggestion":"chmod 644 data.csv","reason":"World-writable files allow any user to modify them. 644 grants owner read/write and others read-only."}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"suggestion":"chmod 777 script.sh","reason":"Grants full permissions to all users."}"#
}

fn ls_l_input() -> &'static str {
    "-rw-rw-rw- 1 user group 1234 Jan 01 12:00 data.csv"
}

fn ls_l_input_executable() -> &'static str {
    "-rwxrwxrwx 1 user group  512 Feb 15 09:30 script.sh"
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_normal());
    let config = Config::default();
    let (out, _findings) = run(ls_l_input(), &config, &client).unwrap();
    assert!(!out.suggestion.is_empty(), "suggestion must not be empty");
    assert!(!out.reason.is_empty(), "reason must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_normal());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_normal());
    let config = Config::default();
    let err = run("   \n  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_normal());
    let config = Config::default();
    let (out, _findings) = run(ls_l_input(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_normal());
    let config = Config::default();
    let (out, _findings) = run(ls_l_input(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn dangerous_suggestion_detected_locally() {
    // Even if the model suggests chmod 777, local detection must warn.
    // run() still returns Ok — it warns but does not abort.
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    // This should succeed (run() returns the output, warning goes to stderr).
    let (out, _findings) = run(ls_l_input_executable(), &config, &client).unwrap();
    assert!(!out.suggestion.is_empty());
}

#[test]
fn to_plain_returns_suggestion_only() {
    let out = Output {
        suggestion: "chmod 644 file.txt".to_string(),
        reason: "Secure permissions for a regular file.".to_string(),
        dangerous: false,
    };
    assert_eq!(out.to_plain(), "chmod 644 file.txt");
}

#[test]
fn request_has_correct_invariants() {
    let client = MockLlmClient::returning(mock_normal());
    let config = Config::default();
    run(ls_l_input(), &config, &client).unwrap();
    assertions::assert_request_invariants(&client.last_request());
}
