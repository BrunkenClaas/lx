use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxgitignore::run::{run, Output};

fn mock_response_rust() -> &'static str {
    "{\"content\": \"# Build artifacts\\n/target/\\n\\n# Editor files\\n.idea/\\n.vscode/\\n\"}"
}

fn mock_response_python() -> &'static str {
    "{\"content\": \"# Python bytecode\\n__pycache__/\\n*.pyc\\n\\n# Virtual environments\\nvenv/\\n.venv/\\n\"}"
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let out = run(
        "Cargo.toml\nCargo.lock\nsrc/\n  src/main.rs\ntarget/",
        None,
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.content.is_empty(), "content must not be empty");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let _ = run("Cargo.toml\nsrc/\n  src/main.rs", None, &config, &client);
    let req = client.last_request();
    assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
}

#[test]
fn system_prompt_not_empty() {
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let _ = run("Cargo.toml\nsrc/\n  src/main.rs", None, &config, &client);
    let req = client.last_request();
    assert!(!req.system.is_empty(), "system prompt must not be empty");
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let _ = run("Cargo.toml\nsrc/\n  src/main.rs", None, &config, &client);
    let req = client.last_request();
    assert!(
        req.max_tokens <= 2048,
        "lxgitignore max_tokens should be ≤ 2048, got {}",
        req.max_tokens
    );
}

#[test]
fn empty_input_returns_error() {
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let result = run("   ", None, &config, &client);
    assert!(result.is_err(), "empty input should return an error");
    assert_eq!(client.call_count(), 0, "no LLM call for empty input");
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let out = run(
        "Cargo.toml\nCargo.lock\nsrc/\n  src/main.rs\ntarget/",
        None,
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let out = run(
        "Cargo.toml\nCargo.lock\nsrc/\n  src/main.rs\ntarget/",
        None,
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn python_project_structure_accepted() {
    let client = MockLlmClient::returning(mock_response_python());
    let config = Config::default();
    let out = run(
        "setup.py\nrequirements.txt\nsrc/\n  src/app.py\nvenv/\n__pycache__/",
        None,
        &config,
        &client,
    )
    .unwrap();
    assert!(
        !out.content.is_empty(),
        "python project content must not be empty"
    );
}

#[test]
fn to_plain_returns_content_directly() {
    let out = Output {
        content: "# .gitignore\ntarget/\n*.log\n".to_string(),
    };
    assert_eq!(out.to_plain(), "# .gitignore\ntarget/\n*.log\n");
}

#[test]
fn edit_mode_user_message_contains_existing_content() {
    let existing = "# Build artifacts\n/target/\n\n# Editor\n.idea/\n";
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let _out = run("also ignore .env files", Some(existing), &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit the following .gitignore"),
        "edit mode must include edit instruction in user message"
    );
    assert!(
        req.user.contains("/target/"),
        "edit mode must include existing content"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn create_mode_user_message_is_plain_structure() {
    let client = MockLlmClient::returning(mock_response_rust());
    let config = Config::default();
    let _out = run("Cargo.toml\nsrc/", None, &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        !req.user.contains("Edit the following"),
        "create mode must not include edit instruction"
    );
}

#[test]
fn fsbound_rejects_path_traversal() {
    use std::path::Path;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Isolate in a per-invocation temp dir. A fixed name in the shared system
    // temp dir races with parallel test runs: if another process removes the
    // file or root between setup and the read_file() call, canonicalize() fails
    // with BadUsage instead of the SecurityAbort this test asserts.
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let unique = format!(
        "lxgitignore_fsbound_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let base = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&base).unwrap();

    // The file lives OUTSIDE the allowed root, so reading it must be rejected.
    let file = base.join("outside.txt");
    std::fs::write(&file, b"Cargo.toml\nsrc/\n  src/main.rs\n").unwrap();

    let root = base.join("root");
    std::fs::create_dir_all(&root).unwrap();

    let result = lx_core::io::read_file(Path::new(&file), 1024, Some(Path::new(&root)));
    assert!(
        matches!(result, Err(lx_core::error::LxError::SecurityAbort(_))),
        "fsbound must reject path traversal, got: {result:?}"
    );

    std::fs::remove_dir_all(&base).ok();
}
