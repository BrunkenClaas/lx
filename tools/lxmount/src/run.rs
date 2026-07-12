use crate::danger;
use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, inject_os, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 384;

/// Output of `lxmount`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// Generate mode: the mount command. Explain mode: empty.
    #[serde(default)]
    pub command: String,
    /// Generate mode: the fstab entry (linux/macos) or null (windows). Explain mode: empty.
    #[serde(default)]
    pub fstab_line: Option<String>,
    /// Extra warnings or caveats.
    #[serde(default)]
    pub notes: String,
    /// Explain mode: plain-language explanation of the current mounts/fstab.
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    /// Plain output: command in generate mode, explanation in explain mode.
    pub fn to_plain(&self, explain_mode: bool) -> String {
        if explain_mode {
            self.explanation.clone()
        } else {
            self.command.clone()
        }
    }
}

/// Detect whether piped state looks like it came from a different OS than `target_os`.
pub fn detect_os_mismatch(context: &str, target_os: &str) -> Option<String> {
    let s = context.to_lowercase();
    match target_os {
        "windows" => {
            if s.contains("/dev/")
                || s.contains("/etc/fstab")
                || s.contains(" ext4 ")
                || s.contains(" ntfs-3g ")
            {
                Some(format!(
                    "piped state looks like Linux mount output but --target is {target_os}; \
                     generated commands will use Windows syntax"
                ))
            } else {
                None
            }
        }
        "macos" => {
            if s.contains("new-psdrive") || s.contains("mountvol") {
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
            if s.contains("new-psdrive") || s.contains("mountvol") {
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

/// Core logic for lxmount.
///
/// Create mode (`description` non-empty): generates mount command + fstab/persistence entry.
/// Explain mode (`description` empty, `context` non-empty): explains the current mounts.
/// `target_os` is one of `"linux" | "windows" | "macos"`.
/// NEVER executes the command. Checks for dangerous patterns locally.
/// On Windows, `fstab_line` is set to `None` (no fstab concept on Windows).
pub fn run(
    description: &str,
    context: &str,
    target_os: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, bool), LxError> {
    let explain_mode = description.trim().is_empty() && !context.trim().is_empty();

    if description.trim().is_empty() && context.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_os(
        &inject_lang(SYSTEM_TEMPLATE, &config.output.lang),
        target_os,
    );

    let user_msg = if explain_mode {
        format!("Explain this mount configuration:\n{}", context.trim())
    } else if context.trim().is_empty() {
        description.trim().to_string()
    } else {
        format!(
            "Request: {}\n\nCurrent system state:\n{}",
            description.trim(),
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

    let resp = client.complete(&req).map_err(LxError::from)?;
    let mut out = parse_response::<Output>(&resp.content)?;

    if !explain_mode && out.command.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty command".to_string(),
        ));
    }

    // Enforce no-fstab on Windows — the model may not always comply.
    if target_os == "windows" {
        out.fstab_line = None;
    }

    // Local danger detection — deterministic, never delegated to the LLM.
    if danger::check_and_warn(&out.command) {
        out.dangerous = true;
    }

    Ok((out, explain_mode))
}
