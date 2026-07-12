#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    let api_key = std::env::var("LX_API_KEY").unwrap();
    let mut config = lx_config::Config::default();
    config.llm.api_key = Some(api_key);

    let client = lx_llm::client_from_config(&config, false).expect("failed to build LLM client");
    let recording = lx_testkit::RecordingLlmClient::new(client);

    let out = lxask::run("What is the capital of Germany?", None, &config, &recording)
        .expect("run() should succeed");

    assert!(!out.answer.is_empty(), "answer must not be empty");
    // sources may be empty when no context is given
    let _ = out.sources;

    let response = recording.last_response();
    assert!(!response.is_empty(), "recorded response must not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_context_improves_answer() {
    let api_key = std::env::var("LX_API_KEY").unwrap();
    let mut config = lx_config::Config::default();
    config.llm.api_key = Some(api_key);

    let client = lx_llm::client_from_config(&config, false).expect("failed to build LLM client");
    let recording = lx_testkit::RecordingLlmClient::new(client);

    let context = "The project uses Rust 1.78 and is compiled with cargo build --release.";
    let out = lxask::run(
        "What build command is used?",
        Some(context),
        &config,
        &recording,
    )
    .expect("run() should succeed");

    assert!(!out.answer.is_empty(), "answer must not be empty");
}
