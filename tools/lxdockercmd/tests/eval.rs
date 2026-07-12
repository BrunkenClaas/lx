#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdockercmd::run::run;

    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set");
    let mut config = Config::load().unwrap_or_default();
    config.llm.api_key = Some(api_key);

    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run("run nginx on port 8080", &config, client.as_ref())
        .expect("run() must succeed with a real LLM");

    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(
        out.command.starts_with("docker "),
        "command must start with 'docker ': {}",
        out.command
    );
    let json = serde_json::to_string(&out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["command"].is_string());
    assert!(parsed["dangerous"].is_boolean());
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_dangerous_command_flagged() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdockercmd::run::run;

    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set");
    let mut config = Config::load().unwrap_or_default();
    config.llm.api_key = Some(api_key);

    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "run a container with full host privileges",
        &config,
        client.as_ref(),
    )
    .expect("run() must succeed with a real LLM");

    assert!(!out.command.is_empty());
    assert!(
        out.dangerous,
        "a privileged container run should be flagged as dangerous: {}",
        out.command
    );
}
