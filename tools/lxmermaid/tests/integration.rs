use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxmermaid::run::run;

fn mock_diagram() -> &'static str {
    r#"{"diagram":"flowchart TD\n    A[Start] --> B[End]"}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_diagram());
    let config = Config::default();
    let (out, _findings) = run("simple start to end flow", None, &config, &client).unwrap();
    assert!(!out.diagram.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_diagram());
    let config = Config::default();
    let err = run("", None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_diagram());
    let config = Config::default();
    let err = run("   \n\t  ", None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn empty_diagram_returns_error() {
    let client = MockLlmClient::returning(r#"{"diagram":""}"#);
    let config = Config::default();
    let err = run("describe a flow", None, &config, &client).unwrap_err();
    assert!(
        err.exit_code() != lx_core::exit::SUCCESS,
        "empty diagram must yield an error"
    );
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_diagram());
    let config = Config::default();
    let (out, _findings) = run("simple flow", None, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_diagram());
    let config = Config::default();
    let (out, _findings) = run("simple flow", None, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn edit_mode_user_message_contains_existing_diagram() {
    let existing = "flowchart TD\n    A[Start] --> B[End]";
    let client = MockLlmClient::returning(mock_diagram());
    let config = Config::default();
    let _out = run(
        "add a middle step C between A and B",
        Some(existing),
        &config,
        &client,
    )
    .unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit the following Mermaid diagram"),
        "edit mode must include edit instruction, got: {}",
        req.user
    );
    assert!(
        req.user.contains("flowchart TD"),
        "edit mode must include existing diagram in user message"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn create_mode_user_message_does_not_contain_edit_prefix() {
    let client = MockLlmClient::returning(mock_diagram());
    let config = Config::default();
    let _out = run("a simple flow", None, &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        !req.user.contains("Edit the following Mermaid diagram"),
        "create mode must NOT include edit instruction"
    );
}

#[test]
fn sequence_diagram_is_accepted() {
    let mock = r#"{"diagram":"sequenceDiagram\n    A->>B: hello\n    B-->>A: world"}"#;
    let client = MockLlmClient::returning(mock);
    let config = Config::default();
    let (out, _findings) = run("A sends hello to B and B replies", None, &config, &client).unwrap();
    assert!(out.diagram.contains("sequenceDiagram"));
}
