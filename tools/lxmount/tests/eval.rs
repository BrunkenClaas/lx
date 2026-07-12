#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_generates_mount_command() {
    let api_key = std::env::var("LX_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return;
    }
    let mut config = lx_config::Config::default();
    config.llm.api_key = Some(api_key);
    let client = lx_llm::client_from_config(&config, false).unwrap();
    let (out, _explain_mode) = lxmount::run::run(
        "mount a USB drive at /media/usb read-write",
        "",
        "linux",
        &config,
        client.as_ref(),
    )
    .unwrap();
    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(
        out.fstab_line.as_deref().is_some_and(|f| !f.is_empty()),
        "fstab_line must not be empty on linux"
    );
}
