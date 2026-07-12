use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;
/// Large diffs are truncated before reaching the LLM to keep latency and cost bounded.
const MAX_DIFF_BYTES: usize = 32_000;

/// Output of `lxdiff`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub summary: String,
    pub changes: Vec<String>,
}

impl Output {
    /// Format as human-readable plain text.
    pub fn to_plain(&self) -> String {
        let mut out = format!("{}\n", self.summary);
        for change in &self.changes {
            out.push_str(&format!("  - {change}\n"));
        }
        out.trim_end().to_string()
    }
}

/// Core logic for `lxdiff` — with mandatory redaction (§8.1).
///
/// Redacts the diff BEFORE it reaches the LLM. Diffs frequently contain
/// secrets committed by mistake; we never send them unmasked.
pub fn run(
    diff: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if diff.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no diff provided; pipe `git diff` or a patch file into lxdiff".to_string(),
        ));
    }

    let (diff, warnings) = truncate_diff(diff);

    // MANDATORY: redact before LLM. §8.1 — diffs frequently contain secrets.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(diff, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    let out = send_to_llm(&redacted, config, client)?;
    Ok((out, warnings))
}

/// Variant used when `--no-redact` is passed by the user.
///
/// Sends the raw diff to the LLM without redaction. The caller is responsible
/// for having already warned the user prominently about the risk.
pub fn run_no_redact(
    diff: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if diff.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no diff provided; pipe `git diff` or a patch file into lxdiff".to_string(),
        ));
    }

    let (diff, warnings) = truncate_diff(diff);
    let out = send_to_llm(diff, config, client)?;
    Ok((out, warnings))
}

/// Truncate the diff to `MAX_DIFF_BYTES`, collecting a tier-2 warning (emitted by
/// main.rs) if truncation occurred. Pure — no I/O.
fn truncate_diff(diff: &str) -> (&str, Vec<String>) {
    if diff.len() > MAX_DIFF_BYTES {
        (
            &diff[..MAX_DIFF_BYTES],
            vec![format!(
                "diff truncated to {} bytes (original: {} bytes)",
                MAX_DIFF_BYTES,
                diff.len()
            )],
        )
    } else {
        (diff, Vec::new())
    }
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

    if out.summary.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty summary".to_string(),
        ));
    }

    Ok(out)
}
