use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, inject_os, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

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
    /// Returns the result field for plain stdout:
    /// - explain mode: explanation
    /// - generate mode: command
    pub fn to_plain(&self, explain_mode: bool) -> String {
        if explain_mode {
            self.explanation.clone()
        } else {
            self.command.clone()
        }
    }
}

/// Detect dangerous firewall patterns using string matching only (no regex).
///
/// Returns `true` if the command contains patterns that could lock out access
/// or destroy all firewall rules.
pub fn check_dangerous(cmd: &str) -> bool {
    let c = cmd.to_lowercase();
    // Linux — flush/reset all rules.
    c.contains("iptables -f")
        || c.contains("ip6tables -f")
        || c.contains("nft flush ruleset")
        || c.contains("ufw reset")
        || (c.contains("port 22") && (c.contains(" drop") || c.contains(" reject")))
        || (c.contains("dport 22") && (c.contains("-j drop") || c.contains("-j reject")))
        // Windows — reset all firewall config or delete every rule.
        || c.contains("advfirewall reset")
        || c.contains("delete rule name=all")
        || (c.contains("remove-netfirewallrule") && c.contains("-all"))
        // macOS — disable or flush the packet filter.
        || c.contains("pfctl -f")
        || c.contains("pfctl -d")
}

/// Detect whether piped state looks like it came from a different OS than `target_os`.
///
/// Returns `Some(warning_msg)` on mismatch, `None` otherwise.
pub fn detect_os_mismatch(state: &str, target_os: &str) -> Option<String> {
    let s = state.to_lowercase();
    match target_os {
        "windows" => {
            if s.contains("chain input")
                || s.contains("iptables")
                || s.contains("nft ")
                || s.contains("ufw ")
            {
                Some(format!(
                    "piped state looks like Linux firewall output but --target is {target_os}; \
                     generated commands will use Windows syntax"
                ))
            } else {
                None
            }
        }
        "macos" => {
            if s.contains("chain input")
                || s.contains("iptables")
                || s.contains("new-netfirewallrule")
            {
                Some(format!(
                    "piped state does not match --target {target_os}; \
                     generated commands will use macOS pf/pfctl syntax"
                ))
            } else {
                None
            }
        }
        _ => {
            // linux
            if s.contains("new-netfirewallrule") || s.contains("netsh advfirewall") {
                Some(format!(
                    "piped state looks like Windows firewall output but --target is {target_os}; \
                     generated commands will use Linux syntax"
                ))
            } else {
                None
            }
        }
    }
}

/// Core logic for lxfirewall.
///
/// - Generate mode: intent provided; produces firewall commands (never executed).
/// - Explain mode: no intent, ruleset on stdin; explains what the rules do.
///
/// `target_os` is one of `"linux" | "windows" | "macos"` (from `--target` or `platform::os()`).
///
/// Returns `(Output, explain_mode)`. `explain_mode` is derived locally from the
/// presence/absence of intent and state.
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
            "provide an intent as argument (generate mode) or pipe existing rules (explain mode)"
                .to_string(),
        ));
    }

    let system = inject_os(
        &inject_lang(SYSTEM_TEMPLATE, &config.output.lang),
        target_os,
    );

    let user_msg = if explain_mode {
        format!("Explain these firewall rules:\n{}", state.trim())
    } else if state.trim().is_empty() {
        format!("Intent: {}", intent.trim())
    } else {
        format!(
            "Intent: {}\n\nCurrent ruleset:\n{}",
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
        // always a deliberate refusal of a destructive request (the prompt forbids
        // emitting flush-all rules without explicit guidance). Surface the model's
        // own explanation instead of discarding it behind an opaque error.
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

    // Local danger detection — deterministic, never delegated to the LLM.
    if check_dangerous(&out.command) {
        out.dangerous = true;
    }

    Ok((out, explain_mode))
}
