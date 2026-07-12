use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxlog::run::{aggregate_logs, run, run_no_redact, Anomaly, Output};

// ── helpers ───────────────────────────────────────────────────────────────────

fn mock_response_anomalies() -> &'static str {
    r#"{"anomalies":[{"line":14,"level":"ERROR","message":"Cache server connection refused, falling back to uncached mode"},{"line":25,"level":"FATAL","message":"Worker thread terminated due to unhandled NullPointerException"}],"summary":"Two critical issues detected: cache server down causing performance degradation, and a fatal worker thread crash."}"#
}

fn mock_response_clean() -> &'static str {
    r#"{"anomalies":[],"summary":"Log looks healthy. All entries are informational with no errors or warnings."}"#
}

fn sample_log() -> &'static str {
    include_str!("fixtures/sample_app.log")
}

fn clean_log() -> &'static str {
    include_str!("fixtures/clean.log")
}

fn log_with_sensitive_data() -> &'static str {
    include_str!("fixtures/log_with_sensitive_data.log")
}

// ── schema / invariant tests ──────────────────────────────────────────────────

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response_anomalies());
    let config = Config::default();
    let out = run(sample_log(), &config, &client).unwrap();
    assert!(!out.anomalies.is_empty());
    let a = &out.anomalies[0];
    assert!(!a.message.is_empty());
    assert!(!a.level.is_empty());
    assert!(!out.summary.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn clean_log_returns_empty_anomalies() {
    let client = MockLlmClient::returning(mock_response_clean());
    let config = Config::default();
    let out = run(clean_log(), &config, &client).unwrap();
    assert!(out.anomalies.is_empty());
    assert!(!out.summary.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response_clean());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response_clean());
    let config = Config::default();
    let err = run("   \n\t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

// ── redaction tests ───────────────────────────────────────────────────────────

#[test]
fn secrets_never_reach_llm() {
    let client = MockLlmClient::returning(mock_response_clean());
    let config = Config::default();
    // run() must redact the sk- style key before sending to LLM.
    let _ = run(log_with_sensitive_data(), &config, &client);
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn run_no_redact_still_works() {
    let client = MockLlmClient::returning(mock_response_anomalies());
    let config = Config::default();
    let out = run_no_redact(sample_log(), &config, &client).unwrap();
    assert!(!out.anomalies.is_empty());
}

// ── local aggregation tests ───────────────────────────────────────────────────

#[test]
fn aggregate_empty_log_returns_empty() {
    let (result, used_lines, capped) = aggregate_logs("");
    assert!(result.is_empty());
    assert!(used_lines.is_empty());
    assert!(!capped);
}

#[test]
fn aggregate_includes_error_lines() {
    let log =
        "2024-01-01 INFO  Starting\n2024-01-01 ERROR Something broke\n2024-01-01 INFO  Done\n";
    let (result, _, _) = aggregate_logs(log);
    assert!(
        result.contains("ERROR Something broke"),
        "aggregated result missing ERROR line: {result}"
    );
}

#[test]
fn aggregate_includes_fatal_lines() {
    let log = "2024-01-01 INFO  ok\n2024-01-01 FATAL system crash\n2024-01-01 INFO  done\n";
    let (result, _, _) = aggregate_logs(log);
    assert!(
        result.contains("FATAL system crash"),
        "aggregated result missing FATAL line: {result}"
    );
}

#[test]
fn aggregate_deduplicates_repeated_lines() {
    let repeated = "2024-01-01 ERROR connection refused\n".repeat(5);
    let log = format!("2024-01-01 INFO  start\n{repeated}2024-01-01 INFO  end\n");
    let (result, _, _) = aggregate_logs(&log);
    // Should contain a count indicator like (x5)
    assert!(
        result.contains("x5") || result.contains("(x"),
        "expected deduplication count in: {result}"
    );
}

#[test]
fn aggregate_large_log_does_not_exceed_limit() {
    // Generate a log larger than MAX_SAMPLE_LINES with all ERROR lines
    let mut lines = Vec::new();
    for i in 0..600 {
        lines.push(format!("2024-01-01 ERROR failure number {i}"));
    }
    let log = lines.join("\n");
    let (result, used_lines, capped) = aggregate_logs(&log);
    // Should be bounded
    let output_lines: Vec<&str> = result.lines().collect();
    assert!(
        output_lines.len() <= 510, // MAX_SAMPLE_LINES + some header/context lines
        "aggregated output too large: {} lines",
        output_lines.len()
    );
    assert!(
        used_lines.contains("500"),
        "used_lines should mention the cap: {used_lines}"
    );
    assert!(capped, "capped should be true for oversized log");
}

// ── output formatting tests ───────────────────────────────────────────────────

#[test]
fn to_plain_with_anomalies() {
    let out = Output {
        anomalies: vec![Anomaly {
            line: Some(5),
            level: "ERROR".to_string(),
            message: "Connection refused".to_string(),
        }],
        summary: "One error found.".to_string(),
        used_lines: String::new(),
        capped: false,
    };
    let plain = out.to_plain();
    assert!(
        plain.contains("[ERROR]"),
        "expected [ERROR] in output: {plain}"
    );
    assert!(
        plain.contains("line 5"),
        "expected line number in output: {plain}"
    );
    assert!(
        plain.contains("Connection refused"),
        "expected message in output: {plain}"
    );
    assert!(
        plain.contains("One error found."),
        "expected summary in output: {plain}"
    );
}

#[test]
fn to_plain_anomaly_without_line() {
    let out = Output {
        anomalies: vec![Anomaly {
            line: None,
            level: "WARN".to_string(),
            message: "Multiple retries observed".to_string(),
        }],
        summary: "Warning present.".to_string(),
        used_lines: String::new(),
        capped: false,
    };
    let plain = out.to_plain();
    assert!(plain.contains("[WARN]"), "expected [WARN]: {plain}");
    assert!(!plain.contains("line"), "no line number expected: {plain}");
}

#[test]
fn to_plain_no_anomalies_shows_summary() {
    let out = Output {
        anomalies: vec![],
        summary: "All good.".to_string(),
        used_lines: String::new(),
        capped: false,
    };
    let plain = out.to_plain();
    assert_eq!(plain, "All good.");
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response_anomalies());
    let config = Config::default();
    let out = run(sample_log(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response_anomalies());
    let config = Config::default();
    let out = run(sample_log(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
