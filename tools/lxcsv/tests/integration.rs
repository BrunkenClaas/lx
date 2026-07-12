use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxcsv::run::{run, Output};

fn mock_response() -> &'static str {
    r#"{"answer":"Widget E has the highest revenue at 9300.","used_rows":"10 of 10 rows sampled"}"#
}

fn sample_csv() -> &'static str {
    include_str!("fixtures/sales.csv")
}

fn employees_csv() -> &'static str {
    include_str!("fixtures/employees.csv")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        sample_csv(),
        "Which product has the highest revenue?",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.answer.is_empty(), "answer must not be empty");
    assert!(!out.used_rows.is_empty(), "used_rows must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn secrets_never_reach_llm() {
    // CSV fixture with a sensitive-looking value that the redactor should mask.
    let csv_with_sensitive = "name,value\nuser,BEARER=sk-abcdefghijklmnopqrstuvwxyz123456\n";
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(csv_with_sensitive, "What is the value?", &config, &client);
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn empty_csv_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("", "How many rows?", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn empty_question_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run(sample_csv(), "", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_csv_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("  \n\t\n  ", "How many rows?", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        sample_csv(),
        "Which product has the highest revenue?",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        sample_csv(),
        "Which product has the highest revenue?",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_returns_answer_only() {
    let out = Output {
        answer: "The total revenue is 38550.".to_string(),
        used_rows: "10 of 10 rows sampled".to_string(),
    };
    let plain = out.to_plain();
    assert_eq!(plain, "The total revenue is 38550.");
    // No '#' comment lines on stdout.
    for line in plain.lines() {
        assert!(!line.starts_with('#'), "comment on stdout: {line:?}");
    }
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(sample_csv(), "How many rows?", &config, &client);
    if client.call_count() > 0 {
        let req = client.last_request();
        assert!(req.max_tokens <= 512, "lxcsv max_tokens must be ≤ 512");
    }
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(sample_csv(), "How many rows?", &config, &client);
    if client.call_count() > 0 {
        let req = client.last_request();
        assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
    }
}

#[test]
fn employees_csv_parses_correctly() {
    let client = MockLlmClient::returning(
        r#"{"answer":"The average salary is 85875.","used_rows":"8 of 8 rows sampled"}"#,
    );
    let config = Config::default();
    let out = run(
        employees_csv(),
        "What is the average salary?",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.answer.is_empty());
}
