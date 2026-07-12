use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
// Restates commits/notes into done/next/blockers (~1:1, no input cap). A
// sprint-length `git log | lxstandup` exceeded 512 and truncated. 1024 covers
// realistic multi-day activity.
const MAX_TOKENS: u32 = 1024;

/// Output of `lxstandup`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub done: Vec<String>,
    pub next: Vec<String>,
    pub blockers: Vec<String>,
}

impl Output {
    /// Format as a standup-style plain text output for stdout.
    pub fn to_plain(&self) -> String {
        let mut out = String::new();

        out.push_str("Done:\n");
        if self.done.is_empty() {
            out.push_str("(none)\n");
        } else {
            for item in &self.done {
                out.push_str(&format!("- {}\n", item));
            }
        }

        out.push('\n');
        out.push_str("Next:\n");
        if self.next.is_empty() {
            out.push_str("(none)\n");
        } else {
            for item in &self.next {
                out.push_str(&format!("- {}\n", item));
            }
        }

        out.push('\n');
        out.push_str("Blockers:\n");
        if self.blockers.is_empty() {
            out.push_str("(none)\n");
        } else {
            for item in &self.blockers {
                out.push_str(&format!("- {}\n", item));
            }
        }

        out.trim_end().to_string()
    }
}

/// Core logic for lxstandup — with mandatory redaction (SEC: redact).
///
/// Redacts the input BEFORE it reaches the LLM. Git logs may contain secrets
/// in commit messages or environment variable assignments.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe git log output or work notes into lxstandup".to_string(),
        ));
    }

    // MANDATORY: redact before LLM. Git logs frequently contain secrets.
    // If redaction fails → Exit 5.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(input, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    send_to_llm(&redacted, config, client)
}

/// Variant used when `--no-redact` is passed by the user.
///
/// Sends the raw input to the LLM without redaction. The caller is responsible
/// for having already warned the user prominently about the risk.
pub fn run_no_redact(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe git log output or work notes into lxstandup".to_string(),
        ));
    }

    send_to_llm(input, config, client)
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

    Ok(out)
}
