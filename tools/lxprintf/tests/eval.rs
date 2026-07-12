// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must be named eval_* and carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxprintf::run::run;

    let mut config = Config::load().unwrap();
    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set for eval tests");
    config.llm.api_key = Some(api_key);

    let client = client_from_config(&config, false).unwrap();
    let out = run("ISO date and time", &config, client.as_ref()).unwrap();

    assert!(!out.format.is_empty(), "format must not be empty");
    assert!(!out.explanation.is_empty(), "explanation must not be empty");

    // Minimum quality: the format string should contain at least one % specifier
    assert!(
        out.format.contains('%'),
        "format should contain at least one specifier: {}",
        out.format
    );
}
