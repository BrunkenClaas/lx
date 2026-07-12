use lx_config::Config;
use lx_testkit::mock::MockLlmClient;
use lxpatch::run::run;

#[test]
fn output_schema_is_valid() {
    let mock = r#"{"diff":"--- a/file\n+++ b/file\n@@ -1 +1 @@\n-x = 1\n+y = 1","summary":"Renamed x to y."}"#;
    let client = MockLlmClient::returning(mock);
    let out = run("x = 1\n", "rename x to y", &Config::default(), &client).unwrap();
    assert!(!out.diff.is_empty());
    assert!(!out.dangerous);
    lx_testkit::assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_diff_is_flagged() {
    let mock = r#"{"diff":"--- a/file\n+++ b/file\n@@ -1 +1 @@\n+rm -rf /","summary":"Added destructive command."}"#;
    let client = MockLlmClient::returning(mock);
    let out = run("x = 1\n", "add cleanup", &Config::default(), &client).unwrap();
    assert!(out.dangerous);
}

#[test]
fn empty_description_returns_error() {
    let client = MockLlmClient::returning("{}");
    let err = run("x = 1\n", "", &Config::default(), &client).unwrap_err();
    assert!(matches!(err, lx_core::error::LxError::BadUsage(_)));
}

#[test]
fn empty_file_returns_error() {
    let client = MockLlmClient::returning("{}");
    let err = run("", "rename x to y", &Config::default(), &client).unwrap_err();
    assert!(matches!(err, lx_core::error::LxError::BadUsage(_)));
}

#[test]
fn snapshot_plain_output() {
    let mock = r#"{"diff":"--- a/file\n+++ b/file\n@@ -1 +1 @@\n-x = 1\n+y = 1","summary":"Renamed x to y."}"#;
    let client = MockLlmClient::returning(mock);
    let out = run("x = 1\n", "rename x to y", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let mock = r#"{"diff":"--- a/file\n+++ b/file\n@@ -1 +1 @@\n-x = 1\n+y = 1","summary":"Renamed x to y."}"#;
    let client = MockLlmClient::returning(mock);
    let out = run("x = 1\n", "rename x to y", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
