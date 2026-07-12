use lx_config::Config;
use lxtodo::run::run;

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let client = lx_llm::client_from_config(&Config::default(), false).unwrap();
    let input = std::fs::read_to_string("tests/fixtures/code_with_todos.rs")
        .unwrap_or_else(|_| "// TODO: implement this\n// FIXME: broken\nfn main() {}".to_string());
    let out = run(&input, &Config::default(), client.as_ref()).unwrap();
    // Structure check: todos is a vec (may be empty for clean files, but should exist).
    // For our fixture it should find at least one item.
    assert!(
        !out.todos.is_empty(),
        "expected at least one TODO in fixture, got: {:?}",
        out.todos
    );
    // Each item must have non-empty text.
    for item in &out.todos {
        assert!(!item.text.is_empty(), "todo text must not be empty");
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_empty_input_returns_empty_todos() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let client = lx_llm::client_from_config(&Config::default(), false).unwrap();
    let out = run("", &Config::default(), client.as_ref()).unwrap();
    assert!(out.todos.is_empty(), "empty input must return empty todos");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_json_output_is_valid() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    let client = lx_llm::client_from_config(&Config::default(), false).unwrap();
    let out = run(
        "// TODO: validate JSON output\n// FIXME: error handling missing",
        &Config::default(),
        client.as_ref(),
    )
    .unwrap();
    let json = serde_json::to_string_pretty(&out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("todos").is_some(),
        "JSON must have 'todos' field"
    );
}
