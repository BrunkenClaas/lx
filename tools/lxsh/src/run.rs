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

/// Output of `lxsh`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ShOutput {
    pub command: String,
    pub shell: String,
    pub dangerous: bool,
}

impl ShOutput {
    pub fn to_plain(&self) -> String {
        self.command.clone()
    }
}

/// Core logic for lxsh. Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// Generates a shell command from a plain-English description.
/// NEVER executes the command. Checks for dangerous patterns locally (§8.3);
/// returns any findings for `main.rs` to emit on stderr — this function never prints.
pub fn run(
    description: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(ShOutput, Vec<danger::Finding>), LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
    let system = system
        .replace("{shell}", &config.output.shell)
        .replace("{examples}", examples_for(&config.output.shell));

    let user_msg = format!(
        "Target shell: {}\n\n{}",
        config.output.shell,
        description.trim()
    );

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

    let mut out = parse_response::<ShOutput>(&resp.content)?;

    if out.command.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty command".to_string(),
        ));
    }

    // Local danger detection — deterministic, not delegated to the LLM.
    // If our pattern check detects danger but the model said safe, override.
    // Emission of the findings is main.rs's job (tier-3 stderr); run() stays pure.
    let findings = danger::check(&out.command);
    if !findings.is_empty() {
        out.dangerous = true;
    }

    Ok((out, findings))
}
