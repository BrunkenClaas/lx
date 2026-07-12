#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxsed::run::run;

    let config = Config::load().expect("config must load");
    let client = client_from_config(&config, false).expect("client must build");
    let (out, _findings) = run(
        "print lines where the first field equals ERROR",
        &config,
        client.as_ref(),
    )
    .expect("run must succeed");

    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(
        out.tool == "awk" || out.tool == "sed",
        "tool must be 'awk' or 'sed', got: {}",
        out.tool
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_sed_one_liner() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxsed::run::run;

    let config = Config::load().expect("config must load");
    let client = client_from_config(&config, false).expect("client must build");
    let (out, _findings) = run(
        "replace all occurrences of hello with world",
        &config,
        client.as_ref(),
    )
    .expect("run must succeed");

    assert!(!out.command.is_empty());
    // sed is a reasonable choice here, but awk is also valid
    assert!(out.tool == "awk" || out.tool == "sed");
}
