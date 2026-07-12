#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxnotes::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let notes = include_str!("fixtures/meeting_notes.txt");
    let out = run(notes, &config, client.as_ref()).unwrap();

    assert!(!out.sections.is_empty(), "must return at least one section");
    for section in &out.sections {
        assert!(
            !section.title.is_empty(),
            "every section must have a non-empty title"
        );
        assert!(
            !section.content.is_empty(),
            "every section must have at least one content item"
        );
        for item in &section.content {
            assert!(!item.is_empty(), "content items must not be empty strings");
        }
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_bearer_in_notes() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxnotes::run::run;

    let config = Config::load().unwrap();
    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);

    let notes_with_secret = r#"meeting notes
BEARER = "sk-abcdefghijklmnopqrstuvwxyz123456"
discussed roadmap for next quarter"#;

    let level = lx_redact::RedactLevel::Standard;
    let redacted = lx_redact::redact(notes_with_secret, level).unwrap();
    let _ = run(&redacted, &config, &client);

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
