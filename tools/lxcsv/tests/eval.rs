#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_basic_output_structure() {
    use lx_config::Config;
    use lxcsv::run::{run, Output};

    if std::env::var("LX_API_KEY").is_err() {
        return;
    }

    let client = lx_llm::client_from_config(&Config::default(), false).expect("client from config");
    let csv = include_str!("fixtures/sales.csv");
    let out: Output = run(
        csv,
        "Which product has the highest revenue?",
        &Config::default(),
        client.as_ref(),
    )
    .expect("run succeeded");

    assert!(!out.answer.is_empty(), "answer must not be empty");
    assert!(!out.used_rows.is_empty(), "used_rows must not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_aggregate_question() {
    use lx_config::Config;
    use lxcsv::run::run;

    if std::env::var("LX_API_KEY").is_err() {
        return;
    }

    let client = lx_llm::client_from_config(&Config::default(), false).expect("client from config");
    let csv = include_str!("fixtures/employees.csv");
    let out = run(
        csv,
        "What is the average salary?",
        &Config::default(),
        client.as_ref(),
    )
    .expect("run succeeded");

    assert!(
        out.answer.to_lowercase().contains("salary") || out.answer.contains("85"),
        "answer should relate to salary: {}",
        out.answer
    );
}
