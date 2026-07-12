#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_generates_mv_script() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let out = lxrename::run::run(
        "testFoo.py\ntestBar.py",
        "rename to snake_case",
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();
    assert!(!out.renames.is_empty());
    assert!(!out.script.is_empty());
}
