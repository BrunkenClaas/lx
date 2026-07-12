#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_spam_classification() {
    let api_key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };
    let config = {
        let mut c = lx_config::Config::default();
        c.llm.api_key = Some(api_key);
        c
    };
    let client = lx_llm::client_from_config(&config, false).expect("client must build");
    let labels = vec!["spam".to_string(), "ham".to_string()];
    let out = lxclass::run::run(
        "Congratulations! You have won a FREE prize. Click here now!",
        &labels,
        &config,
        client.as_ref(),
    )
    .expect("run must succeed");
    assert!(!out.label.is_empty(), "label must not be empty");
    assert!(
        labels.contains(&out.label),
        "label must be one of {:?}",
        labels
    );
    assert!(
        out.confidence > 0.0 && out.confidence <= 1.0,
        "confidence must be in (0.0, 1.0]"
    );
    assert_eq!(out.all.len(), 2, "all must have one entry per label");
}

#[test]
#[ignore = "eval: requires LX_API_KEY"]
fn eval_sentiment_classification() {
    let api_key = match std::env::var("LX_API_KEY") {
        Ok(k) => k,
        Err(_) => return,
    };
    let config = {
        let mut c = lx_config::Config::default();
        c.llm.api_key = Some(api_key);
        c
    };
    let client = lx_llm::client_from_config(&config, false).expect("client must build");
    let labels = vec![
        "positive".to_string(),
        "negative".to_string(),
        "neutral".to_string(),
    ];
    let out = lxclass::run::run(
        "The product works great and delivery was fast. Highly recommended!",
        &labels,
        &config,
        client.as_ref(),
    )
    .expect("run must succeed");
    assert!(
        labels.contains(&out.label),
        "label must be one of {:?}",
        labels
    );
    assert_eq!(out.all.len(), 3, "all must have one entry per label");
}
