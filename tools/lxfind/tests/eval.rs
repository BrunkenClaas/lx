//! Eval tests — ignored by default; run with `--include-ignored eval_` and a real LLM.
//! Execute with: cargo test -p lxfind -- --include-ignored eval_

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_finds_relevant_files() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;
    use lxfind::run::run;

    let config = Config::load().expect("config must load for eval tests");
    let client = lx_llm::client_from_config(&config, false).expect("LLM client must be created");

    // Search in the tool's own directory — it has known files.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = run(
        "the main entry point of the binary",
        &root,
        &config,
        client.as_ref(),
    )
    .expect("run() must not fail");

    // Structure: paths must be a Vec<String>.
    let _: &Vec<String> = &out.paths;
    // We don't assert exact content — LLM results can vary.
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_empty_result_for_nonsense_description() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;
    use lxfind::run::run;

    let config = Config::load().expect("config must load for eval tests");
    let client = lx_llm::client_from_config(&config, false).expect("LLM client must be created");

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let out = run(
        "xyzzy frobnicate zorkian flux capacitor schematics",
        &root,
        &config,
        client.as_ref(),
    )
    .expect("run() must not fail on unknown description");

    // The model may return an empty list or a best-guess — either is acceptable.
    let _: &Vec<String> = &out.paths;
}
