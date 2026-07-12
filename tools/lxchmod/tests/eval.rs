#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_suggests_secure_permissions() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxchmod::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let input = "-rw-rw-rw- 1 user group 1234 Jan 01 12:00 data.csv";
    let (out, _findings) = run(input, &config, client.as_ref()).unwrap();

    assert!(!out.suggestion.is_empty(), "suggestion must not be empty");
    assert!(!out.reason.is_empty(), "reason must not be empty");
    // The suggestion should be a chmod command
    assert!(
        out.suggestion.contains("chmod"),
        "suggestion should contain chmod: {}",
        out.suggestion
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_restrictive_for_world_writable() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxchmod::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    // World-writable file — should suggest removing world-write
    let input = "-rw-rw-rw- 1 user group 512 Mar 05 10:00 config.json";
    let (out, _findings) = run(input, &config, client.as_ref()).unwrap();

    assert!(!out.suggestion.is_empty());
    // The suggestion should not make things more permissive
    assert!(
        !out.suggestion.contains("777"),
        "should not suggest 777 for world-writable file: {}",
        out.suggestion
    );
}
