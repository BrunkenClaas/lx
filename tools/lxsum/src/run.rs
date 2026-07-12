use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 768;
/// Bound memory use — very long texts get truncated by the caller.
const MAX_INPUT_BYTES: usize = 32_000;

/// Output format for `lxsum`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SumFormat {
    #[default]
    Bullets,
    Prose,
    Outline,
}

impl SumFormat {
    /// Parse from CLI string; returns None on invalid input.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "bullets" => Some(SumFormat::Bullets),
            "prose" => Some(SumFormat::Prose),
            "outline" => Some(SumFormat::Outline),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            SumFormat::Bullets => "bullets",
            SumFormat::Prose => "prose",
            SumFormat::Outline => "outline",
        }
    }
}

/// Options controlling how lxsum generates its output.
#[derive(Debug, Clone, Default)]
pub struct SumOptions {
    /// Only produce the one-sentence tldr, skip the bullets/body.
    pub short: bool,
    /// Produce a short title/subject line instead of a summary (absorbs lxheadline).
    pub headline: bool,
    /// Approximate word limit for the summary body.
    pub max_words: Option<u32>,
    /// Approximate line limit for the summary body.
    pub max_lines: Option<u32>,
    /// Output format: prose | bullets (default) | outline.
    pub format: SumFormat,
}

/// Output of `lxsum`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub tldr: String,
    /// Bullet/outline points or prose paragraph(s). Empty when `--short` is used.
    #[serde(default)]
    pub bullets: Vec<String>,
    /// Prose body (used when format=prose or format=outline with nested items).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl Output {
    /// Bare headline/subject line for `--headline` mode.
    ///
    /// Returns only the `tldr` with no `Summary:` prefix and no bullets, so the
    /// output can be piped straight into a git commit subject, email subject, or
    /// document title.
    pub fn to_headline(&self) -> String {
        self.tldr.clone()
    }

    /// Format for human-readable terminal output.
    pub fn to_plain(&self) -> String {
        let mut out = format!("Summary: {}", self.tldr);
        // Body (prose/outline) takes precedence over bullets.
        if let Some(body) = &self.body {
            if !body.is_empty() {
                out.push_str(&format!("\n\n{body}"));
                return out;
            }
        }
        if !self.bullets.is_empty() {
            out.push('\n');
            for b in &self.bullets {
                out.push_str(&format!("\n  • {b}"));
            }
        }
        out
    }
}

/// Core logic for lxsum — with mandatory redaction (§8.1) and untrusted-input
/// isolation (§8.2).
///
/// Redacts the input BEFORE it reaches the LLM. No exceptions.
/// User-provided text may contain secrets, PII, or injected instructions.
#[allow(dead_code)]
pub fn run(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    run_with_opts(input, config, client, &SumOptions::default())
}

/// Variant used when `--no-redact` is passed by the user.
#[allow(dead_code)]
pub fn run_no_redact(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    run_no_redact_with_opts(input, config, client, &SumOptions::default())
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

/// Core logic with hub flags support (redaction enabled).
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run_with_opts(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
    opts: &SumOptions,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe text or a file into lxsum".to_string(),
        ));
    }

    let (input, warnings) = truncate_input(input);

    // MANDATORY: redact before LLM. §8.1 — documents and logs frequently
    // contain API keys, tokens, PII, or credentials.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(input, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    let out = send_to_llm(&redacted, config, client, opts)?;
    Ok((out, warnings))
}

/// Variant used when `--no-redact` is passed by the user, with hub flags.
/// Pure function: no I/O, no process::exit.
pub fn run_no_redact_with_opts(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
    opts: &SumOptions,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe text or a file into lxsum".to_string(),
        ));
    }

    let (input, warnings) = truncate_input(input);

    let out = send_to_llm(input, config, client, opts)?;
    Ok((out, warnings))
}

/// Build the constraint string to inject into the system prompt.
fn build_constraints(opts: &SumOptions) -> String {
    let mut parts: Vec<String> = Vec::new();

    if opts.headline {
        parts.push(
            "Produce ONLY a short, punchy title or subject line (5-10 words) in the tldr field. \
             This will be used as an email subject, git commit subject, or document title. \
             Set bullets to [] and omit body."
                .to_string(),
        );
    } else if opts.short {
        parts.push("Produce ONLY the tldr field. Set bullets to [] and omit body.".to_string());
    }

    match opts.format {
        SumFormat::Prose => {
            parts.push(
                "Format: write the body as a single flowing prose paragraph. \
                 Set bullets to []."
                    .to_string(),
            );
        }
        SumFormat::Outline => {
            parts.push(
                "Format: write bullets as an outline with top-level items starting with \
                 a roman numeral (I., II., …) and sub-items indented with two spaces and \
                 a dash."
                    .to_string(),
            );
        }
        SumFormat::Bullets => {} // default — no extra instruction
    }

    if let Some(w) = opts.max_words {
        parts.push(format!(
            "Keep the combined body to roughly {w} words or fewer."
        ));
    }
    if let Some(l) = opts.max_lines {
        parts.push(format!(
            "Limit the number of bullet/outline items (or prose lines) to {l} or fewer."
        ));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("\n\nAdditional constraints:\n{}", parts.join("\n"))
    }
}

/// Build and send the LLM request, parse and validate the response.
fn send_to_llm(
    user_content: &str,
    config: &Config,
    client: &dyn LlmClient,
    opts: &SumOptions,
) -> Result<Output, LxError> {
    let constraints = build_constraints(opts);
    let template_with_constraints = format!("{}{}", SYSTEM_TEMPLATE, constraints);
    let system = inject_lang(&template_with_constraints, &config.output.lang);

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

    if out.tldr.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty tldr".to_string(),
        ));
    }

    // When --short or --headline, bullets being empty is expected (both modes
    // instruct the model to emit only the tldr and set bullets to []).
    if !opts.short && !opts.headline && out.bullets.is_empty() && out.body.is_none() {
        return Err(LxError::LogicalError(
            "model returned empty bullets list".to_string(),
        ));
    }

    Ok(out)
}
