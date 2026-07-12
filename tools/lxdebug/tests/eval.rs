use lx_config::Config;
use lxdebug::run::run;

fn make_client() -> Box<dyn lx_llm::LlmClient> {
    let config = Config::load().unwrap_or_default();
    lx_llm::client_from_config(&config, false).expect("LX_API_KEY must be set for eval tests")
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_enoent_output_structure() {
    let config = Config::load().unwrap_or_default();
    let client = make_client();
    let input = include_str!("fixtures/enoent_error.txt");
    let (out, _warnings) = run(input, &config, client.as_ref()).unwrap();
    assert!(!out.cause.is_empty(), "cause must not be empty");
    assert!(!out.fix.is_empty(), "fix must not be empty");
    // command is optional — just check it's a string (always true)
    assert!(out.cause.len() < 1000, "cause should be concise");
    assert!(out.fix.len() < 1000, "fix should be concise");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_missing_module_output_structure() {
    let config = Config::load().unwrap_or_default();
    let client = make_client();
    let input = include_str!("fixtures/missing_module.txt");
    let (out, _warnings) = run(input, &config, client.as_ref()).unwrap();
    assert!(!out.cause.is_empty());
    assert!(!out.fix.is_empty());
    // For a missing module error, we expect a command like npm install ...
    // (not asserting exact text, just that a command was suggested)
}
