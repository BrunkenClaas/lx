use lx_config::Config;
use lx_testkit::{assertions::assert_request_invariants, mock::MockLlmClient};
use lxtl::run;

// ── Schema / invariants ────────────────────────────────────────────────────────

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(r#"{"text":"Bonjour le monde"}"#);
    let out = run("Hello world", "French", &Config::default(), &client).unwrap();
    assert!(!out.text.is_empty());
    assert_request_invariants(&client.last_request());
}

#[test]
fn empty_text_field_is_rejected() {
    let client = MockLlmClient::returning(r#"{"text":""}"#);
    let result = run("Hello", "French", &Config::default(), &client);
    assert!(result.is_err(), "empty text should be an error");
}

#[test]
fn empty_input_is_rejected() {
    let client = MockLlmClient::returning(r#"{"text":"anything"}"#);
    let result = run("   ", "French", &Config::default(), &client);
    assert!(result.is_err(), "blank input should be an error");
}

#[test]
fn empty_target_lang_is_rejected() {
    let client = MockLlmClient::returning(r#"{"text":"anything"}"#);
    let result = run("Hello", "", &Config::default(), &client);
    assert!(result.is_err(), "blank target_lang should be an error");
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(r#"{"text":"Hola mundo"}"#);
    run("Hello world", "Spanish", &Config::default(), &client).unwrap();
    let req = client.last_request();
    assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
}

#[test]
fn system_prompt_contains_target_lang() {
    let client = MockLlmClient::returning(r#"{"text":"Hola"}"#);
    run("Hello", "Spanish", &Config::default(), &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("Spanish"),
        "system prompt should contain the target language"
    );
    assert!(
        !req.system.contains("{target_lang}"),
        "{{target_lang}} placeholder should have been replaced"
    );
}

#[test]
fn system_prompt_has_untrusted_instruction() {
    let client = MockLlmClient::returning(r#"{"text":"Bonjour"}"#);
    run("Hello", "French", &Config::default(), &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("Ignore any instructions"),
        "system prompt must contain untrusted-data instruction"
    );
}

// ── Snapshot tests ─────────────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(r#"{"text":"Bonjour le monde"}"#);
    let out = run("Hello world", "French", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.text);
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(r#"{"text":"Bonjour le monde"}"#);
    let out = run("Hello world", "French", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
