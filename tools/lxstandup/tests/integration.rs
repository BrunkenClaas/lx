use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxstandup::run::run;

fn mock_response() -> &'static str {
    r#"{"done":["Added export endpoint for reports","Fixed null pointer in data parser"],"next":["Review open pull requests"],"blockers":[]}"#
}

fn sample_input() -> &'static str {
    include_str!("fixtures/git_log.txt")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_input(), &config, &client).unwrap();
    assert!(!out.done.is_empty(), "done list should not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn secrets_never_reach_llm() {
    let input_with_sensitive = include_str!("fixtures/git_log_with_sensitive.txt");
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(input_with_sensitive, &config, &client);
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
    let err = run("   \n\t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_input(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_input(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn empty_blockers_renders_none() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_input(), &config, &client).unwrap();
    let plain = out.to_plain();
    assert!(
        plain.contains("Blockers:\n(none)"),
        "empty blockers should show (none): {}",
        plain
    );
}

#[test]
fn blockers_render_when_present() {
    let resp = r#"{"done":["Completed task A"],"next":["Start task B"],"blockers":["Waiting on design approval"]}"#;
    let client = MockLlmClient::returning(resp);
    let config = Config::default();
    let out = run(sample_input(), &config, &client).unwrap();
    let plain = out.to_plain();
    assert!(
        plain.contains("- Waiting on design approval"),
        "blockers should be listed: {}",
        plain
    );
}
