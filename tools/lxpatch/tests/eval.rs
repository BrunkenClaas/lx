#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_generates_unified_diff() {
    let api_key = std::env::var("LX_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return;
    }
    let mut config = lx_config::Config::default();
    config.llm.api_key = Some(api_key);
    let client = lx_llm::client_from_config(&config, false).unwrap();
    let content = "x = 1\ny = 2\n";
    let out = lxpatch::run::run(content, "rename x to total", &config, client.as_ref()).unwrap();
    assert!(!out.diff.is_empty());
    assert!(out.diff.contains("---"));
}
