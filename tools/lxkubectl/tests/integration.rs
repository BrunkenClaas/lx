use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxkubectl::run::run;

fn mock_safe() -> &'static str {
    r#"{"command":"kubectl get pods -n production -o wide","dangerous":false}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"command":"kubectl delete pods -l app=nginx -n staging","dangerous":true}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run(
        "list all pods in production with wide output",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.command.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_flag_from_model_preserved() {
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    let (out, _findings) = run("delete all nginx pods in staging", &config, &client).unwrap();
    assert!(out.dangerous);
}

#[test]
fn dangerous_pattern_detected_locally() {
    // Even if model sets dangerous:false, local check must override it.
    let client = MockLlmClient::returning(
        r#"{"command":"kubectl delete pods -l app=nginx -n staging","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run("delete nginx pods in staging", &config, &client).unwrap();
    assert!(
        out.dangerous,
        "local danger detection must override model's dangerous:false"
    );
}

#[test]
fn safe_command_not_flagged_dangerous() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run(
        "list all pods in production with wide output",
        &config,
        &client,
    )
    .unwrap();
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
    let (out, _findings) = run("list all pods in production", &config, &client).unwrap();
    assert_eq!(out.to_plain(), out.command);
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run(
        "list all pods in production with wide output",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _findings) = run(
        "list all pods in production with wide output",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn kubectl_delete_is_dangerous() {
    let client = MockLlmClient::returning(
        r#"{"command":"kubectl delete deployment api -n production","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) =
        run("remove the api deployment in production", &config, &client).unwrap();
    assert!(out.dangerous, "kubectl delete must be flagged dangerous");
}

#[test]
fn kubectl_drain_is_dangerous() {
    let client = MockLlmClient::returning(
        r#"{"command":"kubectl drain worker-1 --ignore-daemonsets","dangerous":false}"#,
    );
    let config = Config::default();
    let (out, _findings) = run("drain node worker-1", &config, &client).unwrap();
    assert!(out.dangerous, "kubectl drain must be flagged dangerous");
}

#[test]
fn kubectl_cordon_is_dangerous() {
    let client =
        MockLlmClient::returning(r#"{"command":"kubectl cordon worker-2","dangerous":false}"#);
    let config = Config::default();
    let (out, _findings) = run("cordon worker-2", &config, &client).unwrap();
    assert!(out.dangerous, "kubectl cordon must be flagged dangerous");
}
