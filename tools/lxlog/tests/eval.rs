#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxlog::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let log = include_str!("fixtures/sample_app.log");
    let out = run(log, &config, client.as_ref()).unwrap();

    // Structure checks — not exact text.
    assert!(!out.summary.is_empty(), "summary must be non-empty");
    for anomaly in &out.anomalies {
        assert!(
            !anomaly.message.is_empty(),
            "anomaly message must be non-empty"
        );
        assert!(!anomaly.level.is_empty(), "anomaly level must be non-empty");
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_detects_errors_in_sample_log() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxlog::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let log = include_str!("fixtures/sample_app.log");
    let out = run(log, &config, client.as_ref()).unwrap();

    // The sample log has cache errors and a fatal exception — at least one anomaly expected.
    assert!(
        !out.anomalies.is_empty(),
        "expected anomalies in log with known errors, got summary: {}",
        out.summary
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_sensitive_log() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxlog::run::run;

    let config = Config::load().unwrap();
    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);
    let log = include_str!("fixtures/log_with_sensitive_data.log");
    let _ = run(log, &config, &client);
    let sent = client.last_user_message();
    assert!(
        !sent.contains("sk-abcdefghijklmnopqrstuvwxyz123456"),
        "raw deploy key must not reach LLM"
    );
    assert!(
        sent.contains("[REDACTED]"),
        "redacted placeholder must be present"
    );
}
