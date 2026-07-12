use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 384;

const VALID_VERDICTS: &[&str] = &["network", "host", "dns", "ok"];

/// Output of `lxping`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub explanation: String,
    pub verdict: String,
}

impl Output {
    /// Plain output: explanation is the result field.
    pub fn to_plain(&self) -> String {
        self.explanation.clone()
    }
}

/// Core logic for lxping.
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

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.explanation.is_empty() {
        return Err(LxError::LogicalError(
            "LLM returned empty explanation".to_string(),
        ));
    }

    // Validate verdict; default to "network" if unrecognized.
    if !VALID_VERDICTS.contains(&out.verdict.as_str()) {
        out.verdict = "network".to_string();
    }

    Ok(out)
}
