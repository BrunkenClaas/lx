#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_basic_corrections() {
    let key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };
    use lx_config::Config;

    let mut config = Config::default();
    config.llm.api_key = Some(key);
    let client = lx_llm::client_from_config(&config, false).unwrap();
    let out = lxproof::run::run(
        "I recieved you're letter yesturday.",
        &config,
        client.as_ref(),
    )
    .unwrap();
    assert!(!out.text.is_empty(), "text must be non-empty");
    assert!(
        !out.changes.is_empty(),
        "should detect at least one correction in input with errors"
    );
    assert!(
        !out.text.contains("recieved"),
        "corrected text should fix 'recieved'"
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_clean_text_no_changes() {
    let key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };
    use lx_config::Config;

    let mut config = Config::default();
    config.llm.api_key = Some(key);
    let client = lx_llm::client_from_config(&config, false).unwrap();
    let input = "The quick brown fox jumps over the lazy dog.";
    let out = lxproof::run::run(input, &config, client.as_ref()).unwrap();
    assert_eq!(out.text, input, "clean text should be returned unchanged");
    assert!(
        out.changes.is_empty(),
        "clean text should produce no changes"
    );
}
