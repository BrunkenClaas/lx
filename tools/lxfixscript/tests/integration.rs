use lx_config::Config;
use lx_testkit::mock::MockLlmClient;
use lxfixscript::run::run;

#[test]
fn output_schema_is_valid() {
    let mock = "{\"script\":\"#!/bin/bash\\necho hello\",\"changes\":[\"Fixed quoting\"]}";
    let client = MockLlmClient::returning(mock);
    let out = run(
        "#!/bin/bash\necho 'hello",
        "",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.script.is_empty());
    assert!(!out.dangerous);
    lx_testkit::assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn dangerous_script_is_flagged() {
    let mock = "{\"script\":\"rm -rf /\",\"changes\":[]}";
    let client = MockLlmClient::returning(mock);
    let out = run("rm -rf /", "", "linux", &Config::default(), &client).unwrap();
    assert!(out.dangerous);
}

#[test]
fn empty_script_returns_error() {
    let client = MockLlmClient::returning("{}");
    let err = run("", "", "linux", &Config::default(), &client).unwrap_err();
    assert!(matches!(err, lx_core::error::LxError::BadUsage(_)));
}

#[test]
fn error_msg_is_included_in_prompt() {
    let mock = "{\"script\":\"#!/bin/bash\\nfi\",\"changes\":[\"removed extra fi\"]}";
    let client = MockLlmClient::returning(mock);
    let _ = run(
        "#!/bin/bash\nfi\nfi",
        "unexpected fi",
        "linux",
        &Config::default(),
        &client,
    );
    let req = client.last_request();
    assert!(req.user.contains("unexpected fi"));
}

#[test]
fn target_linux_system_prompt_contains_linux() {
    let mock = "{\"script\":\"#!/bin/bash\\necho hello\",\"changes\":[]}";
    let client = MockLlmClient::returning(mock);
    let _ = run(
        "#!/bin/bash\necho hello",
        "",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap();
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
    let mock = "{\"script\":\"Write-Host hello\",\"changes\":[\"Fixed Write-Hos typo\"]}";
    let client = MockLlmClient::returning(mock);
    let _ = run(
        "Write-Hos hello",
        "",
        "windows",
        &Config::default(),
        &client,
    )
    .unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("windows"),
        "system prompt must contain 'windows'"
    );
}

#[test]
fn target_macos_system_prompt_contains_macos() {
    let mock = "{\"script\":\"#!/bin/zsh\\necho hello\",\"changes\":[]}";
    let client = MockLlmClient::returning(mock);
    let _ = run(
        "#!/bin/zsh\necho hello",
        "",
        "macos",
        &Config::default(),
        &client,
    )
    .unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("macos"),
        "system prompt must contain 'macos'"
    );
}

#[test]
fn snapshot_plain_output() {
    let mock = "{\"script\":\"#!/bin/bash\\necho hello\",\"changes\":[\"Fixed quoting\"]}";
    let client = MockLlmClient::returning(mock);
    let out = run(
        "#!/bin/bash\necho 'hello",
        "",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let mock = "{\"script\":\"#!/bin/bash\\necho hello\",\"changes\":[\"Fixed quoting\"]}";
    let client = MockLlmClient::returning(mock);
    let out = run(
        "#!/bin/bash\necho 'hello",
        "",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
