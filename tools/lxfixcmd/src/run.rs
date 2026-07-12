use crate::danger;
use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const EXAMPLES_BASH: &str = include_str!("../prompts/examples_bash.txt");
const EXAMPLES_POWERSHELL: &str = include_str!("../prompts/examples_powershell.txt");
const EXAMPLES_CMD: &str = include_str!("../prompts/examples_cmd.txt");
const MAX_TOKENS: u32 = 256;

pub fn examples_for(shell: &str) -> &'static str {
    match shell {
        "powershell" => EXAMPLES_POWERSHELL,
        "cmd" => EXAMPLES_CMD,
        _ => EXAMPLES_BASH,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FixOutput {
    pub command: String,
    pub reason: String,
    #[serde(default)]
    pub dangerous: bool,
}

impl FixOutput {
    #[allow(dead_code)]
    pub fn to_plain(&self) -> String {
        format!("{}\n", self.command)
    }
}

/// Pure core logic. `failed_cmd` is the positional arg; `error_context` is optional stdin.
pub fn run(
    failed_cmd: &str,
    error_context: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<FixOutput, LxError> {
    if failed_cmd.trim().is_empty() {
        return Err(LxError::BadUsage("no failed command provided".to_string()));
    }
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang)
        .replace("{shell}", &config.output.shell)
        .replace("{examples}", examples_for(&config.output.shell));
    let user_msg = if error_context.trim().is_empty() {
        format!("Failed command: {}", failed_cmd.trim())
    } else {
        format!(
            "Failed command: {}\n\nError output:\n{}",
            failed_cmd.trim(),
            error_context.trim()
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
    let mut out = parse_response::<FixOutput>(&resp.content)?;
    if out.command.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty command".to_string(),
        ));
    }
    // Local danger detection — deterministic, not delegated to the LLM.
    if danger::check_and_warn(&out.command) {
        out.dangerous = true;
    }
    Ok(out)
}
