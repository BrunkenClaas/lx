use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub explanation: String,
    #[serde(default)]
    pub status: u16,
    pub likely_cause: String,
    pub suggested_fix: String,
}

impl Output {
    pub fn to_plain(&self) -> String {
        self.explanation.clone()
    }
}

pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input; pipe curl -v output or HTTP headers into lxhttp".to_string(),
        ));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
    let req = Request {
        system: &system,
        user: input.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;
    let out = parse_response::<Output>(&resp.content)?;

    if out.explanation.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty explanation".to_string(),
        ));
    }

    Ok(out)
}
