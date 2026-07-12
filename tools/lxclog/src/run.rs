use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// Keep-a-Changelog entries can be verbose; allow enough tokens for a full changelog.
const MAX_TOKENS: u32 = 1024;
/// Large git logs get truncated to keep LLM costs bounded.
const MAX_LOG_BYTES: usize = 48_000;

/// A single changelog entry (one version / release).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChangelogEntry {
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deprecated: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security: Vec<String>,
}

/// Output of `lxclog`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub entries: Vec<ChangelogEntry>,
}

impl Output {
    /// Render the changelog as Keep-a-Changelog markdown text.
    pub fn to_plain(&self) -> String {
        let mut out = String::from(
            "# Changelog\n\nAll notable changes to this project will be documented in this file.\n",
        );

        for entry in &self.entries {
            out.push('\n');
            if entry.date.is_empty() {
                out.push_str(&format!("## [{}]\n", entry.version));
            } else {
                out.push_str(&format!("## [{}] - {}\n", entry.version, entry.date));
            }

            fn push_section(out: &mut String, heading: &str, items: &[String]) {
                if items.is_empty() {
                    return;
                }
                out.push('\n');
                out.push_str(&format!("### {heading}\n\n"));
                for item in items {
                    out.push_str(&format!("- {item}\n"));
                }
            }

            push_section(&mut out, "Added", &entry.added);
            push_section(&mut out, "Changed", &entry.changed);
            push_section(&mut out, "Deprecated", &entry.deprecated);
            push_section(&mut out, "Removed", &entry.removed);
            push_section(&mut out, "Fixed", &entry.fixed);
            push_section(&mut out, "Security", &entry.security);
        }

        out
    }
}

/// Core logic for `lxclog` — with mandatory redaction (§8.1).
///
/// Redacts the git log BEFORE it reaches the LLM. Git logs may contain
/// secrets (tokens, API keys, passwords) in commit messages or diffs.
/// Truncate very large logs to bound memory use, collecting a tier-2 warning
/// (emitted by main.rs) if truncation occurred. Pure — no I/O.
fn truncate_log(input: &str) -> (&str, Vec<String>) {
    if input.len() > MAX_LOG_BYTES {
        (
            &input[..MAX_LOG_BYTES],
            vec![format!("git log truncated to {MAX_LOG_BYTES} bytes")],
        )
    } else {
        (input, Vec::new())
    }
}

/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no git log provided; pipe `git log --oneline` or `git log --format=...` into lxclog"
                .to_string(),
        ));
    }

    // Truncate very large logs before redaction to bound memory use.
    let (input, warnings) = truncate_log(input);

    // MANDATORY: redact before LLM. §8.1 — git logs may contain secrets.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(input, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    let out = send_to_llm(&redacted, config, client)?;
    Ok((out, warnings))
}

/// Variant used when `--no-redact` is passed by the user.
/// Pure function: no I/O, no process::exit.
///
/// Sends the raw git log to the LLM without redaction. The caller is responsible
/// for having already warned the user prominently about the risk.
pub fn run_no_redact(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no git log provided; pipe `git log --oneline` or `git log --format=...` into lxclog"
                .to_string(),
        ));
    }

    let (input, warnings) = truncate_log(input);

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

    if out.entries.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty entries list".to_string(),
        ));
    }

    Ok(out)
}
