use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxsql::run::run;

// ── helpers ──────────────────────────────────────────────────────────────────

fn mock_select() -> &'static str {
    r#"{"sql":"SELECT id, name, email FROM users ORDER BY name","mutating":false}"#
}

fn mock_delete() -> &'static str {
    r#"{"sql":"DELETE FROM sessions WHERE created_at < NOW() - INTERVAL '7 days'","mutating":true}"#
}

fn mock_update_lying() -> &'static str {
    // Model returns mutating:false but SQL contains UPDATE — local check must override.
    r#"{"sql":"UPDATE users SET active = false WHERE last_login < '2020-01-01'","mutating":false}"#
}

fn mock_drop_lying() -> &'static str {
    // Model returns mutating:false but SQL contains DROP TABLE — local check must override.
    r#"{"sql":"DROP TABLE old_logs","mutating":false}"#
}

// ── schema validation ─────────────────────────────────────────────────────────

#[test]
fn output_schema_is_valid_select() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let (out, _warning) = run("get all users", None, None, &config, &client).unwrap();
    assert!(!out.sql.is_empty(), "sql must not be empty");
    assert!(!out.mutating, "SELECT should not be mutating");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn output_schema_is_valid_delete() {
    let client = MockLlmClient::returning(mock_delete());
    let config = Config::default();
    let (out, _warning) = run("delete old sessions", None, None, &config, &client).unwrap();
    assert!(!out.sql.is_empty(), "sql must not be empty");
    assert!(out.mutating, "DELETE should be mutating");
}

// ── local mutating detection ──────────────────────────────────────────────────

#[test]
fn update_detected_locally_even_if_llm_says_false() {
    let client = MockLlmClient::returning(mock_update_lying());
    let config = Config::default();
    let (out, _warning) = run("deactivate old users", None, None, &config, &client).unwrap();
    assert!(
        out.mutating,
        "local detection must override model's mutating:false for UPDATE"
    );
}

#[test]
fn drop_detected_locally_even_if_llm_says_false() {
    let client = MockLlmClient::returning(mock_drop_lying());
    let config = Config::default();
    let (out, _warning) = run("remove the old_logs table", None, None, &config, &client).unwrap();
    assert!(
        out.mutating,
        "local detection must override model's mutating:false for DROP TABLE"
    );
}

#[test]
fn insert_is_mutating() {
    let client = MockLlmClient::returning(
        r#"{"sql":"INSERT INTO events (name, ts) VALUES ('login', NOW())","mutating":false}"#,
    );
    let config = Config::default();
    let (out, _warning) = run("log a login event", None, None, &config, &client).unwrap();
    assert!(out.mutating, "INSERT must be detected as mutating locally");
}

#[test]
fn truncate_is_mutating() {
    let client = MockLlmClient::returning(r#"{"sql":"TRUNCATE TABLE temp_data","mutating":false}"#);
    let config = Config::default();
    let (out, _warning) = run("clear the temp_data table", None, None, &config, &client).unwrap();
    assert!(
        out.mutating,
        "TRUNCATE must be detected as mutating locally"
    );
}

#[test]
fn select_is_not_mutating() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let (out, _warning) = run("list all users", None, None, &config, &client).unwrap();
    assert!(!out.mutating, "SELECT must not be marked mutating");
}

// ── schema hint ───────────────────────────────────────────────────────────────

#[test]
fn schema_hint_is_accepted() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let schema = "CREATE TABLE users (id INT PRIMARY KEY, name TEXT, email TEXT);";
    let (out, _warning) = run("get all users", Some(schema), None, &config, &client).unwrap();
    assert!(!out.sql.is_empty());
    // Verify the schema was included in the user message sent to the LLM.
    let req = client.last_request();
    assert!(
        req.user.contains("Schema"),
        "user message should include schema context"
    );
}

// ── edge cases ────────────────────────────────────────────────────────────────

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let err = run("", None, None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let err = run("   \t\n", None, None, &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

// ── request invariants ────────────────────────────────────────────────────────

#[test]
fn request_invariants_are_satisfied() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    run("count active users", None, None, &config, &client).unwrap();
    assertions::assert_request_invariants(&client.last_request());
}

// ── output helpers ────────────────────────────────────────────────────────────

#[test]
fn to_plain_returns_sql() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let (out, _warning) = run("get all users", None, None, &config, &client).unwrap();
    assert_eq!(out.to_plain(), out.sql);
}

// ── edit mode ─────────────────────────────────────────────────────────────────

#[test]
fn edit_mode_user_message_contains_existing_sql() {
    let existing = "SELECT id, name, email FROM users ORDER BY name";
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let _out = run(
        "add a WHERE active = true filter",
        None,
        Some(existing),
        &config,
        &client,
    )
    .unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit the following SQL"),
        "edit mode must include edit instruction, got: {}",
        req.user
    );
    assert!(
        req.user.contains(existing),
        "edit mode must include existing SQL in user message"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn create_mode_user_message_is_plain_description() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let _out = run("list all active users", None, None, &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        !req.user.contains("Edit the following SQL"),
        "create mode must NOT include edit instruction"
    );
}

// ── snapshots ────────────────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let (out, _warning) = run("get all users", None, None, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_select());
    let config = Config::default();
    let (out, _warning) = run("get all users", None, None, &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
