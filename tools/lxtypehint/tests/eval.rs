#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    use lx_config::Config;
    use lx_llm::client_from_config;

    let config = Config::load().expect("config must load");
    let client = client_from_config(&config, false).expect("client must build");
    let input = include_str!("fixtures/basic.py");
    let (out, _findings) = lxtypehint::run::run(input, &config, client.as_ref())
        .expect("run must succeed with real LLM");

    // Structure checks — not exact text (models vary).
    assert!(!out.code.is_empty(), "code must not be empty");
    // The annotated code should contain some type annotation marker.
    assert!(
        out.code.contains(':') || out.code.contains("->"),
        "annotated code should contain type annotations"
    );
}
