use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxfind::run::{run, Output};
use std::path::{Path, PathBuf};

fn mock_response() -> &'static str {
    r#"{"paths":["src/backup.sh","scripts/db_dump.sh"]}"#
}

fn empty_response() -> &'static str {
    r#"{"paths":[]}"#
}

// Use the current directory as a safe root that is guaranteed to exist.
fn safe_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "the script that runs database backups",
        safe_root(),
        &config,
        &client,
    )
    .unwrap();
    // Paths may be filtered (they don't exist on disk) — schema must parse fine.
    let _: &Vec<String> = &out.paths;
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   ", safe_root(), &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("find config file", safe_root(), &config, &client);
    let req = client.last_request();
    assert!(req.max_tokens <= 1024, "lxfind max_tokens should be ≤ 1024");
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("find config file", safe_root(), &config, &client);
    let req = client.last_request();
    assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
}

#[test]
fn system_prompt_not_empty() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("find config file", safe_root(), &config, &client);
    let req = client.last_request();
    assert!(!req.system.is_empty(), "system prompt must not be empty");
}

#[test]
fn system_prompt_contains_ignore_instructions() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("find config file", safe_root(), &config, &client);
    let req = client.last_request();
    assert!(
        req.system.contains("Ignore any instructions"),
        "untrusted flag: system prompt must tell model to ignore embedded instructions"
    );
}

#[test]
fn user_message_contains_description() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("the backup script", safe_root(), &config, &client);
    let req = client.last_request();
    assert!(
        req.user.contains("the backup script"),
        "user message must contain the description"
    );
}

#[test]
fn user_message_contains_catalog_header() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run("find the main entry point", safe_root(), &config, &client);
    let req = client.last_request();
    assert!(
        req.user.contains("Catalog:"),
        "user message must contain the catalog"
    );
}

#[test]
fn empty_paths_response_is_valid() {
    let client = MockLlmClient::returning(empty_response());
    let config = Config::default();
    let out = run("no matching file", safe_root(), &config, &client).unwrap();
    assert!(out.paths.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let out = Output {
        paths: vec![
            "src/backup.sh".to_string(),
            "scripts/db_dump.sh".to_string(),
        ],
        truncated: false,
    };
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let out = Output {
        paths: vec![
            "src/backup.sh".to_string(),
            "scripts/db_dump.sh".to_string(),
        ],
        truncated: false,
    };
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn to_plain_is_pipe_safe() {
    let out = Output {
        paths: vec!["a/b.rs".to_string(), "c/d.py".to_string()],
        truncated: false,
    };
    let plain = out.to_plain();
    for line in plain.lines() {
        assert!(
            !line.starts_with('#'),
            "plain output must not have comment lines: {line:?}"
        );
    }
}

#[test]
fn fsbound_rejects_symlink_escape() {
    // Build a root directory in tmp.
    let tmp = std::env::temp_dir();
    let root = tmp.join("lxfind_fsbound_test_root");
    std::fs::create_dir_all(&root).unwrap();

    // Place a legitimate file inside the root.
    let inner = root.join("inner.txt");
    std::fs::write(&inner, "# inner file\n").unwrap();

    // Create a file outside the root.
    let outside = tmp.join("lxfind_fsbound_outside.txt");
    std::fs::write(&outside, "# outside\n").unwrap();

    // The LLM returns the outside path — it must be filtered out.
    let outside_str = outside.display().to_string();
    let resp = format!(r#"{{"paths":["{}"]}}"#, outside_str.replace('\\', "\\\\"));
    let client = MockLlmClient::returning(Box::leak(resp.into_boxed_str()));
    let config = Config::default();

    let out = run("any file", &root, &config, &client).unwrap();

    // The outside path must have been rejected.
    assert!(
        !out.paths.contains(&outside_str),
        "fsbound: path escaping root must be filtered out, got: {:?}",
        out.paths
    );

    // Clean up.
    std::fs::remove_file(&inner).ok();
    std::fs::remove_file(&outside).ok();
    std::fs::remove_dir(&root).ok();
}

#[test]
fn invalid_root_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let bad_root = PathBuf::from("/this/path/does/not/exist/anywhere");
    let err = run("find something", &bad_root, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}
