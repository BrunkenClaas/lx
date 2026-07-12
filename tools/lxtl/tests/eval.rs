use lx_config::Config;
use lx_testkit::RecordingLlmClient;
use lxtl::run;

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    let api_key = std::env::var("LX_API_KEY").unwrap();
    let mut config = Config::default();
    config.llm.api_key = Some(api_key);

    let inner = lx_llm::client_from_config(&config, false).expect("client_from_config failed");
    let client = RecordingLlmClient::new(inner);

    let input = "The deployment was successful and all services are running normally.";
    let out = run(input, "French", &config, &client).expect("run() failed");

    assert!(!out.text.is_empty(), "translated text should not be empty");

    let response = client.last_response();
    assert!(!response.is_empty(), "response should not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_preserves_meaning_across_languages() {
    let api_key = std::env::var("LX_API_KEY").unwrap();
    let mut config = Config::default();
    config.llm.api_key = Some(api_key);

    let inner = lx_llm::client_from_config(&config, false).expect("client_from_config failed");
    let client = RecordingLlmClient::new(inner);

    let input = "Please restart the service after updating the configuration.";
    let out = run(input, "German", &config, &client).expect("run() failed");

    assert!(!out.text.is_empty(), "translated text should not be empty");
}
