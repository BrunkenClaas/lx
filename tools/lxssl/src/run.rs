use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// Output of `lxssl`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub explanation: String,
    pub likely_cause: String,
    pub suggested_fix: String,
}

impl Output {
    /// The result field — explanation goes to stdout in plain mode.
    pub fn to_plain(&self) -> String {
        self.explanation.clone()
    }
}

/// Core logic for lxssl.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(
    input: &str,
    host: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_message;
    let user: &str = if host.is_empty() {
        input.trim()
    } else {
        user_message = format!("Host: {host}\n\n{}", input.trim());
        &user_message
    };

    let req = Request {
        system: &system,
        user,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let out = parse_response::<Output>(&resp.content)?;

    if out.explanation.is_empty() {
        return Err(LxError::BadUsage(
            "LLM returned empty explanation".to_string(),
        ));
    }

    Ok(out)
}
