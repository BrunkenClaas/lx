#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxkubectl::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set for eval tests");
    config.llm.api_key = Some(api_key);
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "list all pods in the default namespace",
        &config,
        client.as_ref(),
    )
    .expect("run() must succeed with a real LLM");

    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(
        out.command.starts_with("kubectl "),
        "command must start with 'kubectl ': {}",
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
    use lxkubectl::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set for eval tests");
    config.llm.api_key = Some(api_key);
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "delete all pods with label app=nginx in the staging namespace",
        &config,
        client.as_ref(),
    )
    .expect("run() must succeed with a real LLM");

    assert!(!out.command.is_empty());
    assert!(
        out.dangerous,
        "a delete command should be flagged as dangerous: {}",
        out.command
    );
}
