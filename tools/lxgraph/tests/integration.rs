use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxgraph::run::{parse_input, render_bar_chart, run, Output};

fn mock_response() -> &'static str {
    r#"{"chart_type":"bar","series":["Sales Q1","Sales Q2","Sales Q3","Sales Q4"]}"#
}

fn sample_csv() -> &'static str {
    include_str!("fixtures/sales.txt")
}

// ── Schema / invariants ──────────────────────────────────────────────────────

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_csv(), &config, &client).unwrap();
    assert!(!out.chart.is_empty(), "chart must not be empty");
    assert!(!out.series.is_empty(), "series must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

// ── Snapshots ─────────────────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_csv(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_csv(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   \n   ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

// ── Local parsing ─────────────────────────────────────────────────────────────

#[test]
fn parse_csv_format() {
    let points = parse_input("Alpha,100\nBeta,200\nGamma,150").unwrap();
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].label, "Alpha");
    assert_eq!(points[0].value, 100.0);
    assert_eq!(points[1].label, "Beta");
    assert_eq!(points[1].value, 200.0);
}

#[test]
fn parse_single_numbers_per_line() {
    let points = parse_input("10\n20\n30").unwrap();
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].value, 10.0);
    assert_eq!(points[2].value, 30.0);
}

#[test]
fn parse_space_separated_numbers() {
    let points = parse_input("5 10 15").unwrap();
    assert_eq!(points.len(), 3);
    assert_eq!(points[1].value, 10.0);
}

#[test]
fn parse_invalid_number_on_single_number_line_returns_error() {
    // A purely non-numeric, non-CSV line after data lines is an error.
    let err = parse_input("10\nnotanumber").unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn parse_multi_column_csv_uses_first_numeric_field() {
    // Acceptance fixture format: region,product,q1_sales,q2_sales,...
    // First numeric field (q1_sales) is used as the value; string prefix is the label.
    let input =
        "region,product,q1_sales,q2_sales\nNorth,Widget A,12500,15200\nSouth,Widget B,8900,9200";
    let points = parse_input(input).unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].label, "North Widget A");
    assert_eq!(points[0].value, 12500.0);
    assert_eq!(points[1].label, "South Widget B");
    assert_eq!(points[1].value, 8900.0);
}

#[test]
fn parse_multi_column_csv_header_only_row_is_skipped() {
    // An all-string first line must be treated as a header and skipped.
    let input = "region,product\nNorth,100\nSouth,200";
    let points = parse_input(input).unwrap();
    assert_eq!(points.len(), 2);
}

#[test]
fn parse_csv_with_header_row_is_skipped() {
    // A CSV header like "product,q1_sales,..." must be detected and skipped.
    let points = parse_input("product,q1_sales\nWidgets,500\nGadgets,300").unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0].label, "Widgets");
    assert_eq!(points[0].value, 500.0);
}

#[test]
fn parse_skips_blank_lines_and_comments() {
    let points = parse_input("# header\n\n10\n20\n").unwrap();
    assert_eq!(points.len(), 2);
}

// ── Chart rendering ───────────────────────────────────────────────────────────

#[test]
fn render_chart_has_expected_structure() {
    let points = parse_input("A,100\nB,200").unwrap();
    let chart = render_bar_chart(&points, &[]);
    // The chart should have 2 lines (one per data point).
    let lines: Vec<&str> = chart.lines().collect();
    assert_eq!(lines.len(), 2);
    // Each line should contain the separator pipe character.
    for line in &lines {
        assert!(line.contains('|'), "chart line must contain '|': {}", line);
    }
}

#[test]
fn render_uses_llm_labels_when_count_matches() {
    let points = parse_input("100\n200").unwrap();
    let llm_labels = vec!["First".to_string(), "Second".to_string()];
    let chart = render_bar_chart(&points, &llm_labels);
    assert!(
        chart.contains("First"),
        "chart should use LLM label 'First'"
    );
    assert!(
        chart.contains("Second"),
        "chart should use LLM label 'Second'"
    );
}

#[test]
fn render_falls_back_to_parsed_labels_when_count_mismatch() {
    let points = parse_input("Alpha,100\nBeta,200").unwrap();
    // LLM returns only one label — mismatch, should fall back.
    let llm_labels = vec!["OnlyOne".to_string()];
    let chart = render_bar_chart(&points, &llm_labels);
    assert!(
        chart.contains("Alpha"),
        "should fall back to parsed label 'Alpha'"
    );
}

#[test]
fn to_plain_returns_chart() {
    let out = Output {
        chart: "A | ████ 100".to_string(),
        series: vec!["A".to_string()],
    };
    assert_eq!(out.to_plain(), "A | ████ 100");
}
