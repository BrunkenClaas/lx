// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must be named eval_* and carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_generates_json_data() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxmock::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").unwrap();
    config.llm.api_key = Some(api_key);
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "3 products with id, name, and price as JSON",
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.data.is_empty(), "data must not be empty");
    assert!(!out.format.is_empty(), "format must not be empty");

    // The format hint should indicate JSON
    let lower = out.format.to_lowercase();
    assert!(
        lower.contains("json"),
        "format should be json for JSON request, got: {}",
        out.format
    );
}
