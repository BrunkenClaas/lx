use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

/// Output of `lxmock`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub data: String,
    pub format: String,
}

impl Output {
    /// Return just the generated data for plain stdout.
    pub fn to_plain(&self) -> String {
        self.data.clone()
    }
}

/// Core logic for lxmock.
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

    let out: Output = parse_response(&resp.content)?;

    if out.data.is_empty() {
        return Err(LxError::LogicalError(
            "LLM returned empty data field".to_string(),
        ));
    }

    Ok(out)
}
