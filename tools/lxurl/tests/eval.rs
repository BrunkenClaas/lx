#![forbid(unsafe_code)]

#[test]
#[ignore = "eval: requires LX_API_KEY and network"]
fn eval_url_answer_structure() {
    use lx_config::Config;
    use lx_testkit::RecordingLlmClient;
    use lxurl::run::run;

    let config = Config::load().expect("config must load for eval");
    let inner = lx_llm::client_from_config(&config, false).expect("client must build");
    let client = RecordingLlmClient::new(inner);

    let out = run(
        "https://example.com",
        "What is this page about?",
        &config,
        &client,
    )
    .expect("eval run must succeed");

    assert!(!out.answer.is_empty(), "answer must not be empty");
    assert_eq!(out.url, "https://example.com");
}
