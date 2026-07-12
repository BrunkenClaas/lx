// Eval tests — require a real LLM API key.
// Run with: cargo test -p lxsecret -- --include-ignored eval_

use lx_config::Config;
use lx_llm::client_from_config;
use lxsecret::run::run;

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_detects_and_classifies_aws_key() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let config = Config::default();
    let client = client_from_config(&config, false).expect("LLM client must be available");
    let input = "AWS_ACCESS_KEY_ID=AKIAJ3MV4BNZC9X7PQRF";
    let out = run(input, &config, client.as_ref(), false).expect("run must succeed");

    assert!(
        !out.findings.is_empty(),
        "expected at least one finding from AWS key input"
    );
    let finding = out
        .findings
        .iter()
        .find(|f| f.secret_type == "aws_access_key");
    assert!(finding.is_some(), "expected aws_access_key finding");
    // Masked value must not contain the full secret.
    if let Some(f) = finding {
        assert!(
            !f.masked.contains("J3MV4BNZC9X"),
            "full secret must not appear in masked output"
        );
        // LLM should classify this.
        assert!(
            f.assessment.is_some(),
            "assessment must be set when LLM is available"
        );
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_no_findings_for_clean_input() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let config = Config::default();
    let client = client_from_config(&config, false).expect("LLM client must be available");
    let input = "This is a clean log message with no credentials.";
    let out = run(input, &config, client.as_ref(), false).expect("run must succeed");
    assert!(
        out.findings.is_empty(),
        "expected no findings for clean input, got: {:?}",
        out.findings
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_secret_value_never_sent_to_llm() {
    // This test verifies the invariant with a real client's request/response.
    // We use a recording client so we can inspect what was sent.
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let config = Config::default();
    let inner = client_from_config(&config, false).expect("LLM client must be available");
    let recording = lx_testkit::RecordingLlmClient::new(inner);
    let secret = "AKIAJ3MV4BNZC9X7PQRF";
    let input = format!("AWS_ACCESS_KEY_ID={secret}");
    let _ = run(&input, &config, &recording, false);
    // Inspect all captured calls.
    let calls = recording.calls.lock().unwrap();
    for (req, _resp) in calls.iter() {
        assert!(
            !req.user.contains(secret),
            "full secret must not appear in LLM request user: {}",
            &req.user[..req.user.len().min(200)]
        );
    }
}
