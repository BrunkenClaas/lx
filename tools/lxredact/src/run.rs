#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::RedactLevel;
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
pub const ANON_SYSTEM_TEMPLATE: &str = include_str!("../prompts/anon_system.txt");
/// Max tokens for the optional LLM explain/anon call.
const MAX_TOKENS: u32 = 512;
const ANON_MAX_TOKENS: u32 = 2048;

/// A single redacted item (location only — never the value).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedItem {
    /// Category of secret/PII detected (e.g. "api_key", "email", "private_key").
    pub kind: String,
    /// Human-readable location hint (e.g. "line 5").
    pub location: String,
}

/// Output of `lxredact`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The redacted text (secrets replaced with placeholders).
    pub redacted_text: String,
    /// Number of replacements made.
    pub redacted_count: usize,
    /// Individual redacted items (type + location, never values).
    pub items: Vec<RedactedItem>,
    /// Optional LLM-generated explanation (only when `--explain` is used).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explanation: Option<ExplainOutput>,
    /// Optional anonymised text with PII replaced by role placeholders (only when `--anon` is used).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anon: Option<AnonOutput>,
}

/// Anonymisation output (used only when `--anon` flag is set).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonOutput {
    /// Full text with PII replaced by role placeholders.
    pub text: String,
    /// List of replacements made (original value → placeholder).
    pub replacements: Vec<AnonReplacement>,
}

/// One PII replacement entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonReplacement {
    pub original: String,
    pub replacement: String,
}

impl Output {
    /// Return just the redacted text for plain-mode stdout.
    pub fn to_plain(&self) -> &str {
        &self.redacted_text
    }
}

/// LLM explanation output (used only when `--explain` flag is set).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainOutput {
    pub summary: String,
    pub categories: Vec<String>,
    pub risk_level: String,
    pub notes: String,
}

/// Core logic for `lxredact`.
///
/// Pure function: no I/O, no `process::exit`. Testable with `MockLlmClient`.
///
/// # Arguments
/// * `input`   — raw text to redact
/// * `level`   — redaction depth (Standard or Strict)
/// * `explain` — if `true`, call the LLM to explain what was redacted (never values)
/// * `anon`    — if `true`, call the LLM to anonymise PII with role placeholders
/// * `config`  — loaded tool configuration
/// * `client`  — injected LLM client (MockLlmClient in tests)
pub fn run(
    input: &str,
    level: RedactLevel,
    explain: bool,
    anon: bool,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.is_empty() {
        return Ok(Output {
            redacted_text: String::new(),
            redacted_count: 0,
            items: Vec::new(),
            explanation: None,
            anon: None,
        });
    }

    // Step 1: local deterministic redaction — never touches the network.
    let redacted = lx_redact::redact(input, level)?;

    // Step 2: count and locate redacted items by diffing original vs. redacted.
    let items = locate_redacted_items(input, &redacted);
    let redacted_count = items.len();

    // Step 3: optional LLM call for explanation (never sends the actual values).
    let explanation = if explain && redacted_count > 0 {
        let report = build_explain_prompt(&items);
        let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
        let req = Request {
            system: &system,
            user: &report,
            max_tokens: MAX_TOKENS,
            temperature: 0.0,
            image: None,
        };
        let resp = client.complete(&req).map_err(LxError::from)?;
        let ex: ExplainOutput = parse_response(&resp.content)?;
        Some(ex)
    } else {
        None
    };

    // Step 4: optional LLM call for anonymisation (sends redacted text, not raw).
    let anon_output = if anon {
        let system = inject_lang(ANON_SYSTEM_TEMPLATE, &config.output.lang);
        let req = Request {
            system: &system,
            user: &redacted,
            max_tokens: ANON_MAX_TOKENS,
            temperature: 0.0,
            image: None,
        };
        let resp = client.complete(&req).map_err(LxError::from)?;
        let out: AnonOutput = parse_response(&resp.content)?;
        Some(out)
    } else {
        None
    };

    Ok(Output {
        redacted_text: redacted,
        redacted_count,
        items,
        explanation,
        anon: anon_output,
    })
}

/// Build the user message sent to the LLM for the `--explain` mode.
///
/// Only includes type and location — never the actual secret values.
fn build_explain_prompt(items: &[RedactedItem]) -> String {
    let mut parts = Vec::with_capacity(items.len());
    for item in items {
        parts.push(format!("{} at {}", item.kind, item.location));
    }
    format!(
        "{} item{} redacted: {}",
        items.len(),
        if items.len() == 1 { "" } else { "s" },
        parts.join(", ")
    )
}

/// Derive a list of `RedactedItem` by comparing original and redacted text line by line.
///
/// For each line that changed, we identify what placeholder was inserted and record
/// the line number. This is purely local — no values are ever revealed.
fn locate_redacted_items(original: &str, redacted: &str) -> Vec<RedactedItem> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let red_lines: Vec<&str> = redacted.lines().collect();

    let mut items = Vec::new();

    let max_lines = orig_lines.len().max(red_lines.len());
    for i in 0..max_lines {
        let orig_line = orig_lines.get(i).copied().unwrap_or("");
        let red_line = red_lines.get(i).copied().unwrap_or("");

        if orig_line == red_line {
            continue;
        }

        // Count each placeholder type inserted on this line.
        for &(placeholder, kind) in PLACEHOLDER_KINDS {
            let orig_count = count_occurrences(orig_line, placeholder);
            let red_count = count_occurrences(red_line, placeholder);
            if red_count > orig_count {
                let n = red_count - orig_count;
                for _ in 0..n {
                    items.push(RedactedItem {
                        kind: kind.to_string(),
                        location: format!("line {}", i + 1),
                    });
                }
            }
        }
    }

    items
}

/// Maps placeholder strings to human-readable kind names.
const PLACEHOLDER_KINDS: &[(&str, &str)] = &[
    ("[REDACTED]", "secret"),
    ("[EMAIL]", "email"),
    ("[IP]", "ip_address"),
    ("[HOST]", "hostname"),
    ("[PATH]", "local_path"),
];

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        count += 1;
        start += pos + needle.len();
    }
    count
}
