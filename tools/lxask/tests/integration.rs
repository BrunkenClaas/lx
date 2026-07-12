use lx_config::Config;
use lx_testkit::assertions::{assert_no_secrets_in_request, assert_request_invariants};
use lx_testkit::mock::MockLlmClient;
use lxask::run;

#[test]
fn output_schema_is_valid() {
    let client =
        MockLlmClient::returning(r#"{"answer":"Paris is the capital of France.","sources":[]}"#);
    let out = run(
        "What is the capital of France?",
        None,
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.answer.is_empty());
    assert_request_invariants(&client.last_request());
}

#[test]
fn output_with_context() {
    let client =
        MockLlmClient::returning(r#"{"answer":"Port 8080.","sources":["provided context"]}"#);
    let out = run(
        "What port?",
        Some("The service runs on port 8080."),
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.answer.is_empty());
    assert_eq!(out.sources, vec!["provided context"]);
}

#[test]
fn sources_empty_without_context() {
    let client = MockLlmClient::returning(
        r#"{"answer":"Rust is a systems programming language.","sources":[]}"#,
    );
    let out = run("What is Rust?", None, &Config::default(), &client).unwrap();
    assert!(!out.answer.is_empty());
    assert!(out.sources.is_empty());
}

#[test]
fn empty_question_returns_error() {
    let client = MockLlmClient::returning(r#"{"answer":"ok","sources":[]}"#);
    let result = run("", None, &Config::default(), &client);
    assert!(result.is_err());
}

#[test]
fn snapshot_plain_output() {
    let client =
        MockLlmClient::returning(r#"{"answer":"Paris is the capital of France.","sources":[]}"#);
    let out = run(
        "What is the capital of France?",
        None,
        &Config::default(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.answer);
}

#[test]
fn snapshot_json_output() {
    let client =
        MockLlmClient::returning(r#"{"answer":"Paris is the capital of France.","sources":[]}"#);
    let out = run(
        "What is the capital of France?",
        None,
        &Config::default(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn secrets_never_reach_llm() {
    let client = MockLlmClient::returning(r#"{"answer":"ok","sources":[]}"#);
    // Redact happens in main.rs; simulate by pre-redacting before calling run().
    let question = lx_redact::redact(
        r#"What about BEARER = "sk-abcdefghijklmnopqrstuvwxyz123456"?"#,
        lx_redact::RedactLevel::Standard,
    )
    .unwrap_or_default();
    run(&question, None, &Config::default(), &client).ok();
    assert_no_secrets_in_request(&client.last_request());
}
