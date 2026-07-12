use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxcve::run::{run, Output, Vuln};

// ── helpers ───────────────────────────────────────────────────────────────────

fn mock_response_with_vuln() -> &'static str {
    r#"{"vulns":[{"package":"openssl","version":"0.10.29","cve_id":"CVE-2022-0778","severity":"HIGH","description":"Infinite loop in BN_mod_sqrt() reachable when parsing certificates with invalid explicit elliptic curve parameters."}]}"#
}

fn mock_response_clean() -> &'static str {
    r#"{"vulns":[]}"#
}

fn vulnerable_lockfile() -> &'static str {
    include_str!("fixtures/cargo_lock_vulnerable.toml")
}

fn clean_lockfile() -> &'static str {
    include_str!("fixtures/package_lock_clean.json")
}

// ── schema / invariant tests ──────────────────────────────────────────────────

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response_with_vuln());
    let config = Config::default();
    let out = run(vulnerable_lockfile(), &config, &client).unwrap();
    assert!(!out.vulns.is_empty());
    let v = &out.vulns[0];
    assert!(!v.package.is_empty());
    assert!(!v.version.is_empty());
    assert!(!v.severity.is_empty());
    assert!(!v.description.is_empty());
    assert!(
        ["CRITICAL", "HIGH", "MEDIUM", "LOW"].contains(&v.severity.as_str()),
        "unexpected severity: {}",
        v.severity
    );
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn clean_lockfile_returns_empty_vulns() {
    let client = MockLlmClient::returning(mock_response_clean());
    let config = Config::default();
    let out = run(clean_lockfile(), &config, &client).unwrap();
    assert!(out.vulns.is_empty());
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response_clean());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response_clean());
    let config = Config::default();
    let err = run("   \n\t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

// ── output formatting tests ───────────────────────────────────────────────────

#[test]
fn to_plain_with_vuln() {
    let out = Output {
        vulns: vec![Vuln {
            package: "openssl".to_string(),
            version: "0.10.29".to_string(),
            cve_id: "CVE-2022-0778".to_string(),
            severity: "HIGH".to_string(),
            description: "Infinite loop in BN_mod_sqrt().".to_string(),
            confidence: "high".to_string(),
        }],
    };
    let plain = out.to_plain();
    assert!(plain.contains("[HIGH]"), "expected [HIGH] in: {}", plain);
    assert!(
        plain.contains("openssl"),
        "expected package name in: {}",
        plain
    );
    assert!(
        plain.contains("CVE-2022-0778"),
        "expected CVE ID in: {}",
        plain
    );
}

#[test]
fn to_plain_no_vulns() {
    let out = Output { vulns: vec![] };
    let plain = out.to_plain();
    assert!(
        plain.contains("No known CVEs"),
        "expected no-CVE message: {}",
        plain
    );
}

#[test]
fn to_plain_unknown_cve_id() {
    let out = Output {
        vulns: vec![Vuln {
            package: "somelib".to_string(),
            version: "1.2.3".to_string(),
            cve_id: "".to_string(),
            severity: "MEDIUM".to_string(),
            description: "Unvalidated input can cause a crash.".to_string(),
            confidence: "low".to_string(),
        }],
    };
    let plain = out.to_plain();
    assert!(
        plain.contains("CVE unknown"),
        "expected 'CVE unknown' for empty cve_id: {}",
        plain
    );
}

// ── snapshot tests ────────────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response_with_vuln());
    let config = Config::default();
    let out = run(vulnerable_lockfile(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response_with_vuln());
    let config = Config::default();
    let out = run(vulnerable_lockfile(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
