#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_json_to_yaml_structure() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;
    use lxconv::run::{run, Format};

    let config = Config::load().expect("config");
    let client = lx_llm::client_from_config(&config, false).expect("client");
    let input = r#"{"region":"west","count":42}"#;
    let out = run(input, &Format::Yaml, &config, client.as_ref()).expect("run");
    // Structure check — content must be non-empty and contain key names.
    assert!(!out.content.is_empty(), "content must not be empty");
    // Rough YAML check: should contain the key name.
    assert!(
        out.content.contains("region") || out.content.contains("count"),
        "YAML output should contain field names: {}",
        out.content
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_csv_to_json_structure() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;
    use lxconv::run::{run, Format};

    let config = Config::load().expect("config");
    let client = lx_llm::client_from_config(&config, false).expect("client");
    let input = "city,pop\nBerlin,3700000\nParis,2100000\n";
    let out = run(input, &Format::Json, &config, client.as_ref()).expect("run");
    assert!(!out.content.is_empty());
    // Local conversion handles this — verify it's valid JSON.
    serde_json::from_str::<serde_json::Value>(&out.content).expect("valid json");
}
