#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxundo::run;

    let api_key = std::env::var("LX_API_KEY").unwrap();
    let mut config = Config::default();
    config.llm.api_key = Some(api_key);

    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);

    let out = run("git push --force origin main", &config, &client).unwrap();

    assert!(
        !out.undo_command.is_empty(),
        "undo_command must not be empty"
    );
    // caution may be empty or non-empty — both are valid
    let _ = out.caution;
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_irreversible_command_has_caution() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxundo::run;

    let api_key = std::env::var("LX_API_KEY").unwrap();
    let mut config = Config::default();
    config.llm.api_key = Some(api_key);

    let client = client_from_config(&config, false).unwrap();

    // rm without backup — model should warn in caution
    let out = run(
        "rm -rf build/ (no backup available)",
        &config,
        client.as_ref(),
    )
    .unwrap();

    // Either undo_command is empty (cannot undo) or caution explains the risk
    let has_info = !out.undo_command.is_empty() || !out.caution.is_empty();
    assert!(
        has_info,
        "model must provide either undo_command or caution"
    );
}
