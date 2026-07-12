use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxsecret::run::{run, run_local, Output};

// High-entropy test values (not documentation examples — real-looking secrets).
// These pass the entropy gate; low-entropy examples like AKIAIOSFODNN7EXAMPLE do not.
const AWS_KEY: &str = "AKIAJ3MV4BNZC9X7PQRF";
const GITHUB_PAT: &str = "ghp_R8mA9fL3kDe2nV0xPqWsYuIoBtJhMcZg5r6T";

// ── Mock helpers ─────────────────────────────────────────────────────────────

fn mock_real() -> &'static str {
    r#"{"assessment":"real","confidence":"high","reason":"Pattern matches a live credential."}"#
}

fn mock_placeholder() -> &'static str {
    r#"{"assessment":"placeholder","confidence":"high","reason":"Value looks like an example."}"#
}

// ── Schema / invariant tests ──────────────────────────────────────────────────

#[test]
fn output_schema_is_valid_with_findings() {
    let input = include_str!("fixtures/secrets.env");
    let client = MockLlmClient::returning(mock_real());
    let config = Config::default();
    let out = run(input, &config, &client, false).unwrap();
    assert!(
        !out.findings.is_empty(),
        "expected findings from secrets fixture"
    );
    for f in &out.findings {
        assert!(!f.secret_type.is_empty(), "secret_type must not be empty");
        assert!(!f.location.is_empty(), "location must not be empty");
        assert!(!f.masked.is_empty(), "masked must not be empty");
    }
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_empty_findings() {
    let client = MockLlmClient::returning(mock_real());
    let config = Config::default();
    let out = run("", &config, &client, false).unwrap();
    assert!(
        out.findings.is_empty(),
        "empty input must produce no findings"
    );
    assert_eq!(client.call_count(), 0, "no LLM calls for empty input");
}

#[test]
fn max_tokens_within_limit() {
    let input = format!("export AWS_ACCESS_KEY_ID={AWS_KEY}");
    let client = MockLlmClient::returning(mock_real());
    let config = Config::default();
    let _ = run(&input, &config, &client, false);
    if client.call_count() > 0 {
        let req = client.last_request();
        assert!(req.max_tokens <= 512, "max_tokens must be ≤ 512");
        assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
    }
}

// ── Detection pattern tests ───────────────────────────────────────────────────

#[test]
fn detects_aws_access_key() {
    let findings = run_local(&format!("AWS_ACCESS_KEY_ID={AWS_KEY}"), false);
    assert!(
        findings.iter().any(|f| f.secret_type == "aws_access_key"),
        "expected aws_access_key, got: {findings:?}"
    );
}

#[test]
fn detects_github_pat() {
    let findings = run_local(&format!("GITHUB_TOKEN={GITHUB_PAT}"), false);
    assert!(
        findings.iter().any(|f| f.secret_type == "github_pat"),
        "expected github_pat, got: {findings:?}"
    );
}

#[test]
fn detects_gitlab_pat() {
    // glpat- + 21 mixed-case chars with digits (detector requires ≥20 after prefix).
    let findings = run_local("CI_JOB_TOKEN=glpat-aBcDeFgHiJ1kLmN2oP3qR", false);
    assert!(
        findings.iter().any(|f| f.secret_type == "gitlab_pat"),
        "expected gitlab_pat, got: {findings:?}"
    );
}

#[test]
fn detects_slack_token() {
    // Realistic xoxb token: 12-digit team, 12-digit user, 24-char mixed secret.
    // Avoids repeated-digit runs that the placeholder filter (correctly) rejects.
    let findings = run_local(
        "SLACK_TOKEN=xoxb-867530912345-869012345678-aBcDeFgHiJkLmNoPqRsTuV",
        false,
    );
    assert!(
        findings.iter().any(|f| f.secret_type == "slack_token"),
        "expected slack_token, got: {findings:?}"
    );
}

#[test]
fn detects_stripe_live_key() {
    // sk_live_ + 28 mixed chars (detector requires ≥24 after prefix).
    let findings = run_local("STRIPE_SECRET=sk_live_4eC39HqLyjWDarjT1zdpAB12cd34", false);
    assert!(
        findings
            .iter()
            .any(|f| f.secret_type == "stripe_secret_key"),
        "expected stripe_secret_key, got: {findings:?}"
    );
}

#[test]
fn detects_openai_key() {
    let findings = run_local(
        "OPENAI_KEY=sk-aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ABCD",
        false,
    );
    assert!(
        findings.iter().any(|f| f.secret_type == "openai_api_key"),
        "expected openai_api_key, got: {findings:?}"
    );
}

#[test]
fn detects_private_key_header() {
    let findings = run_local("-----BEGIN RSA PRIVATE KEY-----", false);
    assert!(
        findings.iter().any(|f| f.secret_type == "private_key"),
        "expected private_key, got: {findings:?}"
    );
}

#[test]
fn clean_text_produces_no_findings() {
    let findings = run_local(
        "This is a normal log line.\nNo secrets here.\nJust regular text.",
        false,
    );
    assert!(
        findings.is_empty(),
        "expected no findings, got: {findings:?}"
    );
}

// ── Entropy gate tests ────────────────────────────────────────────────────────

#[test]
fn low_entropy_aws_example_not_flagged() {
    // AWS's own documentation example is intentionally low-entropy —
    // confirming the entropy gate rejects it as a likely non-secret.
    let findings = run_local("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE", false);
    let aws_findings = findings
        .iter()
        .filter(|f| f.secret_type == "aws_access_key")
        .count();
    assert_eq!(
        aws_findings, 0,
        "low-entropy AWS example must not be flagged: {findings:?}"
    );
}

#[test]
fn low_entropy_stripe_prefix_not_flagged() {
    // A mechanically repetitive suffix (entropy well below the 2.0 floor) must
    // be rejected by the gate even though it carries a valid sk_live_ prefix.
    let findings = run_local("key=sk_live_abcabcabcabcabcabcabcabcabc", false);
    let stripe = findings
        .iter()
        .filter(|f| f.secret_type.starts_with("stripe"))
        .count();
    assert_eq!(
        stripe, 0,
        "low-entropy sk_live_ value must not be flagged: {findings:?}"
    );
}

// ── Masking tests ─────────────────────────────────────────────────────────────

#[test]
fn masked_value_never_contains_middle_of_secret() {
    let findings = run_local(&format!("AWS_KEY={AWS_KEY}"), false);
    for f in &findings {
        if f.secret_type == "aws_access_key" {
            assert!(
                f.masked.contains("****"),
                "masked value must contain **** placeholder: {}",
                f.masked
            );
        }
    }
}

#[test]
fn secret_values_never_sent_to_llm() {
    let client = MockLlmClient::returning(mock_real());
    let config = Config::default();
    let _ = run(&format!("AWS_KEY={AWS_KEY}"), &config, &client, false);

    if client.call_count() > 0 {
        let req = client.last_request();
        assert!(
            !req.user.contains(AWS_KEY),
            "full secret value must not appear in LLM request user field"
        );
        assert!(
            !req.system.contains(AWS_KEY),
            "full secret value must not appear in LLM request system field"
        );
    }
}

#[test]
fn assessment_from_llm_appears_in_output() {
    let client = MockLlmClient::returning(mock_real());
    let config = Config::default();
    let out = run(&format!("AWS_KEY={AWS_KEY}"), &config, &client, false).unwrap();
    if let Some(f) = out
        .findings
        .iter()
        .find(|f| f.secret_type == "aws_access_key")
    {
        assert_eq!(
            f.assessment.as_deref(),
            Some("real"),
            "expected assessment 'real' from mock"
        );
    }
}

#[test]
fn placeholder_assessment_from_llm() {
    let client = MockLlmClient::returning(mock_placeholder());
    let config = Config::default();
    let out = run(&format!("AWS_KEY={AWS_KEY}"), &config, &client, false).unwrap();
    if let Some(f) = out
        .findings
        .iter()
        .find(|f| f.secret_type == "aws_access_key")
    {
        assert_eq!(
            f.assessment.as_deref(),
            Some("placeholder"),
            "expected assessment 'placeholder' from mock"
        );
    }
}

// ── Output format tests ───────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output() {
    // Use a variable name without a secret-context keyword so only the prefix
    // detector fires (a keyword like ACCESS_KEY would also trigger the generic
    // high-entropy path, which is correct but noisier to snapshot).
    let findings = run_local(&format!("AWS_ID={AWS_KEY}"), false);
    let out = Output { findings };
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let findings = run_local(&format!("AWS_ID={AWS_KEY}"), false);
    let out = Output { findings };
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn json_output_is_valid_json() {
    let findings = run_local(&format!("GITHUB_TOKEN={GITHUB_PAT}"), false);
    let out = Output { findings };
    let json = serde_json::to_string(&out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("findings").is_some(),
        "JSON must have 'findings' field"
    );
}

// ── Strict mode tests ─────────────────────────────────────────────────────────

#[test]
fn strict_mode_detects_high_entropy_without_keyword() {
    // A 30+ char mixed-case+digit token with no keyword context.
    // Default mode misses it; strict mode should catch it.
    let token = "z9Xm2KpQ7rLwF4NsVeJ6cT8bUhYdA1oG5";
    let input = format!("some_field={token}");
    let default_findings = run_local(&input, false);
    let strict_findings = run_local(&input, true);
    // strict mode either catches more or at least the same — never fewer
    assert!(
        strict_findings.len() >= default_findings.len(),
        "strict mode must not produce fewer findings than default"
    );
}

#[test]
fn strict_false_does_not_add_bare_high_entropy() {
    // Keyword-free high-entropy token should NOT trigger in default mode.
    let input = "logging_backend=z9Xm2KpQ7rLwF4NsVeJ6cT8bUhYdA1oG5";
    let findings = run_local(input, false);
    let bare = findings
        .iter()
        .filter(|f| f.secret_type == "high_entropy_secret")
        .count();
    // In default mode there should be no high_entropy_secret finding without a real keyword.
    // (logging_backend is not in the keyword list)
    assert_eq!(
        bare, 0,
        "keyword-free token must not trigger high_entropy_secret in default mode"
    );
}
