#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let result = lxip::run::run(
        "add a static route to 10.0.0.0/24 via 192.168.1.254",
        "",
        "linux",
        &config,
        client.as_ref(),
    );
    assert!(result.is_ok());
    let (out, _) = result.unwrap();
    assert!(!out.command.is_empty());
}
