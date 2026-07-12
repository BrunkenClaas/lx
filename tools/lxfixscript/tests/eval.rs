#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_fixes_broken_script() {
    let api_key = std::env::var("LX_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return;
    }
    let mut config = lx_config::Config::default();
    config.llm.api_key = Some(api_key);
    let client = lx_llm::client_from_config(&config, false).unwrap();
    let broken = "#!/bin/bash\nif [ -f x ]; then\necho found\nfi\nfi\n";
    let out =
        lxfixscript::run::run(broken, "unexpected fi", "linux", &config, client.as_ref()).unwrap();
    assert!(!out.script.is_empty());
}
