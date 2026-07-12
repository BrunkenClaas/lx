// Eval tests — ignored by default; run with `--include-ignored eval_` and LX_API_KEY set.
// All functions must be named eval_* and carry #[ignore = "eval: requires LX_API_KEY"].

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_formats_meeting_notes() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxmd::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let input = "meeting notes jan 15\nattendees: alice, bob, carol\nagenda\n- review project plan\n- discuss blockers\nnext steps: alice will update the plan by friday";
    let out = run(input, &config, client.as_ref()).unwrap();

    assert!(!out.markdown.is_empty(), "markdown must not be empty");
    // The output should be valid-looking Markdown with some structure
    let md = out.markdown.to_lowercase();
    assert!(
        md.contains("alice") || md.contains("attendee") || md.contains("meeting"),
        "output should reference the meeting content: {}",
        out.markdown
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_formats_bullet_points() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxmd::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let input = "how to deploy\nstep 1 run cargo build\nstep 2 copy binary to server\nstep 3 restart service";
    let out = run(input, &config, client.as_ref()).unwrap();

    assert!(!out.markdown.is_empty(), "markdown must not be empty");
    // Should contain Markdown list indicators or numbering
    assert!(
        out.markdown.contains('-') || out.markdown.contains('#') || out.markdown.contains("1."),
        "output should contain Markdown structure: {}",
        out.markdown
    );
}
