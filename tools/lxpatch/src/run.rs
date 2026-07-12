#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub diff: String,
    pub summary: String,
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    pub fn to_plain(&self) -> String {
        self.diff.clone()
    }
}

pub fn run(
    file_content: &str,
    description: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no change description provided".to_string(),
        ));
    }
    if file_content.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no file content provided on stdin".to_string(),
        ));
    }

    let user_msg = format!("[change: {}]\n---\n{}", description.trim(), file_content);

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;
    let mut out = parse_response::<Output>(&resp.content)?;

    if out.diff.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty diff".to_string(),
        ));
    }

    if is_dangerous(&out.diff) {
        out.dangerous = true;
    }

    Ok(out)
}

fn is_dangerous(diff: &str) -> bool {
    let c = diff.to_lowercase();
    [
        "rm -rf",
        ":(){:|:&};:",
        "dd of=/dev/",
        "mkfs",
        "curl | sh",
        "curl|sh",
        "wget | sh",
        "wget|sh",
        "| bash",
        "|bash",
    ]
    .iter()
    .any(|p| c.contains(p))
}
