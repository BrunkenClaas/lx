#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_basic_repair() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxjson::run::run;

    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    // Trailing comma — should be repaired locally without needing the LLM.
    let out = run(r#"{"name": "Alice", "age": 30,}"#, &config, client.as_ref()).unwrap();
    assert!(!out.json.is_empty(), "json must not be empty");
    serde_json::from_str::<serde_json::Value>(&out.json).unwrap();
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_single_quotes_repair() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxjson::run::run;

    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "{'host': 'localhost', 'port': 8080}",
        &config,
        client.as_ref(),
    )
    .unwrap();
    assert!(!out.json.is_empty());
    let v: serde_json::Value = serde_json::from_str(&out.json).unwrap();
    assert_eq!(v["host"], "localhost");
    assert_eq!(v["port"], 8080);
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_method_field_is_present() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxjson::run::run;

    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let out = run(r#"{"key": "value"}"#, &config, client.as_ref()).unwrap();
    assert!(
        out.method == "local" || out.method == "llm",
        "method must be 'local' or 'llm', got: {}",
        out.method
    );
}
