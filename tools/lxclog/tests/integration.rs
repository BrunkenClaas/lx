use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxclog::run::run;

fn mock_response() -> &'static str {
    r#"{"entries":[{"version":"Unreleased","date":"","added":["Add token refresh endpoint","Add --json output flag to all commands"],"changed":["Bump serde to 1.0.200","Extract whitespace-skipping helper in parser"],"fixed":["Handle null response from upstream","Resolve config file not found on Windows"]}]}"#
}

fn sample_log() -> &'static str {
    include_str!("fixtures/sample_git_log.txt")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_log(), &config, &client).unwrap();
    assert!(!out.entries.is_empty(), "entries must not be empty");
    let first = &out.entries[0];
    assert!(!first.version.is_empty(), "version must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn entries_have_at_least_one_change() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_log(), &config, &client).unwrap();
    let first = &out.entries[0];
    let total_changes = first.added.len()
        + first.changed.len()
        + first.deprecated.len()
        + first.removed.len()
        + first.fixed.len()
        + first.security.len();
    assert!(
        total_changes > 0,
        "each entry must have at least one change"
    );
}

#[test]
fn secrets_never_reach_llm() {
    let log_with_secret = include_str!("fixtures/git_log_with_secret.txt");
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    // Run should succeed (redaction replaces the secret)
    let _ = run(log_with_secret, &config, &client);
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   \n\t\n   ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_log(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_log(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn plain_output_contains_changelog_header() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_log(), &config, &client).unwrap();
    let plain = out.to_plain();
    assert!(
        plain.contains("# Changelog"),
        "plain output must start with # Changelog"
    );
}

#[test]
fn plain_output_contains_version_heading() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_log(), &config, &client).unwrap();
    let plain = out.to_plain();
    assert!(
        plain.contains("## [Unreleased]"),
        "plain output must contain version heading"
    );
}

#[test]
fn empty_entries_response_returns_logical_error() {
    let client = MockLlmClient::returning(r#"{"entries":[]}"#);
    let config = Config::default();
    let err = run(sample_log(), &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::LOGICAL_ERROR);
}
