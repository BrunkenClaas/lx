#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// Keywords that mark a SQL statement as mutating.
/// Checked locally — deterministic, never delegated to the LLM (§8.3 nocmd).
static MUTATING_KEYWORDS: &[&str] = &[
    "delete",
    "drop",
    "update",
    "insert",
    "truncate",
    "alter",
    "create table",
    "create or replace",
    "merge",
    "replace into",
];

/// Output of `lxsql`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub sql: String,
    pub mutating: bool,
}

impl Output {
    /// Plain-text representation: just the SQL statement itself.
    pub fn to_plain(&self) -> String {
        self.sql.clone()
    }
}

/// Return `true` when `sql` contains at least one mutating keyword.
///
/// The check is case-insensitive and deterministic. It overrides any
/// `mutating: false` returned by the LLM.
fn is_mutating(sql: &str) -> bool {
    let lower = sql.to_lowercase();
    MUTATING_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Build the mandatory mutating-SQL warning message (§8.3 nocmd). Pure — no I/O.
///
/// Returns `None` for read-only SQL; `Some(message)` naming the specific mutating
/// keywords when any are present. main.rs emits it as a tier-3 danger warning.
pub fn mutating_warning(sql: &str) -> Option<String> {
    // Identify which specific keywords triggered the warning for clarity.
    let lower = sql.to_lowercase();
    let found: Vec<&str> = MUTATING_KEYWORDS
        .iter()
        .filter(|kw| lower.contains(*kw))
        .copied()
        .collect();

    if found.is_empty() {
        return None;
    }

    let keywords = found
        .iter()
        .map(|s| s.to_uppercase())
        .collect::<Vec<_>>()
        .join(", ");

    Some(format!(
        "⚠  WARNING: generated SQL contains mutating statement ({keywords}) — review carefully before executing"
    ))
}

/// Emit the mutating-SQL warning on stderr (tier-3: always shown, never suppressed by --quiet).
pub fn warn_mutating(warning: Option<&str>) {
    if let Some(msg) = warning {
        eprintln!("{msg}");
    }
}

/// Core logic for `lxsql`.
///
/// When `existing` is `None`, generates SQL from a natural-language `description`.
/// When `existing` is `Some(sql)`, edits the existing SQL applying only the described change.
///
/// SEC flags: nocmd — the SQL is output to stdout only; it is never executed.
/// Mutating statements (DELETE, DROP, UPDATE, INSERT, TRUNCATE, ALTER, CREATE
/// TABLE, …) are flagged on stderr with a prominent warning.
pub fn run(
    description: &str,
    schema_hint: Option<&str>,
    existing: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Option<String>), LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    // Build user message depending on mode.
    let user_message = match existing {
        Some(sql) if !sql.trim().is_empty() => format!(
            "Edit the following SQL — apply this change ONLY: {}\n\nPreserve every other part verbatim.\n\n---\n{}",
            description.trim(),
            sql.trim()
        ),
        _ => match schema_hint {
            Some(schema) if !schema.trim().is_empty() => {
                format!(
                    "Description: {}\n\nSchema:\n{}",
                    description.trim(),
                    schema.trim()
                )
            }
            _ => description.trim().to_string(),
        },
    };

    let req = Request {
        system: &system,
        user: &user_message,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.sql.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty SQL".to_string(),
        ));
    }

    // Local mutating detection — deterministic override (§8.3).
    // If our pattern check says mutating, we set the flag regardless of what
    // the LLM returned.
    let locally_mutating = is_mutating(&out.sql);
    if locally_mutating {
        out.mutating = true;
    }

    // Build the mandatory mutating-SQL warning (§8.3 nocmd). Emission is main.rs's
    // job (tier-3 stderr); run() stays pure. Only emit when the flag is set.
    let warning = if out.mutating {
        mutating_warning(&out.sql)
    } else {
        None
    };

    Ok((out, warning))
}
