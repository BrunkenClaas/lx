#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Output of `lxprintf`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub format: String,
    pub explanation: String,
}

impl Output {
    /// Return just the format string for plain-mode stdout.
    pub fn to_plain(&self) -> String {
        self.format.clone()
    }
}

/// Core logic for lxprintf.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(description: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: description.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    parse_response::<Output>(&resp.content)
}
