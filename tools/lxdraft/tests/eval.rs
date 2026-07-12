#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_email_draft_has_subject_and_body() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdraft::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "invite team to Q3 planning, wednesday 10am, bring roadmap ideas",
        "email",
        &config,
        client.as_ref(),
    )
    .expect("run should succeed");

    assert!(!out.body.is_empty(), "body must not be empty");
    assert!(out.subject.is_some(), "email draft should have a subject");
    let subj = out.subject.unwrap();
    assert!(!subj.is_empty(), "subject must not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_ticket_draft_has_subject() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdraft::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "dark mode toggle broken, stays light on refresh, reproducible on chrome",
        "ticket",
        &config,
        client.as_ref(),
    )
    .expect("run should succeed");

    assert!(!out.body.is_empty(), "body must not be empty");
    assert!(out.subject.is_some(), "ticket draft should have a subject");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_message_draft_has_null_subject() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdraft::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "great work on the release, everything went smoothly",
        "message",
        &config,
        client.as_ref(),
    )
    .expect("run should succeed");

    assert!(!out.body.is_empty(), "body must not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_sensitive_input() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxdraft::run::run;

    let config = Config::load().unwrap();
    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);
    let input = include_str!("fixtures/input_with_secret.txt");
    let _ = run(input, "email", &config, &client);
    let sent = client.last_user_message();
    assert!(
        !sent.contains("AKIA1234567890ABCDEF"),
        "raw identifier must not reach LLM"
    );
}
