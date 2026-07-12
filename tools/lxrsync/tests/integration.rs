use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxrsync::run::run;

fn mock_safe() -> &'static str {
    r#"{"command":"rsync -avz /home/user/docs/ backup@remote:/backup/docs/","dangerous":false}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"command":"rsync -avz --delete /var/www/ web@server:/var/www/","dangerous":true}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("copy local folder to remote server", &config, &client).unwrap();
    assert!(!out.command.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_flag_from_model_preserved() {
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    let (out, _findings) = run(
        "sync and delete remote files not in source",
        &config,
        &client,
    )
    .unwrap();
    assert!(out.dangerous);
}

#[test]
fn dangerous_pattern_detected_locally() {
    // Even if model sets dangerous:false, local check must override it.
    let client = MockLlmClient::returning(
        r#"{"command":"rsync -avz --delete /src/ /dst/","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run("sync and delete files at destination", &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local danger detection must override model's dangerous:false"
    );
}

#[test]
fn safe_command_not_flagged_dangerous() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("copy local folder to remote server", &config, &client).unwrap();
    assert!(!out.dangerous);
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("   \n\t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn pipe_to_shell_is_dangerous() {
    let client =
        MockLlmClient::returning(r#"{"command":"rsync -av /src/ /dst/ | sh","dangerous":false}"#);
    let config = Config::default();
    let (out, _findings) = run("sync and pipe to shell", &config, &client).unwrap();
    assert!(out.dangerous, "pipe to shell must be flagged dangerous");
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("copy local folder to remote server", &config, &client).unwrap();
    insta::assert_snapshot!(out.command.clone());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("copy local folder to remote server", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
