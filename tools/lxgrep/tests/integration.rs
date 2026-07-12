use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxgrep::run::{run, Output};

fn mock_response() -> &'static str {
    r#"{"matches":[{"file":"src/main.rs","line":14,"snippet":"    Err(e) => eprintln!(\"error: {e}\"),"}]}"#
}

fn mock_empty_response() -> &'static str {
    r#"{"matches":[]}"#
}

const SAMPLE_CONTENT: &str = "\
fn main() {\n\
    match do_thing() {\n\
        Ok(v) => println!(\"{v}\"),\n\
        Err(e) => eprintln!(\"error: {e}\"),\n\
    }\n\
}\n\
\n\
fn add(a: i32, b: i32) -> i32 { a + b }\n";

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "error handling",
        &[("src/main.rs", SAMPLE_CONTENT)],
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.matches.is_empty(), "matches must not be empty");
    let m = &out.matches[0];
    assert!(!m.file.is_empty(), "match.file must not be empty");
    assert!(m.line > 0, "match.line must be > 0");
    assert!(!m.snippet.is_empty(), "match.snippet must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn llm_is_always_called_even_without_literal_keyword_overlap() {
    // Regression test: lxgrep previously short-circuited to an empty result
    // WITHOUT calling the LLM whenever the query's keywords had no literal
    // substring match in the content. That defeats semantic search — relevance
    // must always be the model's decision, never a local keyword gate.
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(
        "database connection pool",
        &[("math.rs", "fn add(a: i32, b: i32) -> i32 { a + b }")],
        &config,
        &client,
    )
    .unwrap();
    assert_eq!(
        client.call_count(),
        1,
        "LLM must be called even when the query has no literal keyword overlap"
    );
}

#[test]
fn empty_query_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("  ", &[("f.rs", "fn main() {}")], &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn empty_content_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("error handling", &[], &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "error handling",
        &[("src/main.rs", SAMPLE_CONTENT)],
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "error handling",
        &[("src/main.rs", SAMPLE_CONTENT)],
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn fsbound_rejects_path_traversal() {
    // This test validates the fsbound logic in lx_core::io::read_file.
    // Attempt to read a file that escapes the specified root.
    let tmp = std::env::temp_dir();
    let test_file = tmp.join("lxgrep_fsbound_test.txt");
    std::fs::write(&test_file, b"secret content").unwrap();

    // Root is a subdirectory of tmp — the file is *outside* this root.
    let root = tmp.join("lxgrep_fsbound_root");
    std::fs::create_dir_all(&root).unwrap();

    let result = lx_core::io::read_file(&test_file, 1024, Some(&root));
    assert!(
        matches!(result, Err(lx_core::error::LxError::SecurityAbort(_))),
        "expected SecurityAbort for path traversal, got: {result:?}"
    );

    std::fs::remove_file(&test_file).ok();
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn empty_llm_matches_returns_empty_output() {
    let client = MockLlmClient::returning(mock_empty_response());
    let config = Config::default();
    let out = run(
        "error",
        &[("main.rs", "fn main() { let err = 42; }")],
        &config,
        &client,
    )
    .unwrap();
    assert!(out.matches.is_empty());
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(
        "error handling",
        &[("src/main.rs", SAMPLE_CONTENT)],
        &config,
        &client,
    );
    if client.call_count() > 0 {
        let req = client.last_request();
        assert!(req.max_tokens <= 2048, "lxgrep max_tokens must be ≤ 2048");
    }
}

#[test]
fn to_plain_is_grep_compatible() {
    let out = Output {
        matches: vec![lxgrep::run::Match {
            file: "src/main.rs".to_string(),
            line: 42,
            snippet: "    some_code();".to_string(),
        }],
        capped: false,
    };
    let plain = out.to_plain();
    // grep-compatible format: file:line: snippet
    assert!(plain.starts_with("src/main.rs:42:"));
    // No lines starting with '#' (pipe safety).
    for line in plain.lines() {
        assert!(!line.starts_with('#'), "comment on stdout: {line:?}");
    }
}
