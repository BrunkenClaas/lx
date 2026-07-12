use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

#[derive(Debug, Serialize, Deserialize)]
pub struct PortOutput {
    pub port: u16,
    pub likely_service: String,
    pub explanation: String,
    pub risk: String, // "low" | "medium" | "high"
}

impl PortOutput {
    pub fn to_plain(&self) -> String {
        format!("{}\n", self.explanation)
    }
}

/// `port` is already validated by main.rs. `context` is optional netstat/ss output (may be empty).
pub fn run(
    port: u16,
    context: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<PortOutput, LxError> {
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
    let user_msg = if context.trim().is_empty() {
        format!("Port: {port}")
    } else {
        format!(
            "Port: {port}\n\nNetwork context (from ss/netstat):\n{}",
            context.trim()
        )
    };
    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };
    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;
    let mut out = parse_response::<PortOutput>(&resp.content)?;
    // Set port locally — do not trust the model's value.
    out.port = port;
    // Normalize risk to lowercase.
    out.risk = out.risk.to_lowercase();
    if out.explanation.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty explanation".to_string(),
        ));
    }
    Ok(out)
}
