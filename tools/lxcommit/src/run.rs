use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;
/// Diffs rarely need more than this; large diffs get truncated upstream.
const MAX_DIFF_BYTES: usize = 32_000;

/// Output of `lxcommit`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommitOutput {
    #[serde(rename = "type")]
    pub commit_type: String,
    pub scope: String,
    pub subject: String,
    pub body: String,
}

impl CommitOutput {
    /// Format as a ready-to-use commit message string.
    pub fn to_plain(&self) -> String {
        let header = if self.scope.is_empty() {
            format!("{}: {}", self.commit_type, self.subject)
        } else {
            format!("{}({}): {}", self.commit_type, self.scope, self.subject)
        };
        if self.body.is_empty() {
            header
        } else {
            format!("{}\n\n{}", header, self.body)
        }
    }
}

/// Truncate very large diffs to bound memory use, collecting a tier-2 warning
/// (emitted by main.rs) if truncation occurred. Pure — no I/O.
fn truncate_diff(diff: &str) -> (&str, Vec<String>) {
    if diff.len() > MAX_DIFF_BYTES {
        (
            &diff[..MAX_DIFF_BYTES],
            vec![format!("diff truncated to {MAX_DIFF_BYTES} bytes")],
        )
    } else {
        (diff, Vec::new())
    }
}

/// Core logic for lxcommit — with mandatory redaction (§8.1).
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// Redacts the diff BEFORE it reaches the LLM. No exceptions.
/// Secrets in diffs are a frequent, real-world occurrence.
/// Returns the commit message plus any tier-2 warnings for main.rs to emit.
pub fn run(
    diff: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(CommitOutput, Vec<String>), LxError> {
    if diff.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no diff provided; pipe `git diff --staged` into lxcommit".to_string(),
        ));
    }

    let (diff, warnings) = truncate_diff(diff);

    // MANDATORY: redact before LLM. §8.1 — diffs frequently contain secrets.
    // If redaction fails (e.g. would remove >80% of content) → Exit 5.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(diff, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    let out = send_to_llm(&redacted, config, client)?;
    Ok((out, warnings))
}

/// Variant used when `--no-redact` is passed by the user.
/// Pure function: no I/O, no process::exit.
///
/// Sends the raw diff to the LLM without redaction. The caller is responsible
/// for having already warned the user prominently about the risk.
pub fn run_no_redact(
    diff: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(CommitOutput, Vec<String>), LxError> {
    if diff.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no diff provided; pipe `git diff --staged` into lxcommit".to_string(),
        ));
    }

    let (diff, warnings) = truncate_diff(diff);

    let out = send_to_llm(diff, config, client)?;
    Ok((out, warnings))
}

/// Build and send the LLM request, parse and validate the response.
fn send_to_llm(
    user_content: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<CommitOutput, LxError> {
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

    let out = parse_response::<CommitOutput>(&resp.content)?;

    if out.subject.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty commit subject".to_string(),
        ));
    }

    Ok(out)
}
