#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure() {
    let api_key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };
    let mut config = lx_config::Config::default();
    config.llm.api_key = Some(api_key);

    let client = lx_llm::client_from_config(&config, false).expect("client");
    let lockfile = include_str!("fixtures/cargo_lock_vulnerable.toml");
    let out = lxcve::run::run(lockfile, &config, client.as_ref()).expect("run");
    // Structure check: vulns must be a Vec (may be empty if model finds nothing).
    let _ = out.vulns.len();
    // If any vuln is present, verify required fields are non-empty.
    for v in &out.vulns {
        assert!(!v.package.is_empty(), "package must not be empty");
        assert!(!v.version.is_empty(), "version must not be empty");
        assert!(!v.severity.is_empty(), "severity must not be empty");
        assert!(!v.description.is_empty(), "description must not be empty");
        assert!(
            ["CRITICAL", "HIGH", "MEDIUM", "LOW"].contains(&v.severity.as_str()),
            "unexpected severity: {}",
            v.severity
        );
    }
}
