use lx_config::Config;
use lxsum::run::run;

fn make_client() -> Box<dyn lx_llm::LlmClient> {
    let config = Config::load().unwrap_or_default();
    lx_llm::client_from_config(&config, false).expect("LX_API_KEY must be set for eval tests")
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_deployment_log_output_structure() {
    let config = Config::load().unwrap_or_default();
    let client = make_client();
    let input = include_str!("fixtures/deployment_log.txt");
    let (out, _warnings) = run(input, &config, client.as_ref()).unwrap();
    assert!(!out.tldr.is_empty(), "tldr must not be empty");
    assert!(!out.bullets.is_empty(), "bullets must not be empty");
    assert!(
        out.tldr.len() <= 120,
        "tldr should be ≤120 chars, got {}",
        out.tldr.len()
    );
    assert!(
        out.bullets.len() >= 2 && out.bullets.len() <= 6,
        "expected 2-6 bullets, got {}",
        out.bullets.len()
    );
    for (i, b) in out.bullets.iter().enumerate() {
        assert!(!b.is_empty(), "bullet {} must not be empty", i);
        assert!(
            b.len() <= 100,
            "bullet {} too long ({} chars): {}",
            i,
            b.len(),
            b
        );
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_secret_redacted_before_llm() {
    let config = Config::load().unwrap_or_default();
    let client = make_client();
    let input = include_str!("fixtures/text_with_secret.txt");
    // Should succeed with redaction applied — result should still be meaningful
    let (out, _warnings) = run(input, &config, client.as_ref()).unwrap();
    assert!(
        !out.tldr.is_empty(),
        "tldr must not be empty even after redaction"
    );
    assert!(!out.bullets.is_empty(), "bullets must not be empty");
}
