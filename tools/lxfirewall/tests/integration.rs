use lx_config::Config;
use lx_testkit::mock::MockLlmClient;
use lxfirewall::run::{run, Output};

#[test]
fn generate_mode_output_schema_valid() {
    let json = r#"{"command":"iptables -A INPUT -p tcp --dport 22 -s 10.0.0.0/8 -j ACCEPT","explanation":"Allows SSH from 10/8.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let (out, explain_mode) = run(
        "allow SSH from 10.0.0.0/8",
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
    let json = r#"{"command":"","explanation":"These rules allow SSH from 10/8.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let (out, explain_mode) = run(
        "",
        "Chain INPUT\nACCEPT tcp dpt:ssh",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap();
    assert!(!out.explanation.is_empty());
    assert!(explain_mode);
}

#[test]
fn generate_mode_with_state() {
    let json = r#"{"command":"iptables -I INPUT 1 -s 192.168.50.0/24 -j DROP","explanation":"Blocks traffic from 192.168.50.0/24.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let (out, explain_mode) = run(
        "block all from 192.168.50.0/24",
        "Chain INPUT (policy ACCEPT)\nACCEPT tcp -- 10.0.0.0/8",
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
fn no_intent_no_state_returns_error() {
    let client = MockLlmClient::returning("{}");
    let result = run("", "", "linux", &Config::default(), &client);
    assert!(result.is_err());
}

#[test]
fn empty_command_with_explanation_surfaces_refusal() {
    // Model refuses a destructive request: valid JSON, empty command, explanation
    // carries the reason. The error must embed that reason, not hide it.
    let json = r#"{"command":"","explanation":"Deleting all firewall rules is dangerous; specify the tool and confirm.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let err = run(
        "delete all firewall rules",
        "",
        "linux",
        &Config::default(),
        &client,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("declined"), "got: {msg}");
    assert!(
        msg.contains("Deleting all firewall rules is dangerous"),
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
fn target_linux_system_prompt_contains_linux() {
    let json = r#"{"command":"iptables -A INPUT -p tcp --dport 80 -j ACCEPT","explanation":"ok","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let _ = run("allow HTTP", "", "linux", &Config::default(), &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("linux"),
        "system prompt must contain resolved OS 'linux', got: {}",
        &req.system[..req.system.len().min(200)]
    );
    assert!(
        !req.system.contains("{os}"),
        "{{os}} placeholder must be filled, not literal"
    );
}

#[test]
fn target_windows_system_prompt_contains_windows() {
    let json = r#"{"command":"netsh advfirewall firewall add rule name=\"Block\" dir=in action=block","explanation":"ok","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let _ = run(
        "block all inbound",
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
    let json = r#"{"command":"pfctl -f /etc/pf.conf","explanation":"ok","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let _ = run("reload pf rules", "", "macos", &Config::default(), &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("macos"),
        "system prompt must contain 'macos'"
    );
}

#[test]
fn os_mismatch_linux_state_windows_target() {
    let linux_state = "Chain INPUT (policy ACCEPT)\niptables -A INPUT -j ACCEPT";
    let warn = lxfirewall::run::detect_os_mismatch(linux_state, "windows");
    assert!(warn.is_some(), "should detect mismatch");
}

#[test]
fn os_mismatch_same_os_returns_none() {
    let linux_state = "Chain INPUT (policy ACCEPT)\niptables -A INPUT -j ACCEPT";
    let warn = lxfirewall::run::detect_os_mismatch(linux_state, "linux");
    assert!(warn.is_none(), "same-OS should not warn");
}

#[test]
fn dangerous_flush_detected() {
    assert!(lxfirewall::run::check_dangerous("iptables -F"));
}

#[test]
fn dangerous_ip6tables_flush_detected() {
    assert!(lxfirewall::run::check_dangerous("ip6tables -F"));
}

#[test]
fn dangerous_nft_flush_detected() {
    assert!(lxfirewall::run::check_dangerous("nft flush ruleset"));
}

#[test]
fn dangerous_ufw_reset_detected() {
    assert!(lxfirewall::run::check_dangerous("ufw reset"));
}

#[test]
fn dangerous_ssh_drop_detected() {
    assert!(lxfirewall::run::check_dangerous(
        "iptables -A INPUT --dport 22 -j DROP"
    ));
}

#[test]
fn dangerous_ssh_reject_detected() {
    assert!(lxfirewall::run::check_dangerous(
        "iptables -A INPUT --dport 22 -j REJECT"
    ));
}

#[test]
fn dangerous_ufw_port22_drop_detected() {
    assert!(lxfirewall::run::check_dangerous("ufw deny port 22 drop"));
}

#[test]
fn dangerous_windows_advfirewall_reset_detected() {
    assert!(lxfirewall::run::check_dangerous("netsh advfirewall reset"));
}

#[test]
fn dangerous_windows_delete_all_rules_detected() {
    assert!(lxfirewall::run::check_dangerous(
        "netsh advfirewall firewall delete rule name=all"
    ));
}

#[test]
fn dangerous_macos_pfctl_flush_detected() {
    assert!(lxfirewall::run::check_dangerous("pfctl -F all"));
}

#[test]
fn safe_command_not_flagged() {
    assert!(!lxfirewall::run::check_dangerous(
        "iptables -A INPUT -p tcp --dport 80 -j ACCEPT"
    ));
}

#[test]
fn safe_windows_add_rule_not_flagged() {
    assert!(!lxfirewall::run::check_dangerous(
        "netsh advfirewall firewall add rule name=\"Block 23\" dir=in action=block localport=23"
    ));
}

#[test]
fn safe_http_https_not_flagged() {
    assert!(!lxfirewall::run::check_dangerous(
        "iptables -A INPUT -p tcp -m multiport --dports 80,443 -j ACCEPT"
    ));
}

#[test]
fn output_to_plain_generate_mode_returns_command() {
    let out = Output {
        command: "iptables -A INPUT -p tcp --dport 80 -j ACCEPT".to_string(),
        explanation: "Allow HTTP".to_string(),
        warnings: vec![],
        dangerous: false,
    };
    assert_eq!(
        out.to_plain(false),
        "iptables -A INPUT -p tcp --dport 80 -j ACCEPT"
    );
}

#[test]
fn output_to_plain_explain_mode_returns_explanation() {
    let out = Output {
        command: String::new(),
        explanation: "These rules block all traffic except SSH.".to_string(),
        warnings: vec![],
        dangerous: false,
    };
    assert_eq!(
        out.to_plain(true),
        "These rules block all traffic except SSH."
    );
}

#[test]
fn snapshot_generate_plain() {
    let json = r#"{"command":"iptables -A INPUT -p tcp --dport 22 -s 10.0.0.0/8 -j ACCEPT","explanation":"ok","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let (out, explain_mode) = run("allow SSH", "", "linux", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain(explain_mode));
}

#[test]
fn snapshot_explain_plain() {
    let json = r#"{"command":"","explanation":"Chain INPUT has one ACCEPT for SSH from 10/8 then a catch-all DROP.","warnings":[]}"#;
    let client = MockLlmClient::returning(json);
    let fixture = include_str!("fixtures/iptables_rules.txt");
    let (out, explain_mode) = run("", fixture, "linux", &Config::default(), &client).unwrap();
    insta::assert_snapshot!(out.to_plain(explain_mode));
}
