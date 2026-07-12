#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;

    let config = Config::load().expect("config must load");
    let client = client_from_config(&config, false).expect("client must build");
    let (out, _findings) = lxdockerfile::run::run(
        "Node.js 18 app with npm, exposes port 3000",
        None,
        &config,
        client.as_ref(),
    )
    .expect("run must succeed");

    assert!(!out.content.is_empty(), "content must not be empty");
    assert!(
        out.content.contains("FROM"),
        "content must contain a FROM instruction"
    );
}
