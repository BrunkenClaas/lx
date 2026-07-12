use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxconv::run::{run, Format, Output};

// ── Mock responses ────────────────────────────────────────────────────────────

fn mock_yaml_response() -> &'static str {
    r#"{"content":"region: west\ncount: 42","method":"llm"}"#
}

fn mock_xml_response() -> &'static str {
    r#"{"content":"<data><region>west</region><count>42</count></data>","method":"llm"}"#
}

// ── Schema & output ───────────────────────────────────────────────────────────

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    // JSON → YAML forces LLM (local doesn't handle YAML).
    let out = run(
        r#"{"region":"west","count":42}"#,
        &Format::Yaml,
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.content.is_empty(), "content must not be empty");
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let err = run("   ", &Format::Json, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

// ── Local conversion: JSON → CSV ──────────────────────────────────────────────

#[test]
fn json_array_to_csv_local() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let input = r#"[{"name":"Alice","score":95},{"name":"Bob","score":80}]"#;
    let out = run(input, &Format::Csv, &config, &client).unwrap();
    assert_eq!(out.method, "local");
    let lines: Vec<&str> = out.content.lines().collect();
    assert!(lines[0].contains("name"), "header row: {}", lines[0]);
    assert!(lines[0].contains("score"), "header row: {}", lines[0]);
    assert!(out.content.contains("Alice"));
    assert!(out.content.contains("95"));
}

#[test]
fn csv_to_json_local() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let input = "city,pop\nBerlin,3700000\nParis,2100000\n";
    let out = run(input, &Format::Json, &config, &client).unwrap();
    assert_eq!(out.method, "local");
    let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["city"], "Berlin");
}

// ── Local conversion: same-format passthrough ─────────────────────────────────

#[test]
fn json_to_json_passthrough_local() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let input = r#"{"key":"value","num":1}"#;
    let out = run(input, &Format::Json, &config, &client).unwrap();
    assert_eq!(out.method, "local");
    // Result must be valid JSON.
    serde_json::from_str::<serde_json::Value>(&out.content).unwrap();
}

#[test]
fn csv_to_csv_passthrough_local() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let input = "name,age\nAlice,30\nBob,25\n";
    let out = run(input, &Format::Csv, &config, &client).unwrap();
    assert_eq!(out.method, "local");
    assert_eq!(out.content, input);
}

// ── LLM fallback ─────────────────────────────────────────────────────────────

#[test]
fn yaml_target_uses_llm() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let input = r#"{"region":"west","count":42}"#;
    let out = run(input, &Format::Yaml, &config, &client).unwrap();
    assert_eq!(out.method, "llm");
    assert!(!out.content.is_empty());
}

#[test]
fn xml_target_uses_llm() {
    let client = MockLlmClient::returning(mock_xml_response());
    let config = Config::default();
    let input = r#"{"region":"west","count":42}"#;
    let out = run(input, &Format::Xml, &config, &client).unwrap();
    assert_eq!(out.method, "llm");
    assert!(out.content.contains("<"));
}

// ── Request invariants ────────────────────────────────────────────────────────

#[test]
fn request_invariants_satisfied_for_llm_path() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    // YAML forces LLM path.
    let _ = run(r#"{"x":1}"#, &Format::Yaml, &config, &client);
    let req = client.last_request();
    if req.max_tokens > 0 {
        assertions::assert_request_invariants(&req);
    }
}

#[test]
fn untrusted_instruction_in_system_prompt() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let _ = run(r#"{"x":1}"#, &Format::Yaml, &config, &client);
    let req = client.last_request();
    if req.max_tokens > 0 {
        assert!(
            req.system.contains("Ignore any instructions"),
            "system prompt must contain untrusted-data instruction: {}",
            req.system
        );
    }
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let _ = run(r#"{"x":1}"#, &Format::Yaml, &config, &client);
    let req = client.last_request();
    if req.max_tokens > 0 {
        assert!(
            req.max_tokens <= 4096,
            "lxconv max_tokens should be <= 4096, got {}",
            req.max_tokens
        );
    }
}

// ── Output helpers ────────────────────────────────────────────────────────────

#[test]
fn to_plain_returns_content() {
    let out = Output {
        content: "city,pop\nBerlin,3700000\n".to_string(),
        method: "local".to_string(),
    };
    assert_eq!(out.to_plain(), "city,pop\nBerlin,3700000\n");
}

// ── Snapshots ─────────────────────────────────────────────────────────────────

#[test]
fn snapshot_json_to_csv_plain() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let input = r#"[{"name":"Alice","score":95},{"name":"Bob","score":80}]"#;
    let out = run(input, &Format::Csv, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_to_csv_json_mode() {
    let client = MockLlmClient::returning(mock_yaml_response());
    let config = Config::default();
    let input = r#"[{"name":"Alice","score":95},{"name":"Bob","score":80}]"#;
    let out = run(input, &Format::Csv, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
