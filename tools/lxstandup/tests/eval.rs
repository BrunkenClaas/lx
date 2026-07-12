#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxstandup::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let input = include_str!("fixtures/git_log.txt");
    let out = run(input, &config, client.as_ref()).unwrap();

    // done should have at least one item given the fixture has commits
    assert!(
        !out.done.is_empty(),
        "done list should not be empty for commit log input"
    );

    // Each item should be a non-empty string
    for item in &out.done {
        assert!(!item.trim().is_empty(), "done item must not be blank");
    }
    for item in &out.next {
        assert!(!item.trim().is_empty(), "next item must not be blank");
    }
    for item in &out.blockers {
        assert!(!item.trim().is_empty(), "blocker item must not be blank");
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_sensitive_input() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxstandup::run::run;

    let config = Config::load().unwrap();
    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);
    let input = include_str!("fixtures/git_log_with_sensitive.txt");
    let _ = run(input, &config, &client);
    let sent = client.last_user_message();
    assert!(
        !sent.contains("sk-abcdefghijklmnopqrstuvwxyz123456"),
        "raw bearer value must not reach LLM"
    );
    assert!(
        sent.contains("[REDACTED]"),
        "redacted placeholder must be present"
    );
}
