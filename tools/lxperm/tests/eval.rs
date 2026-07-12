// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must be named eval_* and carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_explains_standard_permissions() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxperm::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let input = "-rw-r--r--  1 alice staff  512 Jan  1 12:00 readme.txt";
    let out = run(input, &config, client.as_ref()).unwrap();

    assert!(!out.items.is_empty(), "items must not be empty");
    let item = &out.items[0];
    assert!(
        !item.explanation.is_empty(),
        "explanation must not be empty"
    );
    assert!(!item.risk.is_empty(), "risk must not be empty");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_flags_world_writable_as_critical() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxperm::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let input = "drwxrwxrwx  2 root root 4096 Jan  1 12:00 /tmp/uploads";
    let out = run(input, &config, client.as_ref()).unwrap();

    assert!(!out.items.is_empty(), "items must not be empty");
    let item = &out.items[0];
    // A world-writable directory should be flagged as critical or warning.
    assert!(
        item.risk == "critical" || item.risk == "warning",
        "world-writable directory should be critical or warning, got: {}",
        item.risk
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_flags_suid_bit_as_warning_or_critical() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxperm::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let input = "-rwsr-xr-x  1 root root 47480 Jan  1 12:00 /usr/bin/passwd";
    let out = run(input, &config, client.as_ref()).unwrap();

    assert!(!out.items.is_empty(), "items must not be empty");
    let item = &out.items[0];
    // SUID bit should be flagged as warning or critical.
    assert!(
        item.risk == "warning" || item.risk == "critical",
        "SUID file should be warning or critical, got: {}",
        item.risk
    );
    // Explanation must mention SUID or setuid or 's'.
    let lower = item.explanation.to_lowercase();
    assert!(
        lower.contains("suid")
            || lower.contains("setuid")
            || lower.contains("set-user")
            || lower.contains("'s'"),
        "explanation should mention SUID: {}",
        item.explanation
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_multi_item_output() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxperm::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let input = concat!(
        "-rw-r--r--  1 alice staff   512 Jan  1 12:00 config.txt\n",
        "-rwxr-xr-x  1 alice staff  1024 Jan  1 12:00 deploy.sh\n",
        "drwxr-xr-x  2 alice staff  4096 Jan  1 12:00 logs\n",
    );
    let out = run(input, &config, client.as_ref()).unwrap();

    // Should produce at least one item per line (3 lines → ≥ 1 item minimum).
    assert!(!out.items.is_empty(), "multi-line input must produce items");

    // Every item must have non-empty required fields.
    for item in &out.items {
        assert!(!item.perm.is_empty(), "perm must not be empty");
        assert!(!item.file.is_empty(), "file must not be empty");
        assert!(!item.risk.is_empty(), "risk must not be empty");
        assert!(
            !item.explanation.is_empty(),
            "explanation must not be empty"
        );
    }
}
