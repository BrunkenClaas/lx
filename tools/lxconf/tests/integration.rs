use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxconf::run::{run, ConfigMode, Finding, Output};

fn mock_response_with_findings() -> &'static str {
    r#"{"findings":[{"line":3,"severity":"error","message":"port value 99999 is outside the valid range 1-65535","hint":"Use a port number between 1 and 65535"},{"line":4,"severity":"warning","message":"duplicate key port overrides the earlier definition on line 3","hint":"Remove the duplicate key"}]}"#
}

fn mock_response_no_findings() -> &'static str {
    r#"{"findings":[]}"#
}

fn mock_response_content() -> &'static str {
    r##"{"content":"# generated config\nhost = localhost\nport = 5432\n"}"##
}

fn sample_config() -> &'static str {
    include_str!("fixtures/broken_toml.toml")
}

fn valid_config() -> &'static str {
    include_str!("fixtures/valid_config.toml")
}

fn config_with_secret() -> &'static str {
    include_str!("fixtures/config_with_secret.toml")
}

#[test]
fn audit_mode_output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response_with_findings());
    let config = Config::default();
    let (out, _warnings) = run(sample_config(), None, ConfigMode::Audit, &config, &client).unwrap();
    assert!(!out.findings.is_empty());
    for f in &out.findings {
        assert!(
            ["error", "warning", "info"].contains(&f.severity.as_str()),
            "unexpected severity: {}",
            f.severity
        );
        assert!(!f.message.is_empty(), "finding message must not be empty");
    }
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn audit_mode_empty_findings_is_valid() {
    let client = MockLlmClient::returning(mock_response_no_findings());
    let config = Config::default();
    let (out, _warnings) = run(valid_config(), None, ConfigMode::Audit, &config, &client).unwrap();
    assert!(out.findings.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn audit_mode_empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response_no_findings());
    let config = Config::default();
    let err = run("", None, ConfigMode::Audit, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn audit_mode_secrets_never_reach_llm() {
    let client = MockLlmClient::returning(mock_response_no_findings());
    let config = Config::default();
    let _ = run(
        config_with_secret(),
        None,
        ConfigMode::Audit,
        &config,
        &client,
    );
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn create_mode_returns_content() {
    let client = MockLlmClient::returning(mock_response_content());
    let config = Config::default();
    let (out, _warnings) = run(
        "postgres config for 8 cores",
        None,
        ConfigMode::Create,
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.content.is_empty(), "create mode must return content");
    let req = client.last_request();
    assert!(
        req.user.contains("Generate a config file for:"),
        "create mode must use generate instruction, got: {}",
        req.user
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn edit_mode_user_message_contains_existing_and_intent() {
    let existing = "host = localhost\nport = 5432\n";
    let client = MockLlmClient::returning(mock_response_content());
    let config = Config::default();
    let _out = run(
        "change port to 5433",
        Some(existing),
        ConfigMode::Edit,
        &config,
        &client,
    )
    .unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit the following config file"),
        "edit mode must include edit instruction, got: {}",
        req.user
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn snapshot_plain_output_with_findings() {
    let client = MockLlmClient::returning(mock_response_with_findings());
    let config = Config::default();
    let (out, _warnings) = run(sample_config(), None, ConfigMode::Audit, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain(ConfigMode::Audit));
}

#[test]
fn snapshot_plain_output_no_findings() {
    let client = MockLlmClient::returning(mock_response_no_findings());
    let config = Config::default();
    let (out, _warnings) = run(valid_config(), None, ConfigMode::Audit, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain(ConfigMode::Audit));
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response_with_findings());
    let config = Config::default();
    let (out, _warnings) = run(sample_config(), None, ConfigMode::Audit, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_formats_finding_with_hint() {
    let out = Output {
        findings: vec![Finding {
            line: Some(3),
            severity: "error".to_string(),
            message: "port value 99999 is outside the valid range 1-65535".to_string(),
            hint: Some("Use a port number between 1 and 65535".to_string()),
        }],
        content: String::new(),
    };
    let plain = out.to_plain(ConfigMode::Audit);
    assert!(plain.contains("[error]"));
    assert!(plain.contains("line 3"));
    assert!(plain.contains("hint:"));
}

#[test]
fn to_plain_formats_finding_without_hint() {
    let out = Output {
        findings: vec![Finding {
            line: None,
            severity: "info".to_string(),
            message: "consider using environment variables for sensitive values".to_string(),
            hint: None,
        }],
        content: String::new(),
    };
    let plain = out.to_plain(ConfigMode::Audit);
    assert!(plain.contains("[info]"));
    assert!(plain.contains("file"));
    assert!(!plain.contains("hint:"));
}

#[test]
fn to_plain_empty_findings() {
    let out = Output {
        findings: vec![],
        content: String::new(),
    };
    assert_eq!(out.to_plain(ConfigMode::Audit), "no issues found");
}

#[test]
fn to_plain_create_returns_content() {
    let out = Output {
        findings: vec![],
        content: "host = localhost\nport = 5432\n".to_string(),
    };
    assert_eq!(
        out.to_plain(ConfigMode::Create),
        "host = localhost\nport = 5432\n"
    );
}
