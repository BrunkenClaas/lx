use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxmakefile::run::run;

const MOCK_SAFE: &str = r#"{"content":".PHONY: build\nbuild:\n\tcargo build --release"}"#;

const MOCK_DANGEROUS: &str = r#"{"content":".PHONY: nuke\nnuke:\n\trm -rf / --no-preserve-root"}"#;

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(MOCK_SAFE);
    let config = Config::default();
    let (out, _findings) = run("build a Rust project", None, &config, &client).unwrap();
    assert!(!out.content.is_empty(), "content must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(MOCK_SAFE);
    let config = Config::default();
    let (out, _findings) = run("build a Rust project", None, &config, &client).unwrap();
    insta::assert_snapshot!(out.content);
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(MOCK_SAFE);
    let config = Config::default();
    let (out, _findings) = run("build a Rust project", None, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn dangerous_content_flagged() {
    let client = MockLlmClient::returning(MOCK_DANGEROUS);
    let config = Config::default();
    let (out, _findings) = run("delete everything", None, &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local danger detection must flag rm -rf / in generated content"
    );
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(MOCK_SAFE);
    let config = Config::default();
    let err = run("", None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn safe_content_not_flagged() {
    let client = MockLlmClient::returning(MOCK_SAFE);
    let config = Config::default();
    let (out, _findings) = run("build a Rust project", None, &config, &client).unwrap();
    assert!(
        !out.dangerous,
        "safe content must not be flagged as dangerous"
    );
}

#[test]
fn edit_mode_user_message_contains_existing_content() {
    let existing = ".PHONY: build\nbuild:\n\tcargo build --release";
    let client = MockLlmClient::returning(MOCK_SAFE);
    let config = Config::default();
    let _out = run("add a test target", Some(existing), &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit the following Makefile/justfile"),
        "edit mode must include edit instruction in user message"
    );
    assert!(
        req.user.contains("cargo build"),
        "edit mode must include existing content"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn create_mode_user_message_is_plain_intent() {
    let client = MockLlmClient::returning(MOCK_SAFE);
    let config = Config::default();
    let _out = run("build a Rust project", None, &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        !req.user.contains("Edit the following"),
        "create mode must not include edit instruction"
    );
}
