#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let result = lxfirewall::run::run(
        "allow HTTP and HTTPS from anywhere",
        "",
        "linux",
        &config,
        client.as_ref(),
    );
    assert!(result.is_ok());
    let (out, _) = result.unwrap();
    assert!(!out.command.is_empty());
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_explain_mode() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let fixture = include_str!("fixtures/iptables_rules.txt");
    let result = lxfirewall::run::run("", fixture, "linux", &config, client.as_ref());
    assert!(result.is_ok());
    let (out, explain_mode) = result.unwrap();
    assert!(explain_mode);
    assert!(!out.explanation.is_empty());
}
