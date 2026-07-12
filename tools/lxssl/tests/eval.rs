#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let result = lxssl::run::run(
        "verify error:num=10:certificate has expired\nnotAfter=Jan 1 00:00:00 2023 GMT",
        "expired.example.com",
        &config,
        client.as_ref(),
    );
    assert!(result.is_ok());
    let out = result.unwrap();
    assert!(!out.explanation.is_empty());
}
