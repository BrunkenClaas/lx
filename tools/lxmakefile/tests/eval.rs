#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lxmakefile::run::run;

    if std::env::var("LX_API_KEY").is_err() {
        return;
    }

    let client = lx_llm::client_from_config(&Config::default(), false).unwrap();
    let (out, _findings) = run(
        "build, test, and clean a Rust project",
        None,
        &Config::default(),
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.content.is_empty(), "content must not be empty");
    // The generated content should look like a Makefile
    assert!(
        out.content.contains(':'),
        "Makefile content should contain target definitions with ':'"
    );
}
