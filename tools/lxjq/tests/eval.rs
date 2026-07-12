#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_simple_expression_generated() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxjq::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "extract the name field from each element of the users array",
        None,
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.expression.is_empty(), "expression must not be empty");
    assert!(
        out.expression.contains(".name") || out.expression.contains("name"),
        "expression should reference the name field: {}",
        out.expression
    );
    assert!(!out.explanation.is_empty(), "explanation must not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_expression_with_json_context() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxjq::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let ctx =
        r#"{"items":[{"id":1,"price":9.99,"active":true},{"id":2,"price":4.99,"active":false}]}"#;
    let (out, _findings) = run(
        "filter to active items and return their prices",
        Some(ctx),
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.expression.is_empty());
    assert!(
        out.expression.contains("active") || out.expression.contains("price"),
        "expression should reference active or price fields: {}",
        out.expression
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_explanation_is_meaningful() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxjq::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "count the number of keys in an object",
        None,
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.explanation.is_empty());
    // Explanation should be a non-trivial sentence
    assert!(
        out.explanation.len() > 10,
        "explanation should be a meaningful sentence: {}",
        out.explanation
    );
}
