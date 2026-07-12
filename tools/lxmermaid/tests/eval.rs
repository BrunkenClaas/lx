#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_basic_flowchart() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxmermaid::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "simple flow from start to end",
        None,
        &config,
        client.as_ref(),
    )
    .expect("run should succeed");

    assert!(!out.diagram.is_empty(), "diagram must not be empty");
    // Should start with a recognized Mermaid diagram type
    let diagram_lower = out.diagram.to_lowercase();
    let known_types = [
        "flowchart",
        "graph",
        "sequencediagram",
        "classdiagram",
        "erdiagram",
        "gantt",
        "pie",
        "statediagram",
    ];
    assert!(
        known_types.iter().any(|t| diagram_lower.starts_with(t)),
        "diagram should start with a known Mermaid type, got: {}",
        &out.diagram[..out.diagram.len().min(50)]
    );
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_sequence_diagram() {
    use lx_config::Config;
    use lx_llm::client_from_config;
    use lxmermaid::run::run;

    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();
    let (out, _findings) = run(
        "HTTP request and response between client and server",
        None,
        &config,
        client.as_ref(),
    )
    .expect("run should succeed");

    assert!(!out.diagram.is_empty(), "diagram must not be empty");
}
