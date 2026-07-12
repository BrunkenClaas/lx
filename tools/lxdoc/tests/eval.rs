// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_documents_python_function() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdoc::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();

    let code = include_str!("fixtures/sample_function.py");
    let out = run(code, &config, client.as_ref()).unwrap();

    assert!(!out.code.is_empty(), "code must not be empty");
    // The LLM must preserve the original functions.
    assert!(out.code.contains("def add"), "add function must be present");
    assert!(
        out.code.contains("def multiply"),
        "multiply function must be present"
    );
    // Some form of docstring must have been inserted.
    let lower = out.code.to_lowercase();
    assert!(
        lower.contains("\"\"\"") || lower.contains("'''") || lower.contains("#"),
        "documented Python code should contain docstrings or comments: {}",
        &out.code[..out.code.len().min(300)]
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_documents_rust_function() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdoc::run::{run_with_style, DocStyle};

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();

    let code = include_str!("fixtures/sample_function.rs");
    let out = run_with_style(code, &config, client.as_ref(), &DocStyle::Rustdoc).unwrap();

    assert!(!out.code.is_empty(), "code must not be empty");
    assert!(
        out.code.contains("pub fn add"),
        "add function must be present"
    );
    assert!(
        out.code.contains("///"),
        "rustdoc style should insert /// comments: {}",
        &out.code[..out.code.len().min(300)]
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_documents_javascript_function() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdoc::run::{run_with_style, DocStyle};

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();

    let code = include_str!("fixtures/sample_function.js");
    let out = run_with_style(code, &config, client.as_ref(), &DocStyle::Javadoc).unwrap();

    assert!(!out.code.is_empty(), "code must not be empty");
    assert!(
        out.code.contains("function add"),
        "add function must be present"
    );
    assert!(
        out.code.contains("/**") || out.code.contains("*"),
        "javadoc style should insert /** ... */ comments"
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure_is_valid_json() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxdoc::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();

    let out = run("def noop(): pass", &config, client.as_ref()).unwrap();
    // Serialise and parse to confirm the output is a valid JSON object.
    let json = serde_json::to_string(&out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("code").is_some(),
        "JSON output must have 'code' key"
    );
}
