// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must be named eval_* and carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_unknown_http_code_via_llm() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxerrno::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    // 418 is not in the local table → falls back to LLM
    let out = run("418", &config, client.as_ref()).unwrap();

    assert!(!out.code.is_empty(), "code must not be empty");
    assert!(!out.meaning.is_empty(), "meaning must not be empty");
    let lower = out.meaning.to_lowercase();
    assert!(
        lower.contains("teapot") || lower.contains("coffee") || lower.contains("418"),
        "meaning should reference the teapot joke: {}",
        out.meaning
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_well_known_code_resolves_locally_no_llm_cost() {
    use lx_config::Config;
    use lxerrno::run::run;

    // Use a no-op client to confirm no network call is made.
    struct AssertNotCalledClient;
    impl lx_llm::LlmClient for AssertNotCalledClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("LLM must not be called for well-known code 404");
        }
    }

    let config = Config::load().unwrap_or_else(|_| Config::default());
    let out = run("404", &config, &AssertNotCalledClient).unwrap();
    assert_eq!(out.code, "HTTP 404");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_errno_econnrefused_via_llm_or_local() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxerrno::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    // ECONNREFUSED is in the local table (111) — but test via name to ensure robustness
    let out = run("ECONNREFUSED", &config, client.as_ref()).unwrap();

    assert!(!out.meaning.is_empty());
    let lower = out.meaning.to_lowercase();
    assert!(
        lower.contains("connect") || lower.contains("refused") || lower.contains("listen"),
        "meaning should describe connection refused: {}",
        out.meaning
    );
}
