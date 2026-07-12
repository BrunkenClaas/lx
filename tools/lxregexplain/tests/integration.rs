use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxregexplain::run::run;

fn mock_response() -> &'static str {
    "{\"regex\":\"^\\\\d+$\",\"explanation\":\"Matches one or more digits anchored to the full string.\",\"parts\":[{\"token\":\"^\",\"means\":\"Start of string anchor\"},{\"token\":\"\\\\d+\",\"means\":\"One or more digit characters\"},{\"token\":\"$\",\"means\":\"End of string anchor\"}]}"
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(r"^\d+$", &config, &client).unwrap();
    assert!(!out.explanation.is_empty(), "explanation must not be empty");
    assert!(!out.regex.is_empty(), "regex must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn parts_are_populated() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(r"^\d+$", &config, &client).unwrap();
    assert!(!out.parts.is_empty(), "parts must not be empty");
    assert!(!out.parts[0].token.is_empty(), "token must not be empty");
    assert!(!out.parts[0].means.is_empty(), "means must not be empty");
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
    let _ = run(r"^\d+$", &config, &client);
    let req = client.last_request();
    assert!(
        req.max_tokens <= 512,
        "lxregexplain max_tokens should be <= 512, got {}",
        req.max_tokens
    );
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(r"^\d+$", &config, &client).unwrap();
    insta::assert_snapshot!(out.explanation);
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(r"^\d+$", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
