use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxdraft::run::{run, run_no_redact};

fn mock_email_response() -> &'static str {
    "{\"subject\":\"Meeting rescheduled to Friday\",\"body\":\"Hi team,\\n\\nI wanted to let you know that the meeting has been rescheduled to Friday at 3:00 PM. Please update your calendars accordingly.\\n\\nBest regards\"}"
}

fn mock_ticket_response() -> &'static str {
    "{\"subject\":\"Search results page fails to load\",\"body\":\"The search results page returns a blank screen when more than 50 results are returned.\\n\\nSteps to reproduce:\\n1. Enter a broad search term.\\n2. Observe the results page.\\n\\nExpected: Results are displayed.\\nActual: Blank screen.\"}"
}

fn mock_message_response() -> &'static str {
    "{\"subject\":null,\"body\":\"Thanks for the update! Will review and get back to you by end of day.\"}"
}

fn sample_email_input() -> &'static str {
    include_str!("fixtures/email_notes.txt")
}

#[test]
fn output_schema_is_valid_email() {
    let client = MockLlmClient::returning(mock_email_response());
    let config = Config::default();
    let out = run(sample_email_input(), "email", &config, &client).unwrap();
    assert!(!out.body.is_empty(), "body must not be empty");
    assert!(out.subject.is_some(), "email kind should have a subject");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn output_schema_is_valid_ticket() {
    let client = MockLlmClient::returning(mock_ticket_response());
    let config = Config::default();
    let out = run(
        "search page broken, 50+ results, blank screen",
        "ticket",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.body.is_empty(), "body must not be empty");
    assert!(out.subject.is_some(), "ticket kind should have a subject");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn output_schema_is_valid_message_null_subject() {
    let client = MockLlmClient::returning(mock_message_response());
    let config = Config::default();
    let out = run(
        "thanks for update, will review by eod",
        "message",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.body.is_empty(), "body must not be empty");
    assert!(
        out.subject.is_none(),
        "message kind should have null subject"
    );
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn secrets_never_reach_llm() {
    let input = include_str!("fixtures/input_with_secret.txt");
    let client = MockLlmClient::returning(mock_email_response());
    let config = Config::default();
    let _ = run(input, "email", &config, &client);
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_email_response());
    let config = Config::default();
    let err = run("", "email", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_email_response());
    let config = Config::default();
    let err = run("   \n\t  ", "email", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn run_no_redact_works() {
    let client = MockLlmClient::returning(mock_email_response());
    let config = Config::default();
    let out = run_no_redact(sample_email_input(), "email", &config, &client).unwrap();
    assert!(!out.body.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn snapshot_plain_body() {
    let client = MockLlmClient::returning(mock_email_response());
    let config = Config::default();
    let out = run(sample_email_input(), "email", &config, &client).unwrap();
    insta::assert_snapshot!(out.body);
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_email_response());
    let config = Config::default();
    let out = run(sample_email_input(), "email", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
