#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_email_regex_is_generated() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxregex::run::{run, Flavor};

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _warnings) = run(
        "email address",
        &Flavor::Pcre,
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.pattern.is_empty(), "pattern must not be empty");
    assert!(!out.explanation.is_empty(), "explanation must not be empty");
    // An email pattern should mention @ or domain.
    assert!(
        out.pattern.contains('@') || out.pattern.contains("\\@"),
        "email pattern should reference the @ symbol: {}",
        out.pattern
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_digits_only_rust_flavor() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxregex::run::{run, Flavor};

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _warnings) = run(
        "one or more digits",
        &Flavor::Rust,
        None,
        &config,
        client.as_ref(),
    )
    .unwrap();

    assert!(!out.pattern.is_empty());
    // The Rust pattern should use \d or [0-9].
    assert!(
        out.pattern.contains(r"\d") || out.pattern.contains("[0-9]"),
        "Rust digit pattern should use \\d or [0-9]: {}",
        out.pattern
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_output_structure_is_valid() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxregex::run::{run, Flavor};

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _warnings) =
        run("IPv4 address", &Flavor::Go, None, &config, client.as_ref()).unwrap();

    // Structural checks only — not exact text.
    assert!(!out.pattern.is_empty());
    assert!(!out.explanation.is_empty());
    // IPv4 involves dots and digit ranges.
    assert!(
        out.pattern.contains('.') || out.pattern.contains("0-9"),
        "IPv4 pattern should reference digits and dots: {}",
        out.pattern
    );
}
