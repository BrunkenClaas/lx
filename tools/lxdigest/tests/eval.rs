#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdigest::run::run;
    use std::path::Path;

    if std::env::var("LX_API_KEY").is_err() {
        return;
    }

    let config = Config::load().unwrap_or_else(|_| Config::default());
    let client = client_from_config(&config, false).unwrap();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let out = run(manifest_dir, false, &config, client.as_ref()).unwrap();

    // Check structure, not exact text.
    assert!(!out.summary.is_empty(), "summary must not be empty");
    assert!(out.summary.len() > 10, "summary must be a real sentence");
    let _: &Vec<String> = &out.files;
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_json_round_trips() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdigest::run::run;
    use std::path::Path;

    if std::env::var("LX_API_KEY").is_err() {
        return;
    }

    let config = Config::load().unwrap_or_else(|_| Config::default());
    let client = client_from_config(&config, false).unwrap();
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let out = run(manifest_dir, false, &config, client.as_ref()).unwrap();

    let json = serde_json::to_string(&out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["summary"].is_string());
    assert!(parsed["files"].is_array());
}
