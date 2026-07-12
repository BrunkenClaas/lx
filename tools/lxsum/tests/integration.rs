use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxsum::run::{run, run_with_opts, Output, SumFormat, SumOptions};

fn mock_response() -> &'static str {
    r#"{"tldr":"CI deployment succeeded for v3.7.2 after a brief health-check delay","bullets":["Unit tests passed (847/847)","Docker image built and pushed to registry","Staging health check was slow but recovered","Smoke tests passed (12/12)","Production deployment complete"]}"#
}

fn mock_prose_response() -> &'static str {
    r#"{"tldr":"CI deployment succeeded for v3.7.2 after a brief health-check delay","bullets":[],"body":"All unit tests passed and the Docker image was pushed to the registry. The staging health check experienced a brief slowdown but recovered. Smoke tests passed and the production deployment completed successfully."}"#
}

fn mock_short_response() -> &'static str {
    r#"{"tldr":"CI deployment succeeded for v3.7.2 after a brief health-check delay","bullets":[]}"#
}

fn mock_outline_response() -> &'static str {
    r#"{"tldr":"CI deployment succeeded for v3.7.2 after a brief health-check delay","bullets":["I. Build","  - Unit tests passed (847/847)","  - Docker image pushed","II. Staging","  - Health check recovered","III. Production","  - Smoke tests passed","  - Deployment complete"]}"#
}

fn deployment_log() -> &'static str {
    include_str!("fixtures/deployment_log.txt")
}

fn text_with_secret() -> &'static str {
    include_str!("fixtures/text_with_secret.txt")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(deployment_log(), &config, &client).unwrap();
    assert!(!out.tldr.is_empty(), "tldr must not be empty");
    assert!(!out.bullets.is_empty(), "bullets must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   \n\t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn secrets_never_reach_llm() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(text_with_secret(), &config, &client);
    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(deployment_log(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let (out, _warnings) = run(deployment_log(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_contains_tldr_and_bullets() {
    let out = Output {
        tldr: "Short summary of things".to_string(),
        bullets: vec!["Point one".to_string(), "Point two".to_string()],
        body: None,
    };
    let plain = out.to_plain();
    assert!(plain.contains("Short summary of things"));
    assert!(plain.contains("Point one"));
    assert!(plain.contains("Point two"));
    assert!(plain.contains('•'));
}

#[test]
fn to_plain_no_bullets_when_empty() {
    let out = Output {
        tldr: "A summary".to_string(),
        bullets: vec![],
        body: None,
    };
    let plain = out.to_plain();
    assert!(plain.contains("A summary"));
    assert!(!plain.contains('•'));
}

// ── Hub flag tests ────────────────────────────────────────────────────────────

#[test]
fn short_flag_produces_tldr_only() {
    let client = MockLlmClient::returning(mock_short_response());
    let config = Config::default();
    let opts = SumOptions {
        short: true,
        ..SumOptions::default()
    };
    let (out, _warnings) = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    assert!(!out.tldr.is_empty(), "tldr must not be empty");
    assert!(out.bullets.is_empty(), "short mode must produce no bullets");
    assert!(out.body.is_none(), "short mode must produce no body");
}

#[test]
fn headline_flag_accepts_empty_bullets() {
    // Regression: --headline instructs the model to emit only the tldr with
    // bullets:[] (like --short). The empty-bullets guard must not reject it.
    let client = MockLlmClient::returning(mock_short_response());
    let config = Config::default();
    let opts = SumOptions {
        headline: true,
        ..SumOptions::default()
    };
    let (out, _warnings) = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    assert!(!out.tldr.is_empty(), "headline tldr must not be empty");
    assert!(out.bullets.is_empty(), "headline mode produces no bullets");
    // Headline output must be the bare line — no "Summary:" prefix.
    let headline = out.to_headline();
    assert_eq!(headline, out.tldr);
    assert!(
        !headline.starts_with("Summary:"),
        "headline must not carry the Summary: prefix"
    );
}

#[test]
fn snapshot_short_mode() {
    let client = MockLlmClient::returning(mock_short_response());
    let config = Config::default();
    let opts = SumOptions {
        short: true,
        ..SumOptions::default()
    };
    let (out, _warnings) = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn prose_format_produces_body() {
    let client = MockLlmClient::returning(mock_prose_response());
    let config = Config::default();
    let opts = SumOptions {
        format: SumFormat::Prose,
        ..SumOptions::default()
    };
    let (out, _warnings) = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    assert!(!out.tldr.is_empty());
    assert!(out.body.is_some(), "prose format must populate body");
    assert!(
        out.bullets.is_empty(),
        "prose format must have empty bullets"
    );
}

#[test]
fn snapshot_prose_format() {
    let client = MockLlmClient::returning(mock_prose_response());
    let config = Config::default();
    let opts = SumOptions {
        format: SumFormat::Prose,
        ..SumOptions::default()
    };
    let (out, _warnings) = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn outline_format_produces_bullets() {
    let client = MockLlmClient::returning(mock_outline_response());
    let config = Config::default();
    let opts = SumOptions {
        format: SumFormat::Outline,
        ..SumOptions::default()
    };
    let (out, _warnings) = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    assert!(!out.tldr.is_empty());
    assert!(
        !out.bullets.is_empty(),
        "outline format must produce bullets"
    );
}

#[test]
fn snapshot_outline_format() {
    let client = MockLlmClient::returning(mock_outline_response());
    let config = Config::default();
    let opts = SumOptions {
        format: SumFormat::Outline,
        ..SumOptions::default()
    };
    let (out, _warnings) = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn max_words_constraint_included_in_system_prompt() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let opts = SumOptions {
        max_words: Some(50),
        ..SumOptions::default()
    };
    let _ = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("50"),
        "system prompt must mention the max_words value"
    );
}

#[test]
fn max_lines_constraint_included_in_system_prompt() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let opts = SumOptions {
        max_lines: Some(3),
        ..SumOptions::default()
    };
    let _ = run_with_opts(deployment_log(), &config, &client, &opts).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("3"),
        "system prompt must mention the max_lines value"
    );
}

#[test]
fn sum_format_parse_valid_values() {
    assert!(matches!(
        SumFormat::parse("bullets"),
        Some(SumFormat::Bullets)
    ));
    assert!(matches!(SumFormat::parse("prose"), Some(SumFormat::Prose)));
    assert!(matches!(
        SumFormat::parse("outline"),
        Some(SumFormat::Outline)
    ));
    assert!(matches!(
        SumFormat::parse("BULLETS"),
        Some(SumFormat::Bullets)
    ));
    assert!(SumFormat::parse("invalid").is_none());
}
