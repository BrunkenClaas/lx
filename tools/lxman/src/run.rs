use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// Output of `lxman`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ManOutput {
    pub summary: String,
    pub examples: Vec<String>,
}

impl ManOutput {
    /// Render as human-readable plain text.
    pub fn to_plain(&self) -> String {
        let mut out = format!("{}\n", self.summary);
        for (i, ex) in self.examples.iter().enumerate() {
            out.push_str(&format!("  {}. {ex}\n", i + 1));
        }
        out
    }
}

/// Core logic for lxman.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(tool_name: &str, config: &Config, client: &dyn LlmClient) -> Result<ManOutput, LxError> {
    let tool_name = tool_name.trim();
    if tool_name.is_empty() {
        return Err(LxError::BadUsage("no tool name provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: tool_name,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    parse_response::<ManOutput>(&resp.content)
}
