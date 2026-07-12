#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxkill::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set for eval tests");
    config.llm.api_key = Some(api_key);

    let client = client_from_config(&config, false).unwrap();
    let (out, _dangerous) = run(
        "process listening on port 8080",
        "",
        "linux",
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(!out.target.is_empty(), "target must not be empty");
    assert!(!out.reason.is_empty(), "reason must not be empty");

    // Result should involve port-related tools
    let lower = out.command.to_lowercase();
    assert!(
        lower.contains("8080")
            || lower.contains("lsof")
            || lower.contains("fuser")
            || lower.contains("ss")
            || lower.contains("netstat"),
        "command should involve port lookup: {}",
        out.command
    );
}
