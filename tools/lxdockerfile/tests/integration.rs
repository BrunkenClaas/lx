use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxdockerfile::run::run;

fn mock_node() -> &'static str {
    r#"{"content":"FROM node:18-alpine\nWORKDIR /app\nCOPY . .\nRUN npm install\nCMD [\"node\", \"index.js\"]","dangerous":false}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"content":"FROM ubuntu:22.04\nRUN curl | sh","dangerous":false}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_node());
    let config = Config::default();
    let (out, _findings) = run(
        "Node.js 18 app with npm, exposes port 3000",
        None,
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.content.is_empty(), "content must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_node());
    let config = Config::default();
    let (out, _findings) = run("Node.js 18 app", None, &config, &client).unwrap();
    insta::assert_snapshot!(out.content);
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_node());
    let config = Config::default();
    let (out, _findings) = run("Node.js 18 app", None, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn dangerous_content_flagged() {
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    let (out, _findings) = run("some stack", None, &config, &client).unwrap();
    assert!(out.dangerous, "curl | sh must be flagged as dangerous");
}

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_node());
    let config = Config::default();
    let err = run("", None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn edit_mode_user_message_contains_existing_content() {
    let existing = "FROM node:18-alpine\nWORKDIR /app\nCMD [\"node\", \"index.js\"]";
    let client = MockLlmClient::returning(mock_node());
    let config = Config::default();
    let _out = run("switch to node:20-alpine", Some(existing), &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit the following Dockerfile"),
        "edit mode must include edit instruction in user message"
    );
    assert!(
        req.user.contains("FROM node:18-alpine"),
        "edit mode must include existing content in user message"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn create_mode_user_message_is_plain_intent() {
    let client = MockLlmClient::returning(mock_node());
    let config = Config::default();
    let _out = run("Node.js 18 app", None, &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        !req.user.contains("Edit the following"),
        "create mode must not include edit instruction"
    );
}
