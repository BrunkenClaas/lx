use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;
const MAX_INPUT_BYTES: usize = 32_000;

/// Output of `lxdebug`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub cause: String,
    pub fix: String,
    pub command: String,
}

impl Output {
    /// Plain-text output: fix + optional run command (pipe-safe).
    /// Cause goes to stderr as context via main.rs.
    pub fn to_plain(&self) -> String {
        let mut out = format!("Fix:    {}", self.fix);
        if !self.command.is_empty() {
            out.push_str(&format!("\n\nRun:    {}", self.command));
        }
        out
    }
}

/// Truncate very large input to bound memory use, collecting a tier-2 warning
/// (emitted by main.rs) if truncation occurred. Pure — no I/O.
fn truncate_input(input: &str) -> (&str, Vec<String>) {
    if input.len() > MAX_INPUT_BYTES {
        (
            &input[..MAX_INPUT_BYTES],
            vec![format!("input truncated to {MAX_INPUT_BYTES} bytes")],
        )
    } else {
        (input, Vec::new())
    }
}

/// Core logic for lxdebug — with mandatory redaction (§8.1) and untrusted-input
/// isolation (§8.2). The suggested command is NEVER executed (§8.3).
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no error output provided; pipe stderr into lxdebug".to_string(),
        ));
    }

    let (input, warnings) = truncate_input(input);

    // MANDATORY: redact before LLM. §8.1 — error logs frequently contain secrets,
    // API keys, tokens, or PII embedded in tracebacks and environment dumps.
    // If redaction fails (e.g. would remove >80% of content) → Exit 5.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(input, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    let out = send_to_llm(&redacted, config, client)?;
    Ok((out, warnings))
}

/// Variant used when `--no-redact` is passed.
/// Pure function: no I/O, no process::exit.
///
/// Sends raw error output to the LLM without redaction. Caller is responsible
/// for having warned the user prominently about the risk.
pub fn run_no_redact(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no error output provided; pipe stderr into lxdebug".to_string(),
        ));
    }

    let (input, warnings) = truncate_input(input);

    let out = send_to_llm(input, config, client)?;
    Ok((out, warnings))
}

/// Build and send the LLM request, parse and validate the response.
///
/// System prompt is static and trusted (embedded at compile time).
/// User content is the (already-redacted) error output — untrusted data.
/// The system prompt instructs the model to ignore embedded instructions (§8.2).
fn send_to_llm(
    user_content: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: user_content,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let out = parse_response::<Output>(&resp.content)?;

    if out.cause.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty cause".to_string(),
        ));
    }
    if out.fix.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty fix".to_string(),
        ));
    }

    Ok(out)
}
