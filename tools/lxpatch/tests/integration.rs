use lx_config::Config;
use lx_testkit::mock::MockLlmClient;
use lxpatch::run::run;

#[test]
fn output_schema_is_valid() {
    let mock = r#"{"diff":"--- a/file\n+++ b/file\n@@ -1 +1 @@\n-x = 1\n+y = 1","summary":"Renamed x to y."}"#;
    let client = MockLlmClient::returning(mock);
    let (out, _warnings) = run("x = 1\n", "rename x to y", &Config::default(), &client).unwrap();
    assert!(!out.diff.is_empty());
    assert!(!out.dangerous);
    lx_testkit::assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_diff_is_flagged() {
    let mock = r#"{"diff":"--- a/file\n+++ b/file\n@@ -1 +1 @@\n+rm -rf /","summary":"Added destructive command."}"#;
    let client = MockLlmClient::returning(mock);
    let (out, _warnings) = run("x = 1\n", "add cleanup", &Config::default(), &client).unwrap();
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
    let (out, _warnings) = run("x = 1\n", "rename x to y", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let mock = r#"{"diff":"--- a/file\n+++ b/file\n@@ -1 +1 @@\n-x = 1\n+y = 1","summary":"Renamed x to y."}"#;
    let client = MockLlmClient::returning(mock);
    let (out, _warnings) = run("x = 1\n", "rename x to y", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn oversized_input_is_capped_before_the_llm_call() {
    let mock = r#"{"diff":"--- a\n+++ b\n","summary":"renamed","dangerous":false}"#;
    let client = MockLlmClient::returning(mock);
    let mut huge = String::new();
    for i in 0..20_000 {
        huge.push_str(&format!("let v{i} = {i};\n"));
    }
    let (_out, warnings) = run(&huge, "rename things", &Config::default(), &client).unwrap();
    assert!(warnings.iter().any(|w| w.contains("truncated")));
    // The user message embeds the description too, so allow a small margin.
    assert!(client.last_request().user.len() <= 48_000 + 1_000);
}
