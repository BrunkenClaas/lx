use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 768;

/// Output of `lxdraft`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// Subject line for email/ticket kinds; null for reply/message kinds.
    pub subject: Option<String>,
    /// The full draft body text.
    pub body: String,
}

/// Core logic for lxdraft — with mandatory redaction (§8.1).
///
/// Redacts the input BEFORE it reaches the LLM. No exceptions.
pub fn run(
    input: &str,
    kind: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pass bullet points as argument or via stdin".to_string(),
        ));
    }

    // MANDATORY: redact before LLM (SEC: redact flag).
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(input, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    send_to_llm(&redacted, kind, config, client)
}

/// Variant used when `--no-redact` is passed by the user.
///
/// Sends the raw input to the LLM without redaction. The caller is responsible
/// for having already warned the user prominently about the risk.
pub fn run_no_redact(
    input: &str,
    kind: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pass bullet points as argument or via stdin".to_string(),
        ));
    }

    send_to_llm(input, kind, config, client)
}

/// Build and send the LLM request, parse and validate the response.
fn send_to_llm(
    user_content: &str,
    kind: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    // Prepend the kind hint to the user message so the model knows the format.
    let user_msg = format!("(kind={kind}) {user_content}");

    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let out = parse_response::<Output>(&resp.content)?;

    if out.body.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty draft body".to_string(),
        ));
    }

    Ok(out)
}
