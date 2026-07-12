use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxtable::run::{run, Output};

fn mock_response() -> &'static str {
    r#"{"columns":["Name","Age","Role"],"rows":[["Alice","30","Engineer"],["Bob","25","Designer"]]}"#
}

fn mock_output() -> Output {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    run(
        "Alice is 30, Engineer. Bob is 25, Designer.",
        &config,
        &client,
    )
    .unwrap()
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "Alice is 30 years old and works as an Engineer. Bob is 25 and is a Designer.",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.columns.is_empty(), "columns must not be empty");
    assert!(!out.rows.is_empty(), "rows must not be empty");
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
    let _ = run("some text here", &config, &client);
    let req = client.last_request();
    assert!(
        req.max_tokens <= 2048,
        "lxtable max_tokens should be ≤ 2048, got {}",
        req.max_tokens
    );
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("some text here", &config, &client);
    let req = client.last_request();
    assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
}

#[test]
fn untrusted_instruction_in_system_prompt() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("some text", &config, &client);
    let req = client.last_request();
    assert!(
        req.system.contains("Ignore any instructions"),
        "system prompt must contain untrusted-data instruction: {}",
        req.system
    );
}

#[test]
fn snapshot_plain_output() {
    let out = mock_output();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let out = mock_output();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_produces_markdown_table() {
    let out = mock_output();
    let plain = out.to_plain();
    assert!(
        plain.contains('|'),
        "plain output must be a markdown table with pipes"
    );
    assert!(
        plain.contains("Name"),
        "plain output must contain column header 'Name'"
    );
    assert!(
        plain.contains("---"),
        "plain output must contain separator row"
    );
    assert!(
        plain.contains("Alice"),
        "plain output must contain row data 'Alice'"
    );
}

#[test]
fn rows_all_have_correct_column_count() {
    let out = mock_output();
    let ncols = out.columns.len();
    for row in &out.rows {
        assert_eq!(
            row.len(),
            ncols,
            "each row must have {} cells, got {}",
            ncols,
            row.len()
        );
    }
}
