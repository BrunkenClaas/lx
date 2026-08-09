use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxdebug::run::{run, Output};

fn mock_response() -> &'static str {
    r#"{"cause":"The application cannot find its configuration file.","fix":"Create the missing config file or verify the path.","command":"cp config.json.example config.json"}"#
}

fn mock_response_no_command() -> &'static str {
    r#"{"cause":"Out of memory error in JVM heap space.","fix":"Increase JVM heap size with the -Xmx flag.","command":""}"#
}

fn enoent_error() -> &'static str {
    include_str!("fixtures/enoent_error.txt")
}

fn error_with_secret() -> &'static str {
    include_str!("fixtures/error_with_secret.txt")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(enoent_error(), &config, &client).unwrap();
    assert!(!out.cause.is_empty(), "cause must not be empty");
    assert!(!out.fix.is_empty(), "fix must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn command_field_may_be_empty_string() {
    let client = MockLlmClient::returning(mock_response_no_command());
    let config = Config::default();
    let (out, _warnings) = run(enoent_error(), &config, &client).unwrap();
    assert!(out.command.is_empty());
    assert!(!out.cause.is_empty());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   \n\t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn secrets_never_reach_llm() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(error_with_secret(), &config, &client);
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(enoent_error(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(enoent_error(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_omits_run_line_when_command_empty() {
    let out = Output {
        cause: "Some error".to_string(),
        fix: "Do something".to_string(),
        command: String::new(),
        input_truncated: false,
    };
    let plain = out.to_plain();
    assert!(
        !plain.contains("Run:"),
        "plain output must not contain Run: when command is empty"
    );
}

#[test]
fn to_plain_includes_run_line_when_command_present() {
    let out = Output {
        cause: "Some error".to_string(),
        fix: "Do something".to_string(),
        command: "npm install".to_string(),
        input_truncated: false,
    };
    let plain = out.to_plain();
    assert!(plain.contains("Run:    npm install"));
}

#[test]
fn oversized_multibyte_input_does_not_panic() {
    // The inner cap used to slice at a raw byte offset, which panics when the
    // offset falls inside a multi-byte character. "ä" is two bytes, so a bare
    // run of them puts every character on an even offset and an even cap would
    // land cleanly between two of them. The single-byte "x" prefix shifts every
    // "ä" onto an odd offset, so the cap is guaranteed to fall mid-character.
    let input = format!("x{}", "ä".repeat(MAX_INPUT_BYTES_FOR_TEST / 2 + 100));
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (_out, warnings) = run(&input, &config, &client).unwrap();
    assert!(
        warnings.iter().any(|w| w.contains("truncated")),
        "oversized input must report truncation"
    );
}

/// Mirrors `run::MAX_INPUT_BYTES`, which is private.
const MAX_INPUT_BYTES_FOR_TEST: usize = 32000;
