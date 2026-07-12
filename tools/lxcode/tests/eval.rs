#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_rust_function_generated() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxcode::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "a function that reverses a string",
        Some("rust"),
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.code.is_empty(), "code must not be empty");
    assert_eq!(out.language, "rust", "language must be rust");
    // Expect idiomatic Rust — some form of fn definition
    assert!(
        out.code.contains("fn "),
        "rust code should contain a function definition: {}",
        out.code
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_python_code_generated_with_hint() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxcode::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "read a CSV file and print each row",
        Some("python"),
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.code.is_empty());
    assert_eq!(out.language, "python");
    assert!(
        out.code.contains("csv") || out.code.contains("open"),
        "python CSV code should reference csv or open: {}",
        out.code
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_language_auto_detected() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxcode::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    // Description implies SQL — model should auto-detect
    let out = run(
        "select all users from the users table where age > 18",
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.code.is_empty());
    assert!(!out.language.is_empty());
    let lower = out.language.to_lowercase();
    assert!(
        lower.contains("sql"),
        "language should be sql for a SQL description: {}",
        out.language
    );
}
