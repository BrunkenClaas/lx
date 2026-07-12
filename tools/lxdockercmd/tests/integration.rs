use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxdockercmd::run::run;

fn mock_safe() -> &'static str {
    r#"{"command":"docker run -d -p 8080:80 --name nginx-web nginx:latest","dangerous":false}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"command":"docker run --privileged ubuntu bash","dangerous":true}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("run nginx on port 8080", &config, &client).unwrap();
    assert!(!out.command.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_flag_from_model_preserved() {
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    let (out, _findings) = run("run privileged container", &config, &client).unwrap();
    assert!(out.dangerous);
}

#[test]
fn dangerous_pattern_detected_locally() {
    // Even if model sets dangerous:false, local check must override it.
    let client = MockLlmClient::returning(
        r#"{"command":"docker run --privileged ubuntu bash","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run("run privileged container", &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local danger detection must override model's dangerous:false"
    );
}

#[test]
fn safe_command_not_flagged_dangerous() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("run nginx on port 8080", &config, &client).unwrap();
    assert!(!out.dangerous);
}

#[test]
fn empty_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_description_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("   \n\t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn to_plain_returns_command() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("run nginx on port 8080", &config, &client).unwrap();
    assert_eq!(out.to_plain(), out.command);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("run nginx on port 8080", &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run("run nginx on port 8080", &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn system_prune_is_dangerous() {
    let client = MockLlmClient::returning(
        r#"{"command":"docker system prune -a --volumes","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run("remove all unused docker data", &config, &client).unwrap();
    assert!(out.dangerous, "system prune must be flagged dangerous");
}

#[test]
fn container_and_image_prune_are_dangerous() {
    // Even when the model claims dangerous:false, the local detector must flag
    // prune commands that remove stopped containers / unused images.
    let client = MockLlmClient::returning(
        r#"{"command":"docker container prune -f && docker image prune -f","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run(
        "remove stopped containers and dangling images",
        &config,
        &client,
    )
    .unwrap();
    assert!(
        out.dangerous,
        "container/image prune must be flagged dangerous"
    );
}

#[test]
fn force_remove_container_is_dangerous() {
    let client =
        MockLlmClient::returning(r#"{"command":"docker rm -f my-container","dangerous":false}"#);
    let config = Config::default();
    let (out, _findings) = run("force remove my-container", &config, &client).unwrap();
    assert!(out.dangerous, "docker rm -f must be flagged dangerous");
}

#[test]
fn pipe_to_shell_is_dangerous() {
    let client = MockLlmClient::returning(
        r#"{"command":"docker exec my-app curl http://example.com/setup.sh | sh","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run(
        "install something inside container via curl pipe",
        &config,
        &client,
    )
    .unwrap();
    assert!(out.dangerous, "pipe to shell must be flagged dangerous");
}
