#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_changelog_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxclog::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let log = include_str!("fixtures/sample_git_log.txt");
    let (out, _warnings) = run(log, &config, client.as_ref()).unwrap();

    assert!(!out.entries.is_empty(), "entries must not be empty");
    let first = &out.entries[0];
    assert!(!first.version.is_empty(), "version must not be empty");

    // At least one change category must have entries
    let total = first.added.len()
        + first.changed.len()
        + first.deprecated.len()
        + first.removed.len()
        + first.fixed.len()
        + first.security.len();
    assert!(total > 0, "at least one change must be classified");

    // Verify the plain output is valid Keep-a-Changelog markdown
    let plain = out.to_plain();
    assert!(
        plain.contains("# Changelog"),
        "plain output must contain header"
    );
    assert!(
        plain.contains("## ["),
        "plain output must contain version heading"
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_tagged_log_produces_multiple_entries() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxclog::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let log = include_str!("fixtures/git_log_with_tags.txt");
    let (out, _warnings) = run(log, &config, client.as_ref()).unwrap();

    assert!(
        out.entries.len() >= 2,
        "tagged log should produce multiple entries, got {}",
        out.entries.len()
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_secret_in_log() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxclog::run::run;

    let config = Config::load().unwrap();
    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);
    let log = include_str!("fixtures/git_log_with_secret.txt");
    let _ = run(log, &config, &client);
    let sent = client.last_user_message();
    assert!(
        !sent.contains("sk-abcdefghijklmnopqrstuvwxyz"),
        "raw secret must not reach LLM"
    );
    assert!(
        sent.contains("[REDACTED]"),
        "redacted placeholder must be present"
    );
}
