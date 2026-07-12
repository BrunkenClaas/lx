#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_basic_table_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxtable::run::run;

    let config = Config::load().expect("config must load");
    let client = client_from_config(&config, false).expect("client must be created");
    let out = run(
        "Alice is 30 years old and works as an Engineer. Bob is 25 and is a Designer. Carol, age 28, works as a Manager.",
        &config,
        client.as_ref(),
    )
    .expect("run must succeed");

    assert!(!out.columns.is_empty(), "columns must not be empty");
    assert!(!out.rows.is_empty(), "rows must not be empty");
    // Each row must have the same number of cells as columns.
    for row in &out.rows {
        assert_eq!(
            row.len(),
            out.columns.len(),
            "row length must match column count"
        );
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_quarterly_data() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxtable::run::run;

    let config = Config::load().expect("config must load");
    let client = client_from_config(&config, false).expect("client must be created");
    let out = run(
        "Q1 revenue was $1.2M, Q2 was $1.5M, Q3 reached $1.8M, Q4 closed at $2.1M.",
        &config,
        client.as_ref(),
    )
    .expect("run must succeed");

    assert!(!out.columns.is_empty(), "columns must not be empty");
    assert_eq!(out.rows.len(), 4, "should extract 4 quarters");
}
