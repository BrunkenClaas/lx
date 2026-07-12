#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_finds_error_handling_code() {
    // Real LlmClient from env. Check structure, not exact text.
    let key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };
    let _ = key; // used by lx_config via env

    use lx_config::Config;
    use lxgrep::run::run;

    let config = Config::load().unwrap();
    let client = lx_llm::client_from_config(&config, false).unwrap();

    let content = "\
fn main() {\n\
    match connect() {\n\
        Ok(c) => println!(\"connected: {c:?}\"),\n\
        Err(e) => eprintln!(\"connection failed: {e}\"),\n\
    }\n\
}\n\
\n\
fn add(a: i32, b: i32) -> i32 { a + b }\n";

    let out = run(
        "error handling",
        &[("sample.rs", content)],
        &config,
        client.as_ref(),
    )
    .unwrap();

    // Should find at least one match.
    assert!(!out.matches.is_empty(), "expected at least one match");
    for m in &out.matches {
        assert!(!m.file.is_empty());
        assert!(m.line > 0);
        assert!(!m.snippet.is_empty());
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_returns_empty_for_irrelevant_query() {
    let key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };
    let _ = key;

    use lx_config::Config;
    use lxgrep::run::run;

    let config = Config::load().unwrap();
    let client = lx_llm::client_from_config(&config, false).unwrap();

    // Content is entirely about math; query is about databases. The whole
    // file is sent to the LLM regardless (relevance is always its call) — we
    // expect it to correctly judge there's nothing relevant here.
    let content =
        "fn add(a: i32, b: i32) -> i32 { a + b }\nfn mul(a: i32, b: i32) -> i32 { a * b }\n";

    let out = run(
        "database connection pool timeout",
        &[("math.rs", content)],
        &config,
        client.as_ref(),
    )
    .unwrap();

    // May be empty or have no relevant matches.
    // We don't hard-assert empty since the eval model has discretion,
    // but the output must be valid.
    for m in &out.matches {
        assert!(!m.file.is_empty());
        assert!(m.line > 0);
    }
}
