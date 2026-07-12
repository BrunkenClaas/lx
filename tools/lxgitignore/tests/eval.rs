#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_rust_project_structure() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;

    let config = Config::load().expect("Config must load for eval tests");
    let client = lx_llm::client_from_config(&config, false).expect("LLM client must build");
    let input = "Cargo.toml\nCargo.lock\nsrc/\n  src/main.rs\n  src/lib.rs\ntarget/\ntests/\n  tests/integration.rs\n.github/\n  .github/workflows/\n    .github/workflows/ci.yml\n";

    let out = lxgitignore::run::run(input, None, &config, client.as_ref())
        .expect("run() must succeed for eval test");

    assert!(!out.content.is_empty(), "content must not be empty");
    // A Rust gitignore should always include /target/
    assert!(
        out.content.contains("target"),
        "Rust .gitignore must mention target directory, got: {}",
        &out.content[..out.content.len().min(500)]
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_python_project_structure() {
    if std::env::var("LX_API_KEY").is_err() {
        return;
    }
    use lx_config::Config;

    let config = Config::load().expect("Config must load for eval tests");
    let client = lx_llm::client_from_config(&config, false).expect("LLM client must build");
    let input = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/python_project.txt"),
    )
    .expect("fixture must exist");

    let out = lxgitignore::run::run(&input, None, &config, client.as_ref())
        .expect("run() must succeed for eval test");

    assert!(!out.content.is_empty(), "content must not be empty");
    // A Python gitignore should mention __pycache__ or *.pyc
    assert!(
        out.content.contains("__pycache__") || out.content.contains(".pyc"),
        "Python .gitignore must mention Python bytecode, got: {}",
        &out.content[..out.content.len().min(500)]
    );
}
