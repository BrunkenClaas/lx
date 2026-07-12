use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// Output of `lxcert`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub subject: String,
    pub issuer: String,
    pub valid_until: String,
    pub notes: Vec<String>,
}

impl Output {
    /// Render as human-readable plain text (all fields go to stdout — cert explanation IS the result).
    pub fn to_plain(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Subject:     {}\n", self.subject));
        out.push_str(&format!("Issuer:      {}\n", self.issuer));
        out.push_str(&format!("Valid until: {}\n", self.valid_until));
        if !self.notes.is_empty() {
            out.push_str("Notes:\n");
            for note in &self.notes {
                out.push_str(&format!("  • {note}\n"));
            }
        }
        out
    }
}

/// Validate that the input looks like a PEM certificate.
/// Returns the trimmed PEM if valid, or an error.
fn validate_pem(input: &str) -> Result<&str, LxError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }
    if !trimmed.contains("-----BEGIN CERTIFICATE-----") {
        return Err(LxError::BadUsage(
            "input does not appear to be a PEM certificate (missing -----BEGIN CERTIFICATE----- header)".to_string(),
        ));
    }
    Ok(trimmed)
}

/// Core logic for lxcert.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
/// The PEM certificate is passed directly to the LLM which extracts and
/// explains the certificate fields. No external crypto crates are used.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    let pem = validate_pem(input)?;

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: pem,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    parse_response::<Output>(&resp.content)
}
