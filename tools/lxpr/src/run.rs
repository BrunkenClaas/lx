use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// PR title + markdown body comfortably fit within this limit.
const MAX_TOKENS: u32 = 1024;
/// Maximum input bytes before truncation.
const MAX_INPUT_BYTES: usize = 64_000;

/// Output of `lxpr`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub title: String,
    pub body: String,
}

impl Output {
    /// Format the PR text for plain stdout.
    ///
    /// Returns `title\n\nbody` — the full PR text is the result
    /// (analogous to how lxexplain's explanation is its result).
    pub fn to_plain(&self) -> String {
        format!("{}\n\n{}", self.title, self.body)
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

/// Core logic for `lxpr` — with mandatory redaction (SEC: redact, untrusted).
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// Redacts the diff/commit log BEFORE it reaches the LLM. Diffs frequently
/// contain secrets (API keys, tokens, connection strings).
pub fn run(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe `git diff HEAD~1` or `git log -p` into lxpr".to_string(),
        ));
    }

    // Truncate very large diffs before redaction to bound memory use.
    let (input, warnings) = truncate_input(input);

    // MANDATORY: redact before LLM. §8.1 — diffs frequently contain secrets.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(input, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    let out = send_to_llm(&redacted, config, client)?;
    Ok((out, warnings))
}

/// Variant used when `--no-redact` is passed by the user.
/// Pure function: no I/O, no process::exit.
///
/// Sends the raw input to the LLM without redaction. The caller is responsible
/// for having already warned the user prominently about the risk.
pub fn run_no_redact(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe `git diff HEAD~1` or `git log -p` into lxpr".to_string(),
        ));
    }

    let (input, warnings) = truncate_input(input);

    let out = send_to_llm(input, config, client)?;
    Ok((out, warnings))
}

/// Build and send the LLM request, parse and validate the response.
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

    if out.title.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty PR title".to_string(),
        ));
    }
    if out.body.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty PR body".to_string(),
        ));
    }

    Ok(out)
}
