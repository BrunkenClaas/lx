#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdiff::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let diff = include_str!("fixtures/sample.diff");
    let (out, _warnings) = run(diff, &config, client.as_ref()).unwrap();

    assert!(!out.summary.is_empty(), "summary must not be empty");
    assert!(!out.changes.is_empty(), "changes list must not be empty");
    // Each change should be a non-empty sentence.
    for change in &out.changes {
        assert!(!change.is_empty(), "change entry must not be empty");
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_secret_in_diff() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxdiff::run::run;

    let config = Config::load().unwrap();
    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);
    let diff = include_str!("fixtures/sample_with_secret.diff");
    let _ = run(diff, &config, &client);
    // The raw secret must not appear in the user message sent to the LLM.
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
