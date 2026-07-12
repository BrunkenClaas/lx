use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxmd::run::run;

fn mock_response() -> &'static str {
    "{\"markdown\":\"# Meeting Notes\\n\\n## Attendees\\n\\n- Alice\\n- Bob\\n\\n## Action Items\\n\\n- Alice will update the plan\"}"
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "meeting notes\nattendees: alice, bob\naction: alice will update the plan",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.markdown.is_empty(), "markdown must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("some raw text", &config, &client);
    let req = client.last_request();
    assert!(req.max_tokens <= 2048, "lxmd max_tokens should be <= 2048");
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("some raw text", &config, &client);
    let req = client.last_request();
    assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "meeting notes\nattendees: alice, bob\naction: alice will update the plan",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "meeting notes\nattendees: alice, bob\naction: alice will update the plan",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
