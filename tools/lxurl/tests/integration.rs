#![forbid(unsafe_code)]

use lx_testkit::MockLlmClient;
use lxurl::fetch::strip_html;

// ── fetch.rs unit tests (delegated to fetch module, re-checked here) ──────────

#[test]
fn strip_html_removes_script_and_nav() {
    let html = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/simple.html"
    ))
    .unwrap();
    let text = strip_html(&html);
    assert!(!text.contains("alert"), "script must be stripped");
    assert!(!text.contains("hidden"), "style must be stripped");
    assert!(text.contains("Hello World"), "h1 content must remain");
    assert!(text.contains("Rust"), "body content must remain");
    // nav/header/footer stripped
    assert!(!text.contains("Site header"), "header must be stripped");
    assert!(!text.contains("Home | About"), "nav must be stripped");
    assert!(!text.contains("Footer content"), "footer must be stripped");
}

// ── run() tests with MockLlmClient (no network) ──────────────────────────────

fn mock_response() -> &'static str {
    r#"{"url":"https://example.com","title":"Example","answer":"It is about Rust.","truncated":false}"#
}

#[test]
fn output_schema_is_valid() {
    let client = MockLlmClient::returning(mock_response());
    // We can't call run() without a real network fetch, so we test the struct
    // shape by deserialising the mock response directly.
    let out: lxurl::run::Output = serde_json::from_str(mock_response()).unwrap();
    assert_eq!(out.url, "https://example.com");
    assert_eq!(out.title.as_deref(), Some("Example"));
    assert!(!out.answer.is_empty());
    assert!(!out.truncated);
    // Ensure client can be used without panicking.
    let _ = client;
}

#[test]
fn ssrf_check_rejects_localhost() {
    use lxurl::fetch::validate_url;
    assert!(validate_url("http://localhost/secret").is_err());
    assert!(validate_url("http://127.0.0.1/").is_err());
    assert!(validate_url("http://10.0.0.1/").is_err());
    assert!(validate_url("http://192.168.0.1/").is_err());
    assert!(validate_url("http://172.20.0.1/").is_err());
    assert!(validate_url("http://169.254.0.1/").is_err());
}

#[test]
fn ssrf_check_accepts_public_url() {
    use lxurl::fetch::validate_url;
    assert!(validate_url("https://example.com/page").is_ok());
    assert!(validate_url("http://8.8.8.8/").is_ok());
}

#[test]
fn ssrf_check_rejects_non_http_scheme() {
    use lxurl::fetch::validate_url;
    assert!(validate_url("ftp://example.com/").is_err());
    assert!(validate_url("file:///etc/passwd").is_err());
}

#[test]
fn to_plain_includes_url_and_answer() {
    let out = lxurl::run::Output {
        url: "https://example.com".to_string(),
        title: Some("Example".to_string()),
        answer: "It is about Rust.".to_string(),
        truncated: false,
    };
    let plain = out.to_plain();
    assert!(plain.contains("https://example.com"));
    assert!(plain.contains("Example"));
    assert!(plain.contains("It is about Rust."));
}

#[test]
fn to_plain_shows_truncated_warning() {
    let out = lxurl::run::Output {
        url: "https://example.com".to_string(),
        title: None,
        answer: "answer".to_string(),
        truncated: true,
    };
    let plain = out.to_plain();
    assert!(plain.contains("truncated"));
}

#[test]
fn snapshot_plain_output() {
    let out = lxurl::run::Output {
        url: "https://example.com".to_string(),
        title: Some("Example Domain".to_string()),
        answer: "This domain is for illustrative examples.".to_string(),
        truncated: false,
    };
    insta::assert_snapshot!(out.to_plain());
}

#[test]
fn snapshot_json_output() {
    let out = lxurl::run::Output {
        url: "https://example.com".to_string(),
        title: Some("Example Domain".to_string()),
        answer: "This domain is for illustrative examples.".to_string(),
        truncated: false,
    };
    insta::assert_snapshot!(serde_json::to_string_pretty(&out).unwrap());
}
