use lx_config::Config;
use lx_llm::client_from_config;
use lxpull::run::run;

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_extracts_contacts() {
    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();

    let input = include_str!("fixtures/contacts.txt");
    let fields = vec!["name".to_string(), "email".to_string()];
    let out = run(input, &fields, &config, client.as_ref()).unwrap();

    assert!(
        !out.records.is_empty(),
        "should extract at least one record"
    );
    for record in &out.records {
        assert!(
            record.contains_key("name"),
            "each record must have 'name' field"
        );
        assert!(
            record.contains_key("email"),
            "each record must have 'email' field"
        );
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_extracts_invoices() {
    let config = Config::load().unwrap();
    let client = client_from_config(&config, false).unwrap();

    let input = include_str!("fixtures/invoices.txt");
    let fields = vec![
        "date".to_string(),
        "amount".to_string(),
        "description".to_string(),
    ];
    let out = run(input, &fields, &config, client.as_ref()).unwrap();

    assert!(
        !out.records.is_empty(),
        "should extract at least one invoice"
    );
    for record in &out.records {
        assert!(
            record.contains_key("date"),
            "each invoice must have 'date' field"
        );
        assert!(
            record.contains_key("amount"),
            "each invoice must have 'amount' field"
        );
    }
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_redaction_fires_on_credential_in_input() {
    use lx_testkit::RecordingLlmClient;
    use lxpull::run::run as lxpull_run;

    let config = Config::load().unwrap();
    let inner = client_from_config(&config, false).unwrap();
    let client = RecordingLlmClient::new(inner);

    let input = include_str!("fixtures/text_with_credential.txt");
    let fields = vec!["email".to_string(), "phone".to_string()];
    let _ = lxpull_run(input, &fields, &config, &client);

    let sent = client.last_user_message();
    assert!(
        !sent.contains("sk-abcdefghijklmnopqrstuvwxyz123456"),
        "raw credential must not reach LLM"
    );
    assert!(
        sent.contains("[REDACTED]"),
        "redacted placeholder must be present"
    );
}
