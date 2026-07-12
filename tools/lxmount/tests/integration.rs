use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxmount::run::run;

fn mock_safe() -> &'static str {
    r#"{"command":"mount -t ntfs-3g /dev/sdb1 /media/usb -o rw,uid=1000,gid=1000","fstab_line":"/dev/sdb1  /media/usb  ntfs-3g  rw,uid=1000,gid=1000,auto  0  0","notes":"Replace /dev/sdb1 with the actual device path from lsblk."}"#
}

fn mock_nfs() -> &'static str {
    r#"{"command":"mount -t nfs 192.168.1.10:/data /mnt/data","fstab_line":"192.168.1.10:/data  /mnt/data  nfs  defaults,_netdev  0  0","notes":"Ensure nfs-common is installed."}"#
}

fn mock_dangerous() -> &'static str {
    r#"{"command":"mkfs.ext4 /dev/sdb1","fstab_line":"/dev/sdb1  /mnt/data  ext4  defaults  0  2","notes":"This will format the device."}"#
}

fn mock_explain() -> &'static str {
    r#"{"command":"","fstab_line":"","notes":"","explanation":"/dev/sda1 is the root filesystem on ext4."}"#
}

fn mock_windows() -> &'static str {
    r#"{"command":"New-PSDrive -Name Z -PSProvider FileSystem -Root \\\\server\\share -Persist","fstab_line":null,"notes":"Run in elevated PowerShell."}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, explain_mode) = run(
        "mount a USB drive at /media/usb read-write with NTFS filesystem",
        "",
        "linux",
        &config,
        &client,
    )
    .unwrap();
    assert!(!explain_mode, "should be generate mode");
    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(out.fstab_line.is_some(), "fstab_line must be Some on linux");
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn with_context_includes_system_state() {
    let client = MockLlmClient::returning(mock_nfs());
    let config = Config::default();
    let fstab_context =
        "/dev/sda1  /  ext4  defaults  0  1\n/dev/sda2  /home  ext4  defaults  0  2";
    let (out, _) = run(
        "mount NFS share from 192.168.1.10:/data at /mnt/data",
        fstab_context,
        "linux",
        &config,
        &client,
    )
    .unwrap();
    assert!(!out.command.is_empty());
    let req = client.last_request();
    assert!(
        req.user.contains("Current system state"),
        "user message must include context header"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn dangerous_command_flagged() {
    let client = MockLlmClient::returning(mock_dangerous());
    let config = Config::default();
    let (out, _) = run(
        "format and mount /dev/sdb1 at /mnt/data",
        "",
        "linux",
        &config,
        &client,
    )
    .unwrap();
    assert!(
        out.dangerous,
        "mkfs in command must be flagged as dangerous"
    );
}

#[test]
fn empty_description_and_context_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let err = run("", "", "linux", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn max_tokens_within_limit() {
    let client = MockLlmClient::returning(mock_safe());
    let _ = run(
        "mount a USB drive at /media/usb",
        "",
        "linux",
        &Config::default(),
        &client,
    );
    assert!(client.last_request().max_tokens <= 384);
}

#[test]
fn windows_target_sets_fstab_line_to_none() {
    let client = MockLlmClient::returning(mock_windows());
    let config = Config::default();
    let (out, explain_mode) = run(
        "map server share to drive Z permanently",
        "",
        "windows",
        &config,
        &client,
    )
    .unwrap();
    assert!(!explain_mode);
    assert!(
        out.fstab_line.is_none(),
        "fstab_line must be None on windows target"
    );
}

#[test]
fn target_linux_system_prompt_contains_linux() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let _ = run("mount USB at /media/usb", "", "linux", &config, &client).unwrap();
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
    let client = MockLlmClient::returning(mock_windows());
    let config = Config::default();
    let _ = run("map share to Z", "", "windows", &config, &client).unwrap();
    let req = client.last_request();
    assert!(
        req.system.contains("windows"),
        "system prompt must contain 'windows'"
    );
}

#[test]
fn os_mismatch_linux_state_windows_target() {
    let linux_state = "/dev/sda1 / ext4 rw 0 1";
    let warn = lxmount::run::detect_os_mismatch(linux_state, "windows");
    assert!(warn.is_some(), "should detect mismatch");
}

#[test]
fn os_mismatch_same_os_returns_none() {
    let linux_state = "/dev/sda1 / ext4 rw 0 1";
    let warn = lxmount::run::detect_os_mismatch(linux_state, "linux");
    assert!(warn.is_none(), "same-OS should not warn");
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, explain_mode) = run(
        "mount a USB drive at /media/usb read-write",
        "",
        "linux",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(out.to_plain(explain_mode));
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (out, _) = run(
        "mount a USB drive at /media/usb read-write",
        "",
        "linux",
        &config,
        &client,
    )
    .unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn explain_mode_activates_when_context_only() {
    let client = MockLlmClient::returning(mock_explain());
    let config = Config::default();
    let context = "/dev/sda1 / ext4 rw 0 1";
    let (out, explain_mode) = run("", context, "linux", &config, &client).unwrap();
    assert!(explain_mode, "no description + context → explain mode");
    assert!(!out.explanation.is_empty(), "explanation must not be empty");
    let req = client.last_request();
    assert!(
        req.user.contains("Explain this mount configuration"),
        "explain mode must use explain instruction"
    );
    assertions::assert_request_invariants(&req);
}

#[test]
fn create_mode_user_message_is_plain_request() {
    let client = MockLlmClient::returning(mock_safe());
    let config = Config::default();
    let (_out, explain_mode) = run(
        "mount USB drive at /media/usb",
        "",
        "linux",
        &config,
        &client,
    )
    .unwrap();
    assert!(!explain_mode);
    let req = client.last_request();
    assert!(
        !req.user.contains("Explain"),
        "generate mode must not include explain instruction"
    );
}
