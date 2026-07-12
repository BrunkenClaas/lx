use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// Output of `lxdns`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub explanation: String,
    pub likely_cause: String,
    pub suggested_fix: String,
}

impl Output {
    pub fn to_plain(&self) -> String {
        self.explanation.clone()
    }
}

/// Core logic for lxdns.
///
/// Diagnoses DNS problems from dig/nslookup/host output.
/// The `domain` argument, if non-empty, is prepended to the user message as context.
/// This function is pure — no I/O, no process::exit.
pub fn run(
    input: &str,
    domain: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe dig/nslookup/host output via stdin or use --file".to_string(),
        ));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_msg = if domain.trim().is_empty() {
        input.trim().to_string()
    } else {
        format!("Domain: {}\n\n{}", domain.trim(), input.trim())
    };

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

    if out.explanation.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty explanation".to_string(),
        ));
    }

    Ok(out)
}
