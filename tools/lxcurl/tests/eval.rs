#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_basic_get_request() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxcurl::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "GET all users from https://api.example.com/users",
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.command.is_empty(), "command must not be empty");
    assert!(
        out.command.contains("curl"),
        "command must contain 'curl': {}",
        out.command
    );
    assert!(
        out.command.contains("api.example.com"),
        "command must contain the URL: {}",
        out.command
    );
    assert!(!out.dangerous, "a simple GET should not be dangerous");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_post_request_with_json_body() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxcurl::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        r#"POST {"name":"Alice","role":"admin"} to https://api.example.com/users"#,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.command.is_empty());
    assert!(out.command.contains("curl"));
    assert!(
        out.command.to_uppercase().contains("POST"),
        "POST request must include POST method: {}",
        out.command
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure_is_valid() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxcurl::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "fetch https://httpbin.org/get with header Accept: application/json",
        &config,
        client.as_ref(),
    )
    .unwrap();

    // Verify the Output struct is well-formed.
    assert!(!out.command.is_empty());
    let json = serde_json::to_string(&out).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["command"].is_string());
    assert!(parsed["dangerous"].is_boolean());
}
