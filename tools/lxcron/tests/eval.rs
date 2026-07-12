#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_generate_produces_valid_cron() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxcron::run::{run, Mode};
    let config = Config::load().unwrap_or_default();
    let client = client_from_config(&config, false).unwrap();
    let out = run(
        "every weekday at 9am run backup.sh",
        Mode::Generate,
        &config,
        client.as_ref(),
    )
    .unwrap();
    assert!(!out.crontab.is_empty());
    // A valid crontab line has at least 6 whitespace-separated fields
    assert!(out.crontab.split_whitespace().count() >= 6);
}
