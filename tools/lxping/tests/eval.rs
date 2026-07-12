#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let result = lxping::run::run(
        "PING google.com (142.250.80.46)\nRequest timeout for icmp_seq 0\n100% packet loss",
        &config,
        client.as_ref(),
    );
    assert!(result.is_ok());
    let out = result.unwrap();
    assert!(!out.explanation.is_empty());
    assert!(["network", "host", "dns", "ok"].contains(&out.verdict.as_str()));
}
