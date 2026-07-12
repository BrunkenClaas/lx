use lx_config::Config;
use lx_redact::RedactLevel;
use lx_testkit::{assertions, MockLlmClient};
use lxredact::run::run;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn config() -> Config {
    Config::default()
}

fn mock_explain_response() -> &'static str {
    r#"{"summary":"An API key was redacted.","categories":["api_key"],"risk_level":"high","notes":"Rotate the affected key immediately."}"#
}

// ── Schema / basic correctness ────────────────────────────────────────────────

#[test]
fn output_schema_is_valid_no_secrets() {
    let client = MockLlmClient::returning(mock_explain_response());
    let out = run(
        "cargo build --release",
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert_eq!(out.redacted_text, "cargo build --release");
    assert_eq!(out.redacted_count, 0);
    assert!(out.items.is_empty());
    assert!(out.explanation.is_none());
    // No LLM call should have been made (explain=false, no secrets).
    assert_eq!(client.call_count(), 0);
}

#[test]
fn empty_input_returns_empty_output() {
    let client = MockLlmClient::returning(mock_explain_response());
    let out = run("", RedactLevel::Standard, false, false, &config(), &client).unwrap();
    assert!(out.redacted_text.is_empty());
    assert_eq!(out.redacted_count, 0);
    assert!(out.items.is_empty());
}

// ── Redaction pattern tests ───────────────────────────────────────────────────

#[test]
fn redacts_openai_api_key() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "api_key=sk-abcdefghijklmnopqrstu12345";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert!(!out.redacted_text.contains("sk-abcdefghijklmnopqrstu12345"));
    assert!(out.redacted_text.contains("[REDACTED]"));
    assert!(out.redacted_count > 0);
}

#[test]
fn redacts_aws_access_key() {
    let client = MockLlmClient::returning(mock_explain_response());
    // Realistic high-entropy key. (AWS's own AKIAIOSFODNN7EXAMPLE is intentionally
    // a documentation example and is now correctly left untouched by the gate.)
    let input = "AWS_KEY=AKIAJ3MV4BNZC9X7PQRF";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert!(!out.redacted_text.contains("AKIAJ3MV4BNZC9X7PQRF"));
    assert!(out.redacted_text.contains("[REDACTED]"));
    assert!(out.redacted_count > 0);
}

#[test]
fn redacts_connection_string_password() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "DATABASE_URL=postgres://admin:s3cr3t_passw0rd@db.example.com/mydb";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert!(!out.redacted_text.contains("s3cr3t_passw0rd"));
    assert!(out.redacted_text.contains("[REDACTED]"));
    assert!(out.redacted_count > 0);
}

#[test]
fn redacts_private_key_block() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "-----BEGIN RSA PRIVATE KEY-----\nMIIE...\n-----END RSA PRIVATE KEY-----";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert!(!out.redacted_text.contains("MIIe"));
    assert!(out.redacted_text.contains("[REDACTED]"));
    assert!(out.redacted_count > 0);
}

#[test]
fn redacts_email_in_standard_mode() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "contact: alice@example.com";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert!(!out.redacted_text.contains("alice@example.com"));
    assert!(out.redacted_text.contains("[EMAIL]"));
    assert!(out.redacted_count > 0);
}

#[test]
fn strict_mode_redacts_ip_addresses() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "server at 192.168.1.100 is down";
    let out = run(input, RedactLevel::Strict, false, false, &config(), &client).unwrap();
    assert!(!out.redacted_text.contains("192.168.1.100"));
    assert!(out.redacted_text.contains("[IP]"));
}

#[test]
fn clean_text_passes_through_unchanged() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "Hello, world! This is a normal sentence with no secrets.";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert_eq!(out.redacted_text, input);
    assert_eq!(out.redacted_count, 0);
}

// ── --explain mode ────────────────────────────────────────────────────────────

#[test]
fn explain_mode_calls_llm_when_secrets_found() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "token=sk-abcdefghijklmnopqrstuvwxyz12345";
    let out = run(
        input,
        RedactLevel::Standard,
        true,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert!(out.redacted_count > 0);
    // LLM must have been called exactly once.
    assert_eq!(client.call_count(), 1);
    let explanation = out.explanation.expect("explanation should be present");
    assert!(!explanation.summary.is_empty());
    assert!(!explanation.risk_level.is_empty());
}

#[test]
fn explain_mode_does_not_call_llm_when_no_secrets() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "no secrets here at all";
    let out = run(
        input,
        RedactLevel::Standard,
        true,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert_eq!(out.redacted_count, 0);
    assert_eq!(
        client.call_count(),
        0,
        "LLM must not be called when nothing was redacted"
    );
    assert!(out.explanation.is_none());
}

#[test]
fn explain_llm_request_invariants() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "api_key=sk-abcdefghijklmnopqrstuvwxyz12345";
    let _ = run(
        input,
        RedactLevel::Standard,
        true,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn explain_llm_request_never_contains_secret_values() {
    let client = MockLlmClient::returning(mock_explain_response());
    // Use a realistic-looking key.
    let input = "api_key=sk-abcdefghijklmnopqrstuvwxyz12345";
    let _ = run(
        input,
        RedactLevel::Standard,
        true,
        false,
        &config(),
        &client,
    )
    .unwrap();
    let req = client.last_request();
    // The user prompt sent to the LLM must not contain the actual key value.
    assert!(
        !req.user.contains("sk-abcdefghijklmnopqrstuvwxyz12345"),
        "LLM must never receive the actual secret value; user field: {:?}",
        req.user
    );
}

// ── Security invariants ───────────────────────────────────────────────────────

#[test]
fn no_secrets_reach_llm_in_explain_mode() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "password=hunter2 and token sk-abcdefghijklmnopqrstu12345";
    let _ = run(
        input,
        RedactLevel::Standard,
        true,
        false,
        &config(),
        &client,
    )
    .unwrap();
    // Verify via the testkit assertion helper.
    assertions::assert_no_secrets_in_request(&client.last_request());
}

// ── Item location tracking ────────────────────────────────────────────────────

#[test]
fn items_have_location_hints() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "line1: normal\nline2: api_key=sk-abcdefghijklmnopqrstuvwxyz";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    assert!(out.redacted_count > 0);
    assert!(!out.items.is_empty());
    // The item must report a line number.
    assert!(
        out.items.iter().any(|i| i.location.starts_with("line")),
        "items should have line-based location hints: {:?}",
        out.items
    );
}

// ── Snapshot tests ────────────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output_with_secret() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "api_key=sk-abcdefghijklmnopqrstuvwxyz12345";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output_with_secret() {
    let client = MockLlmClient::returning(mock_explain_response());
    let input = "api_key=sk-abcdefghijklmnopqrstuvwxyz12345";
    let out = run(
        input,
        RedactLevel::Standard,
        false,
        false,
        &config(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
