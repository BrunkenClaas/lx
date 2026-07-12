use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 2048;

/// Output of `lxmd`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The formatted Markdown string.
    pub markdown: String,
}

impl Output {
    /// Render as plain text (the markdown IS the result).
    pub fn to_plain(&self) -> String {
        self.markdown.clone()
    }
}

/// Core logic for lxmd.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
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
