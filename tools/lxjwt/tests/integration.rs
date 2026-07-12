use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxjwt::run::{run, Output};

// A well-formed JWT:
// Header:  {"alg":"HS256","typ":"JWT"}
// Payload: {"sub":"user-42","iss":"example-service","iat":1700000000,"exp":1700003600,"role":"viewer"}
const SAMPLE_JWT: &str = include_str!("fixtures/sample.jwt");

fn mock_response() -> &'static str {
    r#"{"header":"Signed with HMAC-SHA256 algorithm, standard JWT type.","payload":"Issued by example-service for subject user-42 with a viewer role; valid for one hour.","notes":["Token has a 1-hour lifetime (iat to exp)","role claim grants viewer-level access","No audience (aud) claim is set"]}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(SAMPLE_JWT.trim(), &config, &client).unwrap();
    assert!(!out.header.is_empty(), "header must not be empty");
    assert!(!out.payload.is_empty(), "payload must not be empty");
    assert!(!out.notes.is_empty(), "notes must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(SAMPLE_JWT.trim(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(SAMPLE_JWT.trim(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn secrets_never_reach_llm() {
    // The JWT itself (raw token) must not appear in the LLM request — only
    // decoded header/payload should be sent.
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(SAMPLE_JWT.trim(), &config, &client);
    let req = client.last_request();
    // The raw JWT string contains dots and base64url; the user message must
    // contain decoded JSON, not the raw token.
    assert!(
        !req.user.contains("eyJhbGci"),
        "raw JWT header must not appear in LLM user message"
    );
    assertions::assert_no_secrets_in_request(&req);
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn invalid_jwt_returns_error() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("not.a.valid.jwt.at.all.extra", &config, &client).unwrap_err();
    // splitn(3, '.') on "not.a.valid.jwt.at.all.extra" gives 3 parts — last
    // part has dots. The header/payload decode will fail or succeed depending
    // on base64 validity. Test that it doesn't panic.
    let _ = err; // result is an error — that's what we check
}

#[test]
fn non_jwt_string_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("thisisnot-a-jwt", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn to_plain_renders_all_fields() {
    let out = Output {
        header: "HS256 header".to_string(),
        payload: "claims for user".to_string(),
        notes: vec!["note one".to_string(), "note two".to_string()],
    };
    let plain = out.to_plain();
    assert!(plain.contains("HS256 header"));
    assert!(plain.contains("claims for user"));
    assert!(plain.contains("note one"));
    assert!(plain.contains("note two"));
}
