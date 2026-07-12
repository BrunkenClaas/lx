#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lxconf::run::{run, ConfigMode};

    let api_key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };

    let mut config = Config::default();
    config.llm.api_key = Some(api_key);

    let client = lx_llm::client_from_config(&config, false).expect("client should build");

    let input = include_str!("fixtures/broken_toml.toml");
    let (out, _warnings) =
        run(input, None, ConfigMode::Audit, &config, client.as_ref()).expect("run should succeed");

    // Verify structure — not exact text (LLM output varies).
    for f in &out.findings {
        assert!(
            ["error", "warning", "info"].contains(&f.severity.as_str()),
            "unexpected severity: {}",
            f.severity
        );
        assert!(!f.message.is_empty(), "finding message must not be empty");
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_valid_config_returns_empty_or_few_findings() {
    use lx_config::Config;
    use lxconf::run::{run, ConfigMode};

    let api_key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };

    let mut config = Config::default();
    config.llm.api_key = Some(api_key);

    let client = lx_llm::client_from_config(&config, false).expect("client should build");

    let input = include_str!("fixtures/valid_config.toml");
    let (out, _warnings) =
        run(input, None, ConfigMode::Audit, &config, client.as_ref()).expect("run should succeed");

    // A clearly valid config should produce zero or very few findings.
    assert!(
        out.findings.len() <= 2,
        "valid config should produce 0-2 findings, got: {}",
        out.findings.len()
    );
}
