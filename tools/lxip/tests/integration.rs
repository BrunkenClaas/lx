use lx_config::Config;
use lx_testkit::mock::MockLlmClient;
use lxip::run::run;

#[test]
fn generate_mode_output_schema_valid() {
    let json = r#"{"command":"ip route add 10.0.0.0/24 via 192.168.1.254","explanation":"Adds route.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let (out, explain_mode) = run(
        "add route to 10.0.0.0/24 via 192.168.1.254",
        "",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.command.is_empty());
    assert!(!explain_mode);
    lx_testkit::assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn explain_mode_triggered_by_stdin_no_intent() {
    let json = r#"{"command":"","explanation":"Standard routing table.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let (out, explain_mode) = run(
        "",
        "default via 192.168.1.1 dev eth0",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.explanation.is_empty());
    assert!(explain_mode);
}

#[test]
fn no_intent_no_state_returns_error() {
    let client = MockLlmClient::returning("{}");
    let result = run("", "", "linux", &Config::default(), &client);
    assert!(result.is_err());
}

#[test]
fn empty_command_with_explanation_surfaces_refusal() {
    // Model refuses a destructive request: valid JSON, empty command, explanation
    // carries the reason. The error must embed that reason, not hide it.
    let json = r#"{"command":"","explanation":"Deleting all routes is destructive; specify which routes.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let err = run(
        "delete all routing entries",
        "",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("declined"), "got: {msg}");
    assert!(
        msg.contains("Deleting all routes is destructive"),
        "refusal explanation must be surfaced, got: {msg}"
    );
}

#[test]
fn empty_command_no_explanation_falls_back() {
    let json = r#"{"command":"","explanation":"","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let err = run("do something", "", "linux", &Config::default(), &client).unwrap_err();
    assert!(err.to_string().contains("declined"), "got: {err}");
}

#[test]
fn dangerous_default_route_delete_detected() {
    assert!(lxip::run::check_dangerous("ip route del default"));
}

#[test]
fn dangerous_route_flush_detected() {
    assert!(lxip::run::check_dangerous("ip route flush"));
}

#[test]
fn dangerous_windows_route_delete_all_detected() {
    assert!(lxip::run::check_dangerous("netsh route delete all"));
    assert!(lxip::run::check_dangerous("route delete *"));
}

#[test]
fn dangerous_macos_route_flush_detected() {
    assert!(lxip::run::check_dangerous("route -n flush"));
}

#[test]
fn safe_command_not_flagged() {
    assert!(!lxip::run::check_dangerous(
        "ip route add 10.0.0.0/24 via 192.168.1.1"
    ));
}

#[test]
fn safe_windows_add_address_not_flagged() {
    assert!(!lxip::run::check_dangerous(
        "New-NetIPAddress -InterfaceAlias Ethernet -IPAddress 192.168.1.10 -PrefixLength 24"
    ));
}

#[test]
fn target_linux_system_prompt_contains_linux() {
    let json = r#"{"command":"ip addr show","explanation":"Shows addresses.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let _ = run(
        "show all addresses",
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
    let json = r#"{"command":"New-NetIPAddress -IPAddress 10.0.0.1 -InterfaceAlias Ethernet","explanation":"ok","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let _ = run(
        "add address 10.0.0.1 to Ethernet",
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
    let json = r#"{"command":"ifconfig en0 10.0.0.1 netmask 255.255.255.0","explanation":"ok","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let _ = run("set ip on en0", "", "macos", &Config::default(), &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("macos"),
        "system prompt must contain 'macos'"
    );
}

#[test]
fn os_mismatch_linux_state_windows_target() {
    let linux_state = "default via 192.168.1.1 dev eth0\n192.168.1.0/24 dev eth0 proto kernel";
    let warn = lxip::run::detect_os_mismatch(linux_state, "windows");
    assert!(
        warn.is_some(),
        "should detect mismatch for Linux state with Windows target"
    );
}

#[test]
fn os_mismatch_same_os_returns_none() {
    let linux_state = "default via 192.168.1.1 dev eth0";
    let warn = lxip::run::detect_os_mismatch(linux_state, "linux");
    assert!(warn.is_none(), "same-OS should not warn");
}

#[test]
fn snapshot_generate_plain() {
    let json = r#"{"command":"ip route add 10.0.0.0/24 via 192.168.1.1","explanation":"Adds route.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let (out, explain_mode) = run("add route", "", "linux", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain(explain_mode));
}

#[test]
fn snapshot_json_output() {
    let json = r#"{"command":"ip route add 10.0.0.0/24 via 192.168.1.1","explanation":"Adds route.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let (out, _) = run("add route", "", "linux", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
