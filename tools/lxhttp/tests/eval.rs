#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let result = lxhttp::run::run(
        "> GET /api/users HTTP/2\n< HTTP/2 401\n{\"error\":\"invalid_token\"}",
        &config,
        client.as_ref(),
    );
    assert!(result.is_ok());
    let out = result.unwrap();
    assert!(!out.explanation.is_empty());
}
