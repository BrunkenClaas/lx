#![forbid(unsafe_code)]

use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxperm::run::{parse_ls_output, run, Output, PermItem};

fn mock_response() -> &'static str {
    r#"{"items":[{"perm":"-rwxr-xr-x","file":"deploy.sh","risk":"warning","explanation":"Owner can read, write, and execute. Group and others can read and execute. This script is world-executable — anyone on the system can run it."}]}"#
}

fn mock_multi_response() -> &'static str {
    r#"{
  "items": [
    {
      "perm": "-rw-r--r--",
      "file": "config.txt",
      "risk": "standard",
      "explanation": "Owner can read and write; group and others can only read. Standard read-only sharing."
    },
    {
      "perm": "-rwxrwxrwx",
      "file": "script.sh",
      "risk": "warning",
      "explanation": "All users can read, write, and execute. Anyone can modify this script."
    },
    {
      "perm": "drwxrwxrwx",
      "file": "uploads",
      "risk": "critical",
      "explanation": "World-writable directory. Any user can create or delete files here."
    }
  ]
}"#
}

// ── Schema / invariants ──────────────────────────────────────────────────────

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "-rwxr-xr-x  1 alice staff 1024 Jan 1 12:00 deploy.sh",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.items.is_empty(), "items must not be empty");
    let item = &out.items[0];
    assert!(!item.perm.is_empty(), "perm must not be empty");
    assert!(!item.file.is_empty(), "file must not be empty");
    assert!(!item.risk.is_empty(), "risk must not be empty");
    assert!(
        !item.explanation.is_empty(),
        "explanation must not be empty"
    );
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(
        "-rw-r--r-- 1 user group 512 Jan 1 12:00 file.txt",
        &config,
        &client,
    );
    let req = client.last_request();
    assert!(
        req.max_tokens <= 4096,
        "lxperm max_tokens should be ≤ 4096, got {}",
        req.max_tokens
    );
}

#[test]
fn temperature_is_zero() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(
        "-rw-r--r-- 1 user group 512 Jan 1 12:00 file.txt",
        &config,
        &client,
    );
    let req = client.last_request();
    assert_eq!(req.temperature, 0.0, "temperature must be 0.0");
}

#[test]
fn system_prompt_is_nonempty() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let _ = run(
        "-rw-r--r-- 1 user group 512 Jan 1 12:00 file.txt",
        &config,
        &client,
    );
    let req = client.last_request();
    assert!(!req.system.is_empty(), "system prompt must not be empty");
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

// ── Output formatting ────────────────────────────────────────────────────────

#[test]
fn to_plain_contains_file_perm_and_risk() {
    let out = Output {
        items: vec![PermItem {
            perm: "-rwxr-xr-x".to_string(),
            file: "script.sh".to_string(),
            risk: "warning".to_string(),
            explanation: "Owner can execute. World-executable script.".to_string(),
        }],
    };
    let plain = out.to_plain();
    assert!(plain.contains("script.sh"), "plain must contain filename");
    assert!(
        plain.contains("-rwxr-xr-x"),
        "plain must contain perm string"
    );
    assert!(plain.contains("warning"), "plain must contain risk level");
    assert!(
        plain.contains("Owner can execute"),
        "plain must contain explanation"
    );
}

#[test]
fn to_plain_indents_explanation() {
    let out = Output {
        items: vec![PermItem {
            perm: "-rw-r--r--".to_string(),
            file: "config.txt".to_string(),
            risk: "standard".to_string(),
            explanation: "Standard permissions.".to_string(),
        }],
    };
    let plain = out.to_plain();
    // Explanation line must be indented (starts with spaces).
    let explanation_line = plain
        .lines()
        .find(|l| l.contains("Standard permissions."))
        .expect("explanation must appear in output");
    assert!(
        explanation_line.starts_with("  "),
        "explanation must be indented: {:?}",
        explanation_line
    );
}

// ── Snapshot tests ───────────────────────────────────────────────────────────

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "-rwxr-xr-x  1 alice staff 1024 Jan 1 12:00 deploy.sh",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(
        "-rwxr-xr-x  1 alice staff 1024 Jan 1 12:00 deploy.sh",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn snapshot_multi_item_plain() {
    let client = MockLlmClient::returning(mock_multi_response());
    let config = Config::default();
    let ls = concat!(
        "-rw-r--r--  1 user group  512 Jan  1 12:00 config.txt\n",
        "-rwxrwxrwx  1 user group 1024 Jan  1 12:00 script.sh\n",
        "drwxrwxrwx  2 root root  4096 Jan  1 12:00 uploads"
    );
    let out = run(ls, &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

// ── ls -l parser tests ───────────────────────────────────────────────────────

#[test]
fn parse_ls_output_handles_total_line() {
    let ls = "total 32\n-rw-r--r-- 1 user group 512 Jan 1 12:00 readme.txt";
    let items = parse_ls_output(ls);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].1, "readme.txt");
}

#[test]
fn parse_ls_output_handles_multiple_entries() {
    let ls = concat!(
        "total 16\n",
        "-rw-r--r-- 1 user group  512 Jan  1 12:00 a.txt\n",
        "-rwxr-xr-x 1 user group 1024 Jan  1 12:00 b.sh\n",
        "drwxr-xr-x 2 user group 4096 Jan  1 12:00 subdir"
    );
    let items = parse_ls_output(ls);
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].1, "a.txt");
    assert_eq!(items[1].1, "b.sh");
    assert_eq!(items[2].1, "subdir");
}

#[test]
fn parse_ls_output_skips_blank_lines() {
    let ls = "\n-rw-r--r-- 1 user group 42 Jan 1 12:00 file.txt\n\n";
    let items = parse_ls_output(ls);
    assert_eq!(items.len(), 1);
}

#[test]
fn parse_ls_output_symlink_strips_arrow() {
    let ls = "lrwxrwxrwx 1 user group 7 Jan 1 12:00 mylink -> /etc/hosts";
    let items = parse_ls_output(ls);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, "lrwxrwxrwx");
    assert_eq!(items[0].1, "mylink");
}
