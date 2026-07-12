#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_pr_description_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxpr::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let diff = include_str!("fixtures/sample_pr.diff");
    let (out, _warnings) = run(diff, &config, client.as_ref()).unwrap();

    assert!(!out.title.is_empty(), "title must not be empty");
    assert!(
        out.title.len() <= 72,
        "title too long: {} chars",
        out.title.len()
    );
    assert!(!out.title.contains('\n'), "title must be single line");
    assert!(!out.body.is_empty(), "body must not be empty");
    // Body should contain the standard sections
    let body_lower = out.body.to_lowercase();
    assert!(
        body_lower.contains("summary") || body_lower.contains("changes"),
        "body should contain Summary or Changes section: {}",
        out.body
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_secret_in_diff() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxpr::run::run;

    let config = Config::load().unwrap();
    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);
    let diff = include_str!("fixtures/diff_with_secret.diff");
    let _ = run(diff, &config, &client);
    // RecordingLlmClient exposes last_user_message() — the redacted user field.
    let sent = client.last_user_message();
    assert!(
        !sent.contains("sk-1234567890"),
        "raw secret must not reach LLM"
    );
    assert!(
        sent.contains("[REDACTED]"),
        "redacted placeholder must be present"
    );
}
