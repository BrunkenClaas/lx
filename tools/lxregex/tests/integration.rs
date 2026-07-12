use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxregex::run::{run, Flavor, Output};

fn mock_email() -> &'static str {
    r#"{"pattern":"^[a-zA-Z0-9._%+\\-]+@[a-zA-Z0-9.\\-]+\\.[a-zA-Z]{2,}$","explanation":"Matches a full email address.","dangerous":false}"#
}

fn mock_redos() -> &'static str {
    r#"{"pattern":"(a+)+b","explanation":"Matches one or more a's followed by b, but has nested quantifiers.","dangerous":false}"#
}

fn mock_empty_pattern() -> &'static str {
    r#"{"pattern":"","explanation":"Nothing.","dangerous":false}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_email());
    let config = Config::default();
    let (out, _warnings) = run("email address", &Flavor::Pcre, None, &config, &client).unwrap();
    assert!(!out.pattern.is_empty(), "pattern must not be empty");
    assert!(!out.explanation.is_empty(), "explanation must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn flavor_appears_in_user_message() {
    let client = MockLlmClient::returning(mock_email());
    let config = Config::default();
    run("email address", &Flavor::Rust, None, &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("rust"),
        "user message should contain the flavor; got: {}",
        req.user
    );
}

#[test]
fn redos_pattern_is_detected_locally() {
    // Model says dangerous:false but the pattern has nested quantifiers — local
    // detection must override.
    let client = MockLlmClient::returning(mock_redos());
    let config = Config::default();
    let (out, _warnings) = run("match nested a's", &Flavor::Pcre, None, &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local ReDoS detection must override model's dangerous:false for pattern: {}",
        out.pattern
    );
}

#[test]
fn model_dangerous_flag_preserved_when_set() {
    let mock = r#"{"pattern":"(a|aa)+b","explanation":"Exponential backtracking example.","dangerous":true}"#;
    let client = MockLlmClient::returning(mock);
    let config = Config::default();
    let (out, _warnings) = run(
        "backtracking example",
        &Flavor::Pcre,
        None,
        &config,
        &client,
    )
    .unwrap();
    assert!(out.dangerous);
}

#[test]
fn safe_pattern_not_flagged() {
    let client = MockLlmClient::returning(mock_email());
    let config = Config::default();
    let (out, _warnings) = run("email address", &Flavor::Pcre, None, &config, &client).unwrap();
    assert!(
        !out.dangerous,
        "email pattern should not be flagged as dangerous"
    );
}

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_email());
    let config = Config::default();
    let err = run("", &Flavor::Pcre, None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn empty_pattern_from_model_returns_error() {
    let client = MockLlmClient::returning(mock_empty_pattern());
    let config = Config::default();
    let err = run("something", &Flavor::Pcre, None, &config, &client).unwrap_err();
    assert_eq!(
        err.exit_code(),
        lx_core::exit::LOGICAL_ERROR,
        "empty pattern from model must be a logical error"
    );
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_email());
    let config = Config::default();
    let (out, _warnings) = run("email address", &Flavor::Pcre, None, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_email());
    let config = Config::default();
    let (out, _warnings) = run("email address", &Flavor::Pcre, None, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_returns_pattern_only() {
    let out = Output {
        pattern: r"^\d+$".to_string(),
        explanation: "One or more digits.".to_string(),
        dangerous: false,
    };
    let plain = out.to_plain();
    assert_eq!(plain, r"^\d+$", "to_plain must return only the pattern");
    assert!(
        !plain.contains("One or more digits."),
        "explanation must not appear in to_plain output (goes to stderr)"
    );
}

#[test]
fn flavor_from_str_all_variants() {
    assert_eq!("pcre".parse::<Flavor>().unwrap(), Flavor::Pcre);
    assert_eq!("rust".parse::<Flavor>().unwrap(), Flavor::Rust);
    assert_eq!("python".parse::<Flavor>().unwrap(), Flavor::Python);
    assert_eq!("js".parse::<Flavor>().unwrap(), Flavor::Js);
    assert_eq!("javascript".parse::<Flavor>().unwrap(), Flavor::Js);
    assert_eq!("go".parse::<Flavor>().unwrap(), Flavor::Go);
    assert_eq!("golang".parse::<Flavor>().unwrap(), Flavor::Go);
    assert_eq!("ere".parse::<Flavor>().unwrap(), Flavor::Ere);
    assert_eq!("posix".parse::<Flavor>().unwrap(), Flavor::Ere);
    assert!("unknown".parse::<Flavor>().is_err());
}

#[test]
fn edit_mode_user_message_contains_existing_pattern() {
    let existing = r"^\d+$";
    let client = MockLlmClient::returning(mock_email());
    let config = Config::default();
    let _out = run(
        "also match optional leading minus sign",
        &Flavor::Pcre,
        Some(existing),
        &config,
        &client,
    )
    .unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit the following pcre regex"),
        "edit mode must include edit instruction, got: {}",
        req.user
    );
    assert!(
        req.user.contains(existing),
        "edit mode must include existing pattern in user message"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn create_mode_user_message_uses_input_prefix() {
    let client = MockLlmClient::returning(mock_email());
    let config = Config::default();
    let _out = run("email address", &Flavor::Pcre, None, &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.user.starts_with("Input (pcre):"),
        "create mode must use Input prefix, got: {}",
        req.user
    );
}

#[test]
fn is_redos_detects_nested_quantifiers() {
    use lxregex::run::is_potentially_redos;
    assert!(is_potentially_redos("(a+)+b"));
    assert!(is_potentially_redos("(a*)+b"));
    assert!(is_potentially_redos("(a+)*b"));
    assert!(is_potentially_redos("(.+)+"));
    assert!(is_potentially_redos("(.*)+"));
    // Safe patterns must not trigger.
    assert!(!is_potentially_redos(r"^\d+$"));
    assert!(!is_potentially_redos(r"^[a-z]+@[a-z]+\.[a-z]{2,}$"));
}
