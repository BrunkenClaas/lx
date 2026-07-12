use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxpull::run::{run, run_no_redact, Output};

fn mock_response() -> &'static str {
    "{\"records\":[{\"name\":\"Alice Johnson\",\"email\":\"alice.johnson@acme.com\"},{\"name\":\"Bob Smith\",\"email\":\"bob.smith@acme.com\"}]}"
}

fn sample_contacts() -> &'static str {
    include_str!("fixtures/contacts.txt")
}

fn sample_fields() -> Vec<String> {
    vec!["name".to_string(), "email".to_string()]
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let fields = sample_fields();
    let out = run(sample_contacts(), &fields, &config, &client).unwrap();
    assert!(!out.records.is_empty(), "records must not be empty");
    for record in &out.records {
        assert!(
            record.contains_key("name"),
            "each record must have 'name' field"
        );
        assert!(
            record.contains_key("email"),
            "each record must have 'email' field"
        );
    }
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn secrets_never_reach_llm() {
    let text_with_cred = include_str!("fixtures/text_with_credential.txt");
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let fields = vec!["email".to_string(), "phone".to_string()];
    // run() should succeed (redaction replaces the credential)
    let _ = run(text_with_cred, &fields, &config, &client);
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let fields = sample_fields();
    let err = run("", &fields, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn empty_fields_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run(sample_contacts(), &[], &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn run_no_redact_produces_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let fields = sample_fields();
    let out = run_no_redact(sample_contacts(), &fields, &config, &client).unwrap();
    assert!(!out.records.is_empty());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let fields = sample_fields();
    let out = run(sample_contacts(), &fields, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain(&fields));
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let fields = sample_fields();
    let out = run(sample_contacts(), &fields, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_empty_records() {
    let out = Output {
        records: vec![],
        truncated: false,
    };
    assert_eq!(out.to_plain(&["name".to_string(), "email".to_string()]), "");
}

#[test]
fn to_plain_aligns_columns() {
    use std::collections::BTreeMap;
    let mut r1 = BTreeMap::new();
    r1.insert("name".to_string(), "Alice".to_string());
    r1.insert("email".to_string(), "alice@example.com".to_string());
    let mut r2 = BTreeMap::new();
    r2.insert("name".to_string(), "Bob".to_string());
    r2.insert("email".to_string(), "bob@example.com".to_string());
    let out = Output {
        records: vec![r1, r2],
        truncated: false,
    };
    let fields = vec!["name".to_string(), "email".to_string()];
    let plain = out.to_plain(&fields);
    let lines: Vec<&str> = plain.lines().collect();
    assert_eq!(lines.len(), 3); // header + 2 data rows
    assert!(
        lines[0].starts_with("name"),
        "header must start with 'name'"
    );
}
