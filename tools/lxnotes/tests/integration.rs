use lx_config::Config;
use lx_testkit::{assertions, MockLlmClient};
use lxnotes::run::{run, Output, Section};

fn mock_response() -> &'static str {
    r#"{"sections":[{"title":"Decisions","content":["Deploy on Thursdays at 9pm","Ops approval required"]},{"title":"Action Items","content":["John to finish dashboard by Friday"]}]}"#
}

fn sample_notes() -> &'static str {
    include_str!("fixtures/meeting_notes.txt")
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_notes(), &config, &client).unwrap();
    assert!(!out.sections.is_empty(), "sections must not be empty");
    for section in &out.sections {
        assert!(!section.title.is_empty(), "section title must not be empty");
        assert!(
            !section.content.is_empty(),
            "section content must not be empty"
        );
    }
    assertions::assert_request_invariants(&client.last_request());
}

#[test]
fn snapshot_plain_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_notes(), &config, &client).unwrap();
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let out = run(sample_notes(), &config, &client).unwrap();
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}

#[test]
fn secrets_never_reach_llm() {
    // Use a bearer credential fixture — redact flag must strip it before LLM sees it.
    let notes_with_secret =
        r#"meeting notes\nBEARER = "sk-abcdefghijklmnopqrstuvwxyz123456"\ndiscussed the roadmap"#;
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();

    // Redact the input before passing to run() — same as main.rs does.
    let level = lx_redact::RedactLevel::Standard;
    let redacted = lx_redact::redact(notes_with_secret, level).unwrap();
    let _ = run(&redacted, &config, &client);

    assertions::assert_no_secrets_in_request(&client.last_request());
}

#[test]
fn empty_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn whitespace_only_input_returns_bad_usage() {
    let client = MockLlmClient::returning(mock_response());
    let config = Config::default();
    let err = run("   \n  \t  ", &config, &client).unwrap_err();
    assert_eq!(err.exit_code(), lx_core::exit::BAD_USAGE);
}

#[test]
fn to_plain_formats_sections_correctly() {
    let out = Output {
        sections: vec![
            Section {
                title: "Decisions".to_string(),
                content: vec![
                    "Deploy on Thursdays".to_string(),
                    "Ops approval required".to_string(),
                ],
            },
            Section {
                title: "Action Items".to_string(),
                content: vec!["John to finish dashboard".to_string()],
            },
        ],
    };
    let plain = out.to_plain();
    assert!(plain.contains("## Decisions"));
    assert!(plain.contains("- Deploy on Thursdays"));
    assert!(plain.contains("## Action Items"));
    assert!(plain.contains("- John to finish dashboard"));
}
