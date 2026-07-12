#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxffmpeg::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").unwrap();
    config.llm.api_key = Some(api_key);
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run("convert video.mp4 to audio mp3", &config, client.as_ref())
        .expect("run() must succeed with a real LLM");

    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(
        out.command.contains("ffmpeg"),
        "command must contain 'ffmpeg': {}",
        out.command
    );
    let json = serde_json::to_string(&out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["command"].is_string());
    assert!(parsed["dangerous"].is_boolean());
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_complex_command_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxffmpeg::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").unwrap();
    config.llm.api_key = Some(api_key);
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "trim video.mp4 from 10 seconds to 30 seconds",
        &config,
        client.as_ref(),
    )
    .expect("run() must succeed with a real LLM");

    assert!(!out.command.is_empty());
    assert!(
        out.command.contains("ffmpeg"),
        "command must contain 'ffmpeg': {}",
        out.command
    );
}
