use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// A single token/group breakdown entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct RegexPart {
    pub token: String,
    pub means: String,
}

/// Output of `lxregexplain`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub regex: String,
    pub explanation: String,
    #[serde(default)]
    pub parts: Vec<RegexPart>,
}

/// Core logic for lxregexplain.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(regex: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if regex.trim().is_empty() {
        return Err(LxError::BadUsage("no regex provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: regex.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    parse_response::<Output>(&resp.content)
}
