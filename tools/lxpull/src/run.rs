use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

/// Maximum number of records returned to the user. lxpull's output scales with
/// the number of entities in the input, which is unbounded; without a cap a
/// large document could produce a JSON response that overflows `MAX_TOKENS` and
/// truncates mid-record. We keep the first `MAX_RECORDS` (the model is asked to
/// emit the most salient first) so the response always fits the budget.
const MAX_RECORDS: usize = 40;

/// A single extracted record — a map from field name to extracted value.
/// BTreeMap ensures stable key ordering for deterministic snapshots.
pub type Record = BTreeMap<String, String>;

/// Output of `lxpull`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub records: Vec<Record>,
    /// True when the record set was capped at `MAX_RECORDS`. Set locally after
    /// parsing the model response — never delegated to the LLM, hence
    /// `#[serde(default)]`.
    #[serde(default)]
    pub truncated: bool,
}

impl Output {
    /// Render as an aligned plain-text table for stdout.
    pub fn to_plain(&self, fields: &[String]) -> String {
        if self.records.is_empty() {
            return String::new();
        }

        // Compute column widths: max of header width and any value width.
        let mut widths: Vec<usize> = fields.iter().map(|f| f.len()).collect();
        for record in &self.records {
            for (i, field) in fields.iter().enumerate() {
                let val_len = record.get(field).map(|v| v.len()).unwrap_or(0);
                if val_len > widths[i] {
                    widths[i] = val_len;
                }
            }
        }

        let mut lines = Vec::new();

        // Header row.
        let header: Vec<String> = fields
            .iter()
            .enumerate()
            .map(|(i, f)| format!("{:<width$}", f, width = widths[i]))
            .collect();
        lines.push(header.join("  "));

        // Data rows.
        for record in &self.records {
            let row: Vec<String> = fields
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let val = record.get(f).map(|v| v.as_str()).unwrap_or("");
                    format!("{:<width$}", val, width = widths[i])
                })
                .collect();
            lines.push(row.join("  "));
        }

        lines.join("\n")
    }
}

/// Core logic for lxpull — with mandatory redaction (SEC: redact, untrusted).
///
/// Redacts the input BEFORE it reaches the LLM. No exceptions.
pub fn run(
    input: &str,
    fields: &[String],
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe text into lxpull or use --file".to_string(),
        ));
    }
    if fields.is_empty() {
        return Err(LxError::BadUsage(
            "no fields specified; use --fields name,email,...".to_string(),
        ));
    }

    // MANDATORY: redact before LLM. §8.1
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(input, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    send_to_llm(&redacted, fields, config, client)
}

/// Variant used when `--no-redact` is passed by the user.
pub fn run_no_redact(
    input: &str,
    fields: &[String],
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no input provided; pipe text into lxpull or use --file".to_string(),
        ));
    }
    if fields.is_empty() {
        return Err(LxError::BadUsage(
            "no fields specified; use --fields name,email,...".to_string(),
        ));
    }

    send_to_llm(input, fields, config, client)
}

/// Build and send the LLM request, parse and validate the response.
fn send_to_llm(
    user_content: &str,
    fields: &[String],
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    let fields_str = fields.join(", ");
    // Replace {fields} first, then inject {lang}.
    let system_with_fields = SYSTEM_TEMPLATE.replace("{fields}", &fields_str);
    let system = inject_lang(&system_with_fields, &config.output.lang);

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

    let mut out = parse_response::<Output>(&resp.content)?;

    // Hard cap on the record set to keep the response within the token budget
    // regardless of how many entities the input contained.
    if out.records.len() > MAX_RECORDS {
        out.records.truncate(MAX_RECORDS);
        out.truncated = true;
    }

    Ok(out)
}
