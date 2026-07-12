use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxkill::run::run;

fn mock_safe() -> &'static str {
    r#"{"command":"kill $(lsof -ti:3000)","target":"process on port 3000","reason":"lsof returns the PID then kill sends SIGTERM"}"#
}

fn mock_safe_windows() -> &'static str {
    r#"{"command":"Stop-Process -Id (Get-NetTCPConnection -LocalPort 3000).OwningProcess","target":"process on port 3000","reason":"Get-NetTCPConnection finds the PID"}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"command":"kill -9 1","target":"init/PID 1","reason":"kills the init process"}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _dangerous) = run("process on port 3000", "", "linux", &config, &client).unwrap();
    assert!(!out.command.is_empty());
    assert!(!out.target.is_empty());
    assert!(!out.reason.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_command_is_flagged() {
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    let (out, _dangerous) = run("kill init", "", "linux", &config, &client).unwrap();
    assert!(out.dangerous, "kill -9 1 must be detected as dangerous");
}

#[test]
fn safe_command_is_not_flagged() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _dangerous) = run("process on port 3000", "", "linux", &config, &client).unwrap();
    assert!(!out.dangerous, "port-kill command should not be flagged");
}

#[test]
fn context_is_included_in_request() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let ps_output = "USER       PID %CPU %MEM COMMAND\nroot      1234  0.1  0.0 node";
    let _out = run("the node server", ps_output, "linux", &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Process list context"),
        "context should appear in user message"
    );
}

#[test]
fn empty_context_omitted() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let _out = run("process on port 3000", "", "linux", &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        !req.user.contains("Process list context"),
        "empty context must not appear in user message"
    );
}

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("", "", "linux", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn target_linux_system_prompt_contains_linux() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let _ = run("process on port 3000", "", "linux", &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("linux"),
        "system prompt must contain 'linux'"
    );
    assert!(
        !req.system.contains("{os}"),
        "{{os}} placeholder must be filled"
    );
}

#[test]
fn target_windows_system_prompt_contains_windows() {
    let client = MockLlmClient::returning(mock_safe_windows());
    let config = Config::default();
    let _ = run("process on port 3000", "", "windows", &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("windows"),
        "system prompt must contain 'windows'"
    );
}

#[test]
fn target_macos_system_prompt_contains_macos() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let _ = run("process on port 3000", "", "macos", &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("macos"),
        "system prompt must contain 'macos'"
    );
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _dangerous) = run("process on port 3000", "", "linux", &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _dangerous) = run("process on port 3000", "", "linux", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
