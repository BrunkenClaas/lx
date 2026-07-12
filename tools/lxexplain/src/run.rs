use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// Output of `lxexplain`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExplainOutput {
    pub summary: String,
    pub details: Vec<String>,
}

impl ExplainOutput {
    /// Render as human-readable plain text.
    pub fn to_plain(&self) -> String {
        let mut out = format!("{}\n", self.summary);
        for d in &self.details {
            out.push_str(&format!("  • {d}\n"));
        }
        out
    }
}

/// Core logic for lxexplain.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<ExplainOutput, LxError> {
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

    parse_response::<ExplainOutput>(&resp.content)
}
