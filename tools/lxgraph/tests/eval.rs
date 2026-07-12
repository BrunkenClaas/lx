#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lxgraph::run::run;

    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set for eval tests");
    let mut config = Config::default();
    config.llm.api_key = Some(api_key);

    let input = "Jan,100\nFeb,150\nMar,130\nApr,200\n";
    let client = lx_llm::client_from_config(&config, false).expect("failed to build LLM client");
    let out = run(input, &config, client.as_ref()).expect("run() must succeed");

    assert!(!out.chart.is_empty(), "chart must not be empty");
    assert!(out.chart.contains('|'), "chart must contain bar separator");
    assert!(!out.series.is_empty(), "series must not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_plain_numbers_input() {
    use lx_config::Config;
    use lxgraph::run::run;

    let api_key = std::env::var("LX_API_KEY").expect("LX_API_KEY must be set for eval tests");
    let mut config = Config::default();
    config.llm.api_key = Some(api_key);

    let input = "42\n17\n88\n35\n60\n";
    let client = lx_llm::client_from_config(&config, false).expect("failed to build LLM client");
    let out = run(input, &config, client.as_ref()).expect("run() must succeed for plain numbers");

    assert!(!out.chart.is_empty(), "chart must not be empty");
    // 5 input values should produce 5 chart lines
    let line_count = out.chart.lines().count();
    assert_eq!(line_count, 5, "chart should have 5 lines for 5 data points");
}
