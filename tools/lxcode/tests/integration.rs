use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxcode::run::run;

// ── Fixture helpers ──────────────────────────────────────────────────────────

fn mock_rust() -> &'static str {
    r#"{"code":"fn add(a: i32, b: i32) -> i32 { a + b }","language":"rust"}"#
}

fn mock_python() -> &'static str {
    r#"{"code":"def greet(name):\n    print(f'Hello, {name}!')","language":"python"}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"code":"import shutil\nshutil.rmtree('/tmp/target')","language":"python"}"#
}

// ── Schema & invariants ──────────────────────────────────────────────────────

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_rust());
    let config = Config::default();
    let out = run("a function that adds two integers", None, &config, &client).unwrap();
    assert!(!out.code.is_empty());
    assert!(!out.language.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn lang_hint_is_forwarded_to_llm() {
    let client = MockLlmClient::returning(mock_python());
    let config = Config::default();
    let out = run("greet a user by name", Some("python"), &config, &client).unwrap();
    assert_eq!(out.language, "python");

    // The lang hint must appear in the user message sent to the LLM.
    let req = client.last_request();
    assert!(
        req.user.contains("python"),
        "user message should mention the target language"
    );
}

#[test]
fn auto_lang_hint_is_not_forwarded() {
    let client = MockLlmClient::returning(mock_rust());
    let config = Config::default();
    let _ = run("add two integers", None, &config, &client).unwrap();

    // When no lang_hint is given the user message is just the description.
    let req = client.last_request();
    assert_eq!(req.user.trim(), "add two integers");
}

// ── Error cases ──────────────────────────────────────────────────────────────

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_rust());
    let config = Config::default();
    let err = run("", None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_rust());
    let config = Config::default();
    let err = run("   \n  ", None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn empty_code_field_returns_logical_error() {
    let client = MockLlmClient::returning(r#"{"code":"","language":"rust"}"#);
    let config = Config::default();
    let err = run("add two numbers", None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::LOGICAL_ERROR);
}

#[test]
fn empty_language_field_returns_logical_error() {
    let client = MockLlmClient::returning(r#"{"code":"fn foo() {}","language":""}"#);
    let config = Config::default();
    let err = run("add two numbers", None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::LOGICAL_ERROR);
}

// ── §8.3 nocmd: danger detection ─────────────────────────────────────────────

#[test]
fn dangerous_pattern_in_code_does_not_abort() {
    // §8.3: dangerous code is printed with a warning, not rejected.
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    // run() must succeed (code is output, not executed) — danger is only warned.
    let out = run("delete the target directory", None, &config, &client).unwrap();
    assert!(!out.code.is_empty());
}

// ── Snapshots ────────────────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_rust());
    let config = Config::default();
    let out = run("a function that adds two integers", None, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_rust());
    let config = Config::default();
    let out = run("a function that adds two integers", None, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn snapshot_python_output() {
    let client = MockLlmClient::returning(mock_python());
    let config = Config::default();
    let out = run("greet a user", Some("python"), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

// ── System prompt ─────────────────────────────────────────────────────────────

#[test]
fn system_prompt_contains_lang_placeholder() {
    let client = MockLlmClient::returning(mock_rust());
    let config = Config::default();
    let _ = run("add two numbers", None, &config, &client).unwrap();
    assertions::assert_lang_placeholder_in_system(&client.last_request());
}
