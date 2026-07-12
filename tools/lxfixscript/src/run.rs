use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, inject_os, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub script: String,
    #[serde(default)]
    pub changes: Vec<String>,
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    pub fn to_plain(&self) -> String {
        self.script.clone()
    }
}

/// Pure core logic. `script` is the broken script content; `error_msg` is the optional error string.
/// `target_os` is one of `"linux" | "windows" | "macos"`.
pub fn run(
    script: &str,
    error_msg: &str,
    target_os: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if script.trim().is_empty() {
        return Err(LxError::BadUsage("no script provided".to_string()));
    }

    let system = inject_os(
        &inject_lang(SYSTEM_TEMPLATE, &config.output.lang),
        target_os,
    );

    let user_msg = if error_msg.trim().is_empty() {
        script.to_string()
    } else {
        format!("[error: {}]\n---\n{}", error_msg.trim(), script)
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

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.script.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty script".to_string(),
        ));
    }

    // Local danger detection — deterministic, not delegated to the LLM.
    if is_dangerous(&out.script) {
        out.dangerous = true;
    }

    Ok(out)
}

fn is_dangerous(script: &str) -> bool {
    let c = script.to_lowercase();
    [
        "rm -rf /",
        "rm -rf /*",
        "rm -fr /",
        ":(){:|:&};:",
        ":(){ :|:& };:",
        "dd if=",
        "dd of=/dev/",
        "mkfs ",
        "> /dev/sda",
        "curl | sh",
        "curl|sh",
        "wget | sh",
        "wget|sh",
        "curl | bash",
        "curl|bash",
        "| bash",
        "|bash",
        "iwr | iex",
        "iwr|iex",
        "invoke-expression",
        "remove-item -recurse /",
        "remove-item -recurse c:\\",
        "drop table",
        "drop database",
        "format c:",
        "shred",
    ]
    .iter()
    .any(|p| c.contains(p))
}
