#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_corrects_typo() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let out = lxfixcmd::run::run("git psh origin main", "", &config, client.as_ref()).unwrap();
    assert!(!out.command.is_empty());
    assert!(out.command.contains("push") || out.command.contains("git"));
}
