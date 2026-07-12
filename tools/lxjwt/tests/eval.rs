#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxjwt::run::run;

    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set");
    let mut config = Config::load().unwrap();
    config.llm.api_key = Some(api_key);

    let client = client_from_config(&config, false).unwrap();
    let jwt = include_str!("fixtures/sample.jwt").trim();
    let out = run(jwt, &config, client.as_ref()).unwrap();

    assert!(!out.header.is_empty(), "header must not be empty");
    assert!(!out.payload.is_empty(), "payload must not be empty");
    assert!(!out.notes.is_empty(), "notes must not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_signature_never_reaches_llm() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lx_testkit::RecordingLlmClient;
    use lxjwt::run::run;

    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set");
    let mut config = Config::load().unwrap();
    config.llm.api_key = Some(api_key);

    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);
    let jwt = include_str!("fixtures/sample.jwt").trim();
    let _ = run(jwt, &config, &client);

    let sent = client.last_user_message();
    // The raw JWT base64url header must not appear in the user message.
    assert!(
        !sent.contains("eyJhbGci"),
        "raw JWT must not reach LLM: {}",
        sent
    );
    // The decoded content should contain the header JSON fields.
    assert!(
        sent.contains("HS256") || sent.contains("alg"),
        "decoded header fields should be present: {}",
        sent
    );
}
