use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, inject_os, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 384;

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    pub fn to_plain(&self, explain_mode: bool) -> String {
        if explain_mode {
            self.explanation.clone()
        } else {
            self.command.clone()
        }
    }
}

/// Detect whether piped state looks like it came from a different OS than `target_os`.
pub fn detect_os_mismatch(state: &str, target_os: &str) -> Option<String> {
    let s = state.to_lowercase();
    match target_os {
        "windows" => {
            if s.contains("dev eth")
                || s.contains("dev lo")
                || s.contains("inet ")
                || s.contains("ip route")
            {
                Some(format!(
                    "piped state looks like Linux ip output but --target is {target_os}; \
                     generated commands will use Windows syntax"
                ))
            } else {
                None
            }
        }
        "macos" => {
            if s.contains("new-netipaddress") || s.contains("netsh interface") {
                Some(format!(
                    "piped state looks like Windows output but --target is {target_os}; \
                     generated commands will use macOS syntax"
                ))
            } else {
                None
            }
        }
        _ => {
            // linux
            if s.contains("new-netipaddress")
                || s.contains("new-netroute")
                || s.contains("netsh interface")
            {
                Some(format!(
                    "piped state looks like Windows output but --target is {target_os}; \
                     generated commands will use Linux syntax"
                ))
            } else {
                None
            }
        }
    }
}

pub fn check_dangerous(cmd: &str) -> bool {
    let c = cmd.to_lowercase();
    // Linux — drop the default route, flush the table, or take down loopback.
    (c.contains("ip route del default") || c.contains("ip route delete default"))
        || c.contains("ip route flush")
        || (c.contains("ip link delete") && c.contains("lo"))
        || (c.contains("ip link del") && c.contains(" lo"))
        || (c.contains("ip link set") && c.contains(" lo ") && c.contains("down"))
        // Windows — wipe the routing table (netsh route delete all / route delete *).
        || c.contains("route delete all")
        || c.contains("route delete *")
        || c.contains("route -f")
        // macOS — flush the routing table.
        || (c.contains("route") && c.contains("flush"))
}

pub fn run(
    intent: &str,
    state: &str,
    target_os: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, bool), LxError> {
    let explain_mode = intent.trim().is_empty() && !state.trim().is_empty();

    if intent.trim().is_empty() && state.trim().is_empty() {
        return Err(LxError::BadUsage(
            "provide an intent as argument (generate mode) or pipe existing ip state (explain mode)".to_string(),
        ));
    }

    let system = inject_os(
        &inject_lang(SYSTEM_TEMPLATE, &config.output.lang),
        target_os,
    );

    let user_msg = if explain_mode {
        format!("Explain this ip state:\n{}", state.trim())
    } else if state.trim().is_empty() {
        format!("Intent: {}", intent.trim())
    } else {
        format!(
            "Intent: {}\n\nCurrent ip state:\n{}",
            intent.trim(),
            state.trim()
        )
    };

    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;
    let mut out = parse_response::<Output>(&resp.content)?;

    if !explain_mode && out.command.is_empty() {
        // The model filled valid JSON but left the command empty. This is almost
        // always a deliberate refusal of a destructive request (e.g. flushing all
        // routes). Surface the model's own explanation instead of discarding it
        // behind an opaque error.
        let reason = out.explanation.trim();
        let msg = if reason.is_empty() {
            "model declined to generate a command (no command returned)".to_string()
        } else {
            format!("model declined to generate a command: {reason}")
        };
        return Err(LxError::LogicalError(msg));
    }
    if explain_mode && out.explanation.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty explanation".to_string(),
        ));
    }

    if check_dangerous(&out.command) {
        out.dangerous = true;
    }

    Ok((out, explain_mode))
}
