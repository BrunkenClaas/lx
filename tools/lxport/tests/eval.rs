#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_port_22_is_ssh() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let out = lxport::run::run(22, "", &config, client.as_ref()).unwrap();
    assert!(!out.explanation.is_empty());
    let lower = out.explanation.to_lowercase();
    assert!(lower.contains("ssh") || lower.contains("secure shell") || lower.contains("remote"));
}
