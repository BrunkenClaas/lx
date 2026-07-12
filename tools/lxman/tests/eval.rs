// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must be named eval_* and carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_grep_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxman::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run("grep", &config, client.as_ref()).unwrap();

    assert!(!out.summary.is_empty(), "summary must not be empty");
    assert!(!out.examples.is_empty(), "examples must not be empty");

    // Minimum semantic quality: summary should mention grep or search or pattern
    let lower = out.summary.to_lowercase();
    assert!(
        lower.contains("grep") || lower.contains("search") || lower.contains("pattern"),
        "summary should describe grep: {}",
        out.summary
    );

    // Examples should look like commands
    for ex in &out.examples {
        assert!(
            ex.contains("grep"),
            "each example should reference grep: {ex}"
        );
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_curl_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxman::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run("curl", &config, client.as_ref()).unwrap();

    assert!(!out.summary.is_empty());
    assert!(!out.examples.is_empty());

    let lower = out.summary.to_lowercase();
    assert!(
        lower.contains("curl")
            || lower.contains("transfer")
            || lower.contains("http")
            || lower.contains("url"),
        "summary should describe curl: {}",
        out.summary
    );
}
