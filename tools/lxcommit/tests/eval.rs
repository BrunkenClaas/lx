#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_commit_message_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxcommit::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let diff = include_str!("fixtures/sample_add_function.diff");
    let (out, _warnings) = run(diff, &config, client.as_ref()).unwrap();

    assert!(!out.commit_type.is_empty());
    assert!(
        out.subject.len() <= 72,
        "subject too long: {}",
        out.subject.len()
    );
    assert!(!out.subject.contains('\n'), "subject must be single line");

    let subject_lower = out.subject.to_lowercase();
    assert!(
        subject_lower.contains("add")
            || subject_lower.contains("implement")
            || subject_lower.contains("refresh")
            || subject_lower.contains("token"),
        "subject should reflect what was added: {}",
        out.subject
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_secret_in_diff() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxcommit::run::run;

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
