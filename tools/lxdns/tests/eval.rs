#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let result = lxdns::run::run(
        "; <<>> DiG 9.18.0 <<>> example.invalid\n;; ->>HEADER<<- opcode: QUERY, status: NXDOMAIN",
        "",
        &config,
        client.as_ref(),
    );
    assert!(result.is_ok());
    let out = result.unwrap();
    assert!(!out.explanation.is_empty());
    assert!(!out.likely_cause.is_empty());
    assert!(!out.suggested_fix.is_empty());
}
