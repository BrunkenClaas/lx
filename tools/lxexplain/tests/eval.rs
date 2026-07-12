// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must be named eval_* and carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_explain_tar_command() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxexplain::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run("tar -xzf archive.tar.gz", &config, client.as_ref()).unwrap();

    assert!(!out.summary.is_empty(), "summary must not be empty");
    assert!(!out.details.is_empty(), "details must not be empty");

    // Minimum semantic quality: the response should mention tar or extract
    let lower = out.summary.to_lowercase();
    assert!(
        lower.contains("extract") || lower.contains("tar") || lower.contains("archive"),
        "summary should describe extraction: {}",
        out.summary
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_explain_error_message() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxexplain::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "ENOENT: no such file or directory, open '/app/config.json'",
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.summary.is_empty());
    let lower = out.summary.to_lowercase();
    assert!(
        lower.contains("file") || lower.contains("exist") || lower.contains("found"),
        "should describe missing file: {}",
        out.summary
    );
}
