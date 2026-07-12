#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    let api_key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };

    let mut config = lx_config::Config::default();
    config.llm.api_key = Some(api_key);

    let client = lx_llm::client_from_config(&config, false).expect("client");

    let pem = include_str!("fixtures/sample.pem");
    let out = lxcert::run::run(pem, &config, client.as_ref()).expect("run should succeed");

    assert!(!out.subject.is_empty(), "subject must not be empty");
    assert!(!out.issuer.is_empty(), "issuer must not be empty");
    assert!(!out.valid_until.is_empty(), "valid_until must not be empty");
    assert!(!out.notes.is_empty(), "notes must not be empty");
}
