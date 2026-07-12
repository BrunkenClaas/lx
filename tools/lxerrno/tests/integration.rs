use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxerrno::run::run;

// ── Mock response for unknown codes that fall through to the LLM ───────────────

fn mock_llm_response() -> &'static str {
    r#"{"code":"HTTP 418","meaning":"I'm a Teapot — the server refuses to brew coffee.","hint":"This is an April Fools code; check API docs for the intended meaning."}"#
}

// ── Local resolution (no LLM call expected) ────────────────────────────────────

#[test]
fn http_404_resolves_locally() {
    // A client that panics if called — proves no LLM roundtrip happens.
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("LLM client must not be called for a well-known code");
        }
    }
    let config = Config::default();
    let out = run("404", &config, &PanicClient).unwrap();
    assert_eq!(out.code, "HTTP 404");
    assert!(
        out.meaning.to_lowercase().contains("not found"),
        "meaning: {}",
        out.meaning
    );
}

#[test]
fn http_200_resolves_locally() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("LLM client must not be called for HTTP 200");
        }
    }
    let config = Config::default();
    let out = run("200", &config, &PanicClient).unwrap();
    assert_eq!(out.code, "HTTP 200");
    assert!(
        out.meaning.to_lowercase().contains("ok") || out.meaning.to_lowercase().contains("success"),
        "meaning: {}",
        out.meaning
    );
}

#[test]
fn errno_enoent_by_name_resolves_locally() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("LLM must not be called for ENOENT");
        }
    }
    let config = Config::default();
    let out = run("ENOENT", &config, &PanicClient).unwrap();
    assert!(out.code.contains("ENOENT"), "code: {}", out.code);
    assert!(
        out.meaning.to_lowercase().contains("file")
            || out.meaning.to_lowercase().contains("directory"),
        "meaning: {}",
        out.meaning
    );
}

#[test]
fn errno_enoent_by_number_resolves_locally() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("LLM must not be called for errno 2");
        }
    }
    let config = Config::default();
    // errno 2 = ENOENT
    let out = run("errno 2", &config, &PanicClient).unwrap();
    assert!(
        out.code.contains("ENOENT") || out.code.contains("2"),
        "code should reference ENOENT or 2: {}",
        out.code
    );
}

#[test]
fn exit_130_resolves_locally() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("LLM must not be called for exit 130");
        }
    }
    let config = Config::default();
    let out = run("exit 130", &config, &PanicClient).unwrap();
    assert!(out.code.contains("130"), "code: {}", out.code);
    assert!(
        out.meaning.to_lowercase().contains("sigint")
            || out.meaning.to_lowercase().contains("ctrl"),
        "meaning: {}",
        out.meaning
    );
}

#[test]
fn exit_0_resolves_locally() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("LLM must not be called for exit 0");
        }
    }
    let config = Config::default();
    let out = run("exit 0", &config, &PanicClient).unwrap();
    assert!(out.code.contains("0"), "code: {}", out.code);
    assert!(
        out.meaning.to_lowercase().contains("success"),
        "meaning: {}",
        out.meaning
    );
}

#[test]
fn signal_exit_128_plus_n_resolves_locally() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("LLM must not be called for exit 141");
        }
    }
    let config = Config::default();
    // 141 = 128 + 13 (SIGPIPE)
    let out = run("exit 141", &config, &PanicClient).unwrap();
    assert!(out.code.contains("141"), "code: {}", out.code);
    assert!(
        out.meaning.to_lowercase().contains("signal"),
        "meaning should mention signal: {}",
        out.meaning
    );
}

// ── LLM fallback for unknown codes ─────────────────────────────────────────────

#[test]
fn unknown_code_falls_back_to_llm() {
    let client = MockLlmClient::returning(mock_llm_response());
    let config = Config::default();
    // 418 is not in our local table
    let out = run("418", &config, &client).unwrap();
    assert!(!out.code.is_empty(), "code must not be empty");
    assert!(!out.meaning.is_empty(), "meaning must not be empty");
    // Verify the LLM was actually called
    assert_eq!(
        client.call_count(),
        1,
        "exactly one LLM call for unknown code"
    );
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn llm_request_has_correct_invariants() {
    let client = MockLlmClient::returning(mock_llm_response());
    let config = Config::default();
    let _ = run("418", &config, &client).unwrap();
    let req = client.last_request();
    assertions::assert_request_invariants(&req);
    assert!(
        req.max_tokens <= 256,
        "max_tokens should be ≤ 256, got {}",
        req.max_tokens
    );
    assert!(!req.system.is_empty(), "system prompt must not be empty");
}

// ── Edge cases ─────────────────────────────────────────────────────────────────

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_llm_response());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_llm_response());
    let config = Config::default();
    let err = run("   \t\n", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

// ── Output format ──────────────────────────────────────────────────────────────

#[test]
fn to_plain_includes_code_and_meaning() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("not expected");
        }
    }
    let config = Config::default();
    let out = run("404", &config, &PanicClient).unwrap();
    let plain = out.to_plain();
    assert!(
        plain.contains("404"),
        "plain output should contain the code"
    );
    assert!(!plain.trim().is_empty(), "plain output must not be empty");
}

#[test]
fn to_plain_includes_hint_when_present() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("not expected");
        }
    }
    let config = Config::default();
    // 404 has a hint in the local table
    let out = run("404", &config, &PanicClient).unwrap();
    if !out.hint.is_empty() {
        let plain = out.to_plain();
        assert!(
            plain.contains("hint:"),
            "plain output should show hint label"
        );
    }
}

// ── Snapshots ──────────────────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output_404() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("not expected");
        }
    }
    let config = Config::default();
    let out = run("404", &config, &PanicClient).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output_404() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("not expected");
        }
    }
    let config = Config::default();
    let out = run("404", &config, &PanicClient).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn snapshot_plain_output_enoent() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("not expected");
        }
    }
    let config = Config::default();
    let out = run("ENOENT", &config, &PanicClient).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_plain_output_exit_130() {
    struct PanicClient;
    impl lx_llm::LlmClient for PanicClient {
        fn complete(
            &self,
            _req: &lx_llm::Request<'_>,
        ) -> Result<lx_llm::Response, lx_llm::LlmError> {
            panic!("not expected");
        }
    }
    let config = Config::default();
    let out = run("exit 130", &config, &PanicClient).unwrap();
    insta::assert_snapshot!(out.to_plain());
}
