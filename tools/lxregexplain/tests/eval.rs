// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_explain_date_regex() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxregexplain::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set");
    config.llm.api_key = Some(api_key);

    let client = client_from_config(&config, false).unwrap();
    let out = run(r"^\d{4}-\d{2}-\d{2}$", &config, client.as_ref()).unwrap();

    assert!(!out.explanation.is_empty(), "explanation must not be empty");
    assert!(!out.parts.is_empty(), "parts must not be empty");

    // Minimum semantic quality: should mention date or digits
    let lower = out.explanation.to_lowercase();
    assert!(
        lower.contains("date") || lower.contains("digit") || lower.contains("year"),
        "explanation should describe a date pattern: {}",
        out.explanation
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_explain_hex_color_regex() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxregexplain::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set");
    config.llm.api_key = Some(api_key);

    let client = client_from_config(&config, false).unwrap();
    let out = run(r"#[0-9a-fA-F]{6}", &config, client.as_ref()).unwrap();

    assert!(!out.explanation.is_empty(), "explanation must not be empty");
    assert!(!out.parts.is_empty(), "parts must not be empty");

    let lower = out.explanation.to_lowercase();
    assert!(
        lower.contains("hex") || lower.contains("color") || lower.contains("colour"),
        "explanation should describe a hex color: {}",
        out.explanation
    );
}
