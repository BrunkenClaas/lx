use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxcron::run::{detect_mode, run, Mode};

fn mock_generate_response() -> &'static str {
    r#"{"crontab":"0 9 * * 1-5 /usr/local/bin/backup.sh","explanation":"Runs at 9am Mon-Fri","dangerous":false}"#
}
fn mock_explain_response() -> &'static str {
    r#"{"crontab":"0 2 * * 0 /home/user/cleanup.sh","explanation":"Runs cleanup.sh at 2am every Sunday","dangerous":false}"#
}

#[test]
fn generate_output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_generate_response());
    let out = run(
        "every weekday at 9am",
        Mode::Generate,
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.crontab.is_empty());
    assert!(!out.explanation.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn explain_output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_explain_response());
    let out = run(
        "0 2 * * 0 /home/user/cleanup.sh",
        Mode::Explain,
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.explanation.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_generate_response());
    let err = run("   ", Mode::Generate, &Config::default(), &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_generate_response());
    let _ = run("every hour", Mode::Generate, &Config::default(), &client);
    assert!(client.last_request().max_tokens <= 256);
}

#[test]
fn detect_mode_identifies_cron_line() {
    assert_eq!(
        detect_mode("0 2 * * 0 /home/user/cleanup.sh"),
        Mode::Explain
    );
    assert_eq!(detect_mode("*/15 * * * * echo hi"), Mode::Explain);
    assert_eq!(detect_mode("every Sunday at 2am"), Mode::Generate);
    assert_eq!(detect_mode("run backup weekly"), Mode::Generate);
}

#[test]
fn dangerous_cron_command_flagged() {
    let resp =
        r#"{"crontab":"0 * * * * rm -rf /","explanation":"deletes everything","dangerous":false}"#;
    let client = MockLlmClient::returning(resp);
    let out = run("every hour", Mode::Generate, &Config::default(), &client).unwrap();
    assert!(out.dangerous);
}

#[test]
fn snapshot_plain_generate() {
    let client = MockLlmClient::returning(mock_generate_response());
    let out = run(
        "every weekday at 9am",
        Mode::Generate,
        &Config::default(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain(Mode::Generate));
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_generate_response());
    let out = run(
        "every weekday at 9am",
        Mode::Generate,
        &Config::default(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn edit_mode_user_message_contains_change_and_existing() {
    let existing_crontab = "0 2 * * 0 /home/user/cleanup.sh";
    let combined = format!("change to run daily\n\n{}", existing_crontab);
    let client = MockLlmClient::returning(mock_generate_response());
    let _out = run(&combined, Mode::Edit, &Config::default(), &client).unwrap();
    let req = client.last_request();
    assert!(
        req.user.contains("Edit this crontab line"),
        "edit mode must include edit instruction in user message"
    );
    assert!(
        req.user.contains(existing_crontab),
        "edit mode must include existing crontab in user message"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn edit_mode_result_is_crontab_field() {
    let existing_crontab = "0 2 * * 0 /home/user/cleanup.sh";
    let combined = format!("change to run daily\n\n{}", existing_crontab);
    let client = MockLlmClient::returning(mock_generate_response());
    let out = run(&combined, Mode::Edit, &Config::default(), &client).unwrap();
    assert_eq!(out.to_plain(Mode::Edit), out.crontab);
}
