use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxcert::run::run;

const MOCK_RESPONSE: &str = r#"{"subject":"CN=localhost, O=Test Corp, C=US","issuer":"CN=localhost, O=Test Corp, C=US","valid_until":"2025-01-01","notes":["Self-signed certificate","Suitable for local development only","No Subject Alternative Names defined"]}"#;

const SAMPLE_PEM: &str = "-----BEGIN CERTIFICATE-----
MIICpDCCAYwCCQDz1p3s6K7LsTANBgkqhkiG9w0BAQsFADAUMRIwEAYDVQQDDAls
b2NhbGhvc3QwHhcNMjQwMTAxMDAwMDAwWhcNMjUwMTAxMDAwMDAwWjAUMRIwEAYD
VQQDDAlsb2NhbGhvc3QwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC7
o4qne60TB3wolDXPpLNQrNOXj41RGvveFg3w0m1AJLP9BSMl3JCPjMBq5CJ9Bhqn
Ef/S2OUhkC7bvxEbW1GBaHTVQlKWZFUPpF5t5GGKi7ENfFuOJ8QaFwg8E2MkYJ
LqkbYHJHqVFTzfm5e7lkD4HMWf5PxJMrW6iEalW8QbGXBD0GVGu8TI6i7H6Wo
AgMBAAEwDQYJKoZIhvcNAQELBQADggEBABXbWpFchLWfnfCQJbNwk3CuNhQzSRL
-----END CERTIFICATE-----";

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(MOCK_RESPONSE);
    let config = Config::default();
    let out = run(SAMPLE_PEM, &config, &client).unwrap();
    assert!(!out.subject.is_empty(), "subject must not be empty");
    assert!(!out.issuer.is_empty(), "issuer must not be empty");
    assert!(!out.valid_until.is_empty(), "valid_until must not be empty");
    assert!(!out.notes.is_empty(), "notes must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(MOCK_RESPONSE);
    let config = Config::default();
    let err = run("   ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn non_pem_input_returns_bad_usage() {
    let client = MockLlmClient::returning(MOCK_RESPONSE);
    let config = Config::default();
    let err = run("this is not a certificate", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(MOCK_RESPONSE);
    let config = Config::default();
    let _ = run(SAMPLE_PEM, &config, &client);
    let req = client.last_request();
    assert!(req.max_tokens <= 512, "lxcert max_tokens should be <= 512");
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(MOCK_RESPONSE);
    let config = Config::default();
    let _ = run(SAMPLE_PEM, &config, &client);
    let req = client.last_request();
    assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(MOCK_RESPONSE);
    let config = Config::default();
    let out = run(SAMPLE_PEM, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(MOCK_RESPONSE);
    let config = Config::default();
    let out = run(SAMPLE_PEM, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
