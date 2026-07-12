use lx_config::Config;
use lx_testkit::MockLlmClient;
use lxping::run::run;

#[test]
fn output_schema_is_valid() {
    let json = r#"{"explanation":"100% packet loss detected.","verdict":"host"}"#;
    let client = MockLlmClient::returning(json);
    let out = run("ping output here", &Config::default(), &client).unwrap();
    assert!(!out.explanation.is_empty());
    assert_eq!(out.verdict, "host");
    lx_testkit::assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_error() {
    let client = MockLlmClient::returning("{}");
    let result = run("", &Config::default(), &client);
    assert!(result.is_err());
}

#[test]
fn unrecognized_verdict_defaults_to_network() {
    let json = r#"{"explanation":"Something went wrong.","verdict":"unknown_verdict"}"#;
    let client = MockLlmClient::returning(json);
    let out = run("traceroute output here", &Config::default(), &client).unwrap();
    assert_eq!(out.verdict, "network");
}

#[test]
fn snapshot_plain_output() {
    let json = r#"{"explanation":"100% packet loss.","verdict":"host"}"#;
    let client = MockLlmClient::returning(json);
    let out = run("ping output", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let json = r#"{"explanation":"100% packet loss.","verdict":"host"}"#;
    let client = MockLlmClient::returning(json);
    let out = run("ping output", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
