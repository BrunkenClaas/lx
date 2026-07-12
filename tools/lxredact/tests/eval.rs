#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_explain_output_structure() {
    use lx_config::Config;
    use lx_redact::RedactLevel;
    use lxredact::run::run;

    if std::env::var("LX_API_KEY").is_err() {
        return;
    }

    let config = Config::load().expect("config must load");
    let client = lx_llm::client_from_config(&config, false).expect("client must build");

    let input = "api_key=sk-abcdefghijklmnopqrstuvwxyz12345\npassword=hunter2";
    let out = run(
        input,
        RedactLevel::Standard,
        true,
        false,
        &config,
        client.as_ref(),
    )
    .expect("run must succeed");

    // Structural checks only — no exact-text assertions.
    assert!(out.redacted_count > 0, "must detect secrets");
    assert!(!out
        .redacted_text
        .contains("sk-abcdefghijklmnopqrstuvwxyz12345"));
    assert!(!out.redacted_text.contains("hunter2"));

    let ex = out
        .explanation
        .expect("explanation must be present when --explain");
    assert!(
        !ex.summary.is_empty(),
        "explanation summary must not be empty"
    );
    assert!(
        ["low", "medium", "high"].contains(&ex.risk_level.as_str()),
        "risk_level must be low/medium/high, got: {}",
        ex.risk_level
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_no_secrets_skips_llm() {
    use lx_config::Config;
    use lx_redact::RedactLevel;
    use lx_testkit::MockLlmClient;
    use lxredact::run::run;

    let config = Config::load().expect("config must load");
    let client = MockLlmClient::returning("{}");

    let input = "Hello, this is a perfectly clean text with no secrets.";
    let out =
        run(input, RedactLevel::Standard, true, false, &config, &client).expect("run must succeed");

    assert_eq!(out.redacted_count, 0);
    assert_eq!(out.redacted_text, input);
    // No LLM call should be made when nothing was redacted.
    assert_eq!(
        client.call_count(),
        0,
        "LLM must not be called when no secrets found"
    );
}
