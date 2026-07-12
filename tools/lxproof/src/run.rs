use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 2048;

/// A single change made during proofreading.
#[derive(Debug, Serialize, Deserialize)]
pub struct Change {
    pub original: String,
    pub corrected: String,
    pub reason: String,
}

/// Output of `lxproof`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub text: String,
    pub changes: Vec<Change>,
}

/// Core logic for lxproof.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
/// Security flag: untrusted — the system prompt instructs the model to ignore
/// instructions embedded in the user-provided text.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: input.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    parse_response::<Output>(&resp.content)
}
