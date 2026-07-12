use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;
/// Configs rarely exceed this; large files get truncated upstream.
const MAX_CONFIG_BYTES: usize = 64_000;

/// A single finding reported by the config auditor.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    /// 1-based line number, or null if the finding applies to the whole file.
    pub line: Option<u32>,
    /// Severity: "error", "warning", or "info".
    pub severity: String,
    /// Concise description of the finding.
    pub message: String,
    /// Optional actionable hint.
    pub hint: Option<String>,
}

/// Operational mode for lxconf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigMode {
    /// Audit existing config content — returns findings.
    Audit,
    /// Generate a fresh config template from a description.
    Create,
    /// Apply a described change to existing config content.
    Edit,
}

/// Output of `lxconf`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// Audit mode: list of findings. Create/edit mode: empty.
    #[serde(default)]
    pub findings: Vec<Finding>,
    /// Create/edit mode: the generated or modified config content. Audit mode: empty.
    #[serde(default)]
    pub content: String,
}

impl Output {
    /// Format for plain-mode stdout, dispatched on mode.
    pub fn to_plain(&self, mode: ConfigMode) -> String {
        match mode {
            ConfigMode::Create | ConfigMode::Edit => self.content.clone(),
            ConfigMode::Audit => {
                if self.findings.is_empty() {
                    "no issues found".to_string()
                } else {
                    self.findings
                        .iter()
                        .map(|f| {
                            let loc = match f.line {
                                Some(n) => format!("line {n}"),
                                None => "file".to_string(),
                            };
                            let hint_part = match &f.hint {
                                Some(h) => format!("\n  hint: {h}"),
                                None => String::new(),
                            };
                            format!("[{}] {}: {}{}", f.severity, loc, f.message, hint_part)
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
        }
    }
}

/// Core logic for lxconf — with mandatory redaction (§8.1).
///
/// Audit mode: redacts config BEFORE sending to LLM.
/// Create/Edit mode: no redaction needed (description or intent is not sensitive).
///   For Edit mode, `input` is the intent and `existing` is the config to edit;
///   redaction is applied to `existing` before it reaches the LLM.
/// Truncate audit input to bound memory use, collecting a tier-2 warning
/// (emitted by main.rs) if truncation occurred. Pure — no I/O.
fn truncate_config(input: &str) -> (&str, Vec<String>) {
    if input.len() > MAX_CONFIG_BYTES {
        (
            &input[..MAX_CONFIG_BYTES],
            vec![format!("input truncated to {MAX_CONFIG_BYTES} bytes")],
        )
    } else {
        (input, Vec::new())
    }
}

/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(
    input: &str,
    existing: Option<&str>,
    mode: ConfigMode,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    match mode {
        ConfigMode::Audit => {
            if input.trim().is_empty() {
                return Err(LxError::BadUsage(
                    "no config content provided; use --file <PATH> or pipe config to stdin"
                        .to_string(),
                ));
            }
            let (input, warnings) = truncate_config(input);
            let level = RedactLevel::parse(&config.redact.level);
            let redacted = redact(input, level)
                .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;
            let out = send_to_llm(&redacted, None, mode, config, client)?;
            Ok((out, warnings))
        }
        ConfigMode::Create => {
            if input.trim().is_empty() {
                return Err(LxError::BadUsage("no description provided".to_string()));
            }
            let out = send_to_llm(input, None, mode, config, client)?;
            Ok((out, Vec::new()))
        }
        ConfigMode::Edit => {
            if input.trim().is_empty() {
                return Err(LxError::BadUsage(
                    "no change description provided".to_string(),
                ));
            }
            // Redact the existing config before sending to LLM.
            let existing_str = existing.unwrap_or("").trim();
            let redacted_existing = if !existing_str.is_empty() {
                let level = RedactLevel::parse(&config.redact.level);
                redact(existing_str, level)
                    .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?
            } else {
                existing_str.to_string()
            };
            let out = send_to_llm(input, Some(&redacted_existing), mode, config, client)?;
            Ok((out, Vec::new()))
        }
    }
}

/// Variant used when `--no-redact` is passed by the user.
/// Pure function: no I/O, no process::exit.
///
/// Sends the raw config content to the LLM without redaction. The caller is
/// responsible for having already warned the user prominently about the risk.
pub fn run_no_redact(
    input: &str,
    existing: Option<&str>,
    mode: ConfigMode,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }

    let (input, warnings) = if mode == ConfigMode::Audit {
        truncate_config(input)
    } else {
        (input, Vec::new())
    };

    let out = send_to_llm(input, existing, mode, config, client)?;
    Ok((out, warnings))
}

/// Build and send the LLM request, parse and validate the response.
fn send_to_llm(
    input: &str,
    existing: Option<&str>,
    mode: ConfigMode,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_msg = match mode {
        ConfigMode::Audit => input.to_string(),
        ConfigMode::Create => format!("Generate a config file for: {}", input.trim()),
        ConfigMode::Edit => {
            let content = existing.unwrap_or("").trim();
            format!(
                "Edit the following config file — apply this change ONLY: {}\n\nPreserve every other line verbatim.\n\n---\n{}",
                input.trim(),
                content
            )
        }
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

    let out = parse_response::<Output>(&resp.content)?;

    Ok(out)
}
