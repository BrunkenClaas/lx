use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxjq::run::run;

fn mock_simple() -> &'static str {
    r#"{"expression":".users[].name","explanation":"Extracts the name field from each element of the users array"}"#
}

fn mock_with_dangerous() -> &'static str {
    r#"{"expression":".data | @sh","explanation":"Formats data as a shell-escaped string"}"#
}

// ---------------------------------------------------------------------------
// Schema / invariant tests
// ---------------------------------------------------------------------------

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let (out, _findings) = run(
        "extract names from users array",
        None,
        None,
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.expression.is_empty(), "expression must not be empty");
    assert!(!out.explanation.is_empty(), "explanation must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn output_schema_with_json_context() {
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let ctx = r#"{"users":[{"name":"Alice","active":true}]}"#;
    let (out, _findings) = run(
        "extract names from users array",
        Some(ctx),
        None,
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.expression.is_empty());
    // The user message sent to the LLM should include the JSON context.
    let req = client.last_request();
    assert!(
        req.user.contains("JSON context"),
        "user message must embed JSON context when provided"
    );
}

// ---------------------------------------------------------------------------
// Danger detection (§8.3 nocmd)
// ---------------------------------------------------------------------------

#[test]
fn dangerous_pattern_is_detected_locally() {
    let client = MockLlmClient::returning(mock_with_dangerous());
    let config = Config::default();
    let (out, _findings) =
        run("format data as shell string", None, None, &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local danger detection must flag @sh expressions"
    );
}

#[test]
fn safe_expression_not_flagged() {
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let (out, _findings) = run("get user names", None, None, &config, &client).unwrap();
    assert!(!out.dangerous, "safe expression must not be flagged");
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let err = run("", None, None, &config, &client).unwrap_err();
    assert_eq!(
        err.exit_code(),
        lx_core::exit::BAD_USAGE,
        "empty description must produce exit code BAD_USAGE"
    );
}

#[test]
fn whitespace_only_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let err = run("   \n\t  ", None, None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

// ---------------------------------------------------------------------------
// Edit mode
// ---------------------------------------------------------------------------

#[test]
fn edit_mode_user_message_contains_existing_expression() {
    let existing = ".users[].name";
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let _out = run(
        "also filter out null values",
        None,
        Some(existing),
        &config,
        &client,
    )
    .unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit the following jq expression"),
        "edit mode must include edit instruction, got: {}",
        req.user
    );
    assert!(
        req.user.contains(existing),
        "edit mode must include existing expression in user message"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn create_mode_user_message_does_not_contain_edit_prefix() {
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let _out = run("extract user names", None, None, &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        !req.user.contains("Edit the following jq expression"),
        "create mode must NOT contain edit instruction"
    );
}

// ---------------------------------------------------------------------------
// Snapshot tests
// ---------------------------------------------------------------------------

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let (out, _findings) = run(
        "extract names from users array",
        None,
        None,
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_simple());
    let config = Config::default();
    let (out, _findings) = run(
        "extract names from users array",
        None,
        None,
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
