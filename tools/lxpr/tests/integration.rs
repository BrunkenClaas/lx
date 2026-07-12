use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxpr::run::run;

fn mock_response() -> &'static str {
    "{\"title\":\"add user profile update endpoint\",\"body\":\"Summary: adds PATCH /users/id/profile endpoint. Only the account owner may update their own profile. Changes: new update_profile handler with validation, register new PATCH route, success and forbidden tests. Test Plan: run cargo test, verify PATCH updates display name, verify 403 when updating another users profile\"}"
}

fn sample_diff() -> &'static str {
    include_str!("fixtures/sample_pr.diff")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_diff(), &config, &client).unwrap();
    assert!(!out.title.is_empty(), "title must not be empty");
    assert!(!out.body.is_empty(), "body must not be empty");
    assert!(
        out.title.len() <= 72,
        "title too long: {} chars",
        out.title.len()
    );
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn secrets_never_reach_llm() {
    let diff_with_secret = include_str!("fixtures/diff_with_secret.diff");
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    // run() redacts before the LLM sees any content
    let _ = run(diff_with_secret, &config, &client);
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
    let (out, _warnings) = run(sample_diff(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_diff(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_contains_title_and_body() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(sample_diff(), &config, &client).unwrap();
    let plain = out.to_plain();
    assert!(
        plain.contains(&out.title),
        "plain output must contain title"
    );
    assert!(plain.contains(&out.body), "plain output must contain body");
    // Title comes before body
    let title_pos = plain.find(&out.title).unwrap();
    let body_pos = plain.find(&out.body).unwrap();
    assert!(
        title_pos < body_pos,
        "title must precede body in plain output"
    );
}
