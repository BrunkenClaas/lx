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

/// A `find .`-style path list: many short, self-contained records.
fn path_list(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        // A handful of genuinely build-related entries scattered late in the
        // list, so they only survive if sampling covers the whole input.
        if i == 700 {
            s.push_str("./crates/lx-core/build.rs\n");
        } else if i == 900 {
            s.push_str("./scripts/build-release-zip.sh\n");
        } else {
            s.push_str(&format!("./target/debug/.fingerprint/dep-lib-crate-{i}\n"));
        }
    }
    s
}

#[test]
fn line_oriented_input_gets_far_more_candidates_than_block_budget() {
    // Regression test for the `find . | lxgrep` failure: on a path list every
    // line is a whole record, so the context-block strategy spent ~5 lines of
    // budget per candidate and only ~40 records ever reached the model. The
    // line-oriented path must put an order of magnitude more records in front
    // of it, from the same input.
    let client = MockLlmClient::returning(mock_empty_response());
    let config = Config::default();
    let content = path_list(2000);
    let _ = run(
        "build related stuff",
        &[("<stdin>", &content)],
        &config,
        &client,
    )
    .unwrap();

    let req = client.last_request();
    let candidate_lines = req.user.lines().filter(|l| l.starts_with("./")).count();
    assert!(
        candidate_lines > 200,
        "line-oriented input must send far more than the 40-block budget, got {candidate_lines}"
    );
}

#[test]
fn line_oriented_sampling_reaches_records_late_in_the_list() {
    // The old block budget was filled by the first ~40 blocks, so a relevant
    // record near the end of a long list could never reach the model regardless
    // of the query. Even coverage must make late records visible.
    let client = MockLlmClient::returning(mock_empty_response());
    let config = Config::default();
    let content = path_list(2000);
    let _ = run(
        "build related stuff",
        &[("<stdin>", &content)],
        &config,
        &client,
    )
    .unwrap();

    let req = client.last_request();
    assert!(
        req.user.contains("build.rs") || req.user.contains("build-release-zip.sh"),
        "records late in the list must be reachable by sampling"
    );
}

#[test]
fn source_code_still_uses_context_blocks() {
    // Guard against the heuristic firing on real content. Source code needs its
    // surrounding lines to be interpretable, so a long code file must keep the
    // block strategy — the model should see more than the single hit line.
    let client = MockLlmClient::returning(mock_empty_response());
    let config = Config::default();
    let mut code = String::new();
    for i in 0..200 {
        code.push_str(&format!("fn function_number_{i}() {{\n"));
        code.push_str("    let value = compute();\n");
        code.push_str("}\n");
        code.push('\n');
    }
    let _ = run("compute", &[("big.rs", &code)], &config, &client).unwrap();

    let req = client.last_request();
    // Context blocks keep the `fn ...` line adjacent to its body; a
    // line-oriented split would have sent bare `let value = compute();` lines.
    assert!(
        req.user.contains("fn function_number_"),
        "source code must keep context blocks, not be split into bare lines"
    );
}

#[test]
fn prose_with_blank_lines_is_not_line_oriented() {
    // Paragraph-separated prose must not be treated as a record list even when
    // its lines are short.
    let client = MockLlmClient::returning(mock_empty_response());
    let config = Config::default();
    let mut prose = String::new();
    for i in 0..100 {
        prose.push_str(&format!("Short sentence number {i}.\n"));
        prose.push('\n');
    }
    let _ = run("sentence", &[("notes.md", &prose)], &config, &client).unwrap();
    // Reaching the LLM at all is the contract; the assertion that matters is
    // that this did not panic or short-circuit on the blank-line path.
    assert_eq!(client.call_count(), 1);
}

#[test]
fn short_line_oriented_input_is_unaffected() {
    // Below the minimum line count everything fits in the block budget anyway,
    // so behaviour must be unchanged.
    let client = MockLlmClient::returning(mock_empty_response());
    let config = Config::default();
    let content = "./a.rs\n./b.rs\n./c.rs\n";
    let _ = run("rust files", &[("<stdin>", content)], &config, &client).unwrap();
    assert_eq!(client.call_count(), 1);
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
        input_truncated: false,
    };
    let plain = out.to_plain();
    // grep-compatible format: file:line: snippet
    assert!(plain.starts_with("src/main.rs:42:"));
    // No lines starting with '#' (pipe safety).
    for line in plain.lines() {
        assert!(!line.starts_with('#'), "comment on stdout: {line:?}");
    }
}

#[test]
fn prompt_size_is_bounded_by_the_candidate_budget_not_the_input_size() {
    // The merge criterion for raising `limits.max_input_bytes`: what is SENT is
    // fixed by the candidate budget, so a bigger read budget costs memory and
    // scan time but not tokens. If this number tracks the input size, the
    // sampler has stopped bounding the prompt and the raise becomes a cost
    // regression.
    let small: String = (0..2_000).map(|i| format!("./src/mod{i}.rs\n")).collect();
    let large: String = (0..40_000).map(|i| format!("./src/mod{i}.rs\n")).collect();

    let p_small = lxgrep::run::preview_user_message("build related stuff", &[("f", &small)]);
    let p_large = lxgrep::run::preview_user_message("build related stuff", &[("f", &large)]);

    assert!(
        p_large.len() < 3 * p_small.len(),
        "prompt grew with input: {} bytes for 2k lines vs {} bytes for 40k",
        p_small.len(),
        p_large.len()
    );
    assert!(
        p_large.len() < 200_000,
        "prompt must stay far below the input size; was {} bytes",
        p_large.len()
    );
}

#[test]
fn capped_is_false_on_small_input_and_true_on_genuinely_capped_input() {
    // Guards both directions of the flag that drives the "results are
    // INCOMPLETE" warning: it must not cry wolf on a small input, and must
    // still fire when the budget really does drop candidates.
    let client = MockLlmClient::returning(r#"{"matches":[]}"#);
    let cfg = Config::default();

    let small: String = (0..134)
        .map(|i| format!("-rw-r--r-- 1 u g {i} Jul 12 f{i}.rs\n"))
        .collect();
    let out = lxgrep::run::run(
        "where do we handle retries?",
        &[("<stdin>", &small)],
        &cfg,
        &client,
    )
    .unwrap();
    assert!(
        !out.capped,
        "134 lines against a 40-block budget is not capped"
    );

    let big: String = (0..20_000).map(|i| format!("./src/mod{i}.rs\n")).collect();
    let out = lxgrep::run::run("build related stuff", &[("<stdin>", &big)], &cfg, &client).unwrap();
    assert!(out.capped, "20k lines against a 400-line budget IS capped");
}
