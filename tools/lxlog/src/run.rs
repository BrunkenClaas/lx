use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use lx_redact::{redact, RedactLevel};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// Worst-case: ~40 anomalies × 50 chars + summary ≈ 600 tokens; 2048 gives headroom for noisy logs.
const MAX_TOKENS: u32 = 2048;
/// Maximum log lines forwarded to the LLM after local aggregation.
/// Covers typical single-service daily logs (samba, nginx, sshd) completely.
/// Only fires on multi-day rotations or verbose debug dumps.
const MAX_SAMPLE_LINES: usize = 500;

// ── Output types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Anomaly {
    pub line: Option<u32>,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub anomalies: Vec<Anomaly>,
    pub summary: String,
    /// Set locally after aggregation — not expected in the LLM response.
    #[serde(default)]
    pub used_lines: String,
    /// True when the log exceeded MAX_SAMPLE_LINES and some lines were not sent to the LLM.
    #[serde(default)]
    pub capped: bool,
}

impl Output {
    /// Render as plain text suitable for stdout in plain mode.
    pub fn to_plain(&self) -> String {
        if self.anomalies.is_empty() {
            return self.summary.to_string();
        }
        let mut lines: Vec<String> = self
            .anomalies
            .iter()
            .map(|a| match a.line {
                Some(n) => format!("[{}] line {}: {}", a.level, n, a.message),
                None => format!("[{}]: {}", a.level, a.message),
            })
            .collect();
        lines.push(String::new());
        lines.push(self.summary.clone());
        lines.join("\n")
    }
}

// ── Local aggregation ─────────────────────────────────────────────────────────

/// Patterns that indicate issues — covers standard log levels plus common daemon signals
/// (samba NT_STATUS_*, systemd failed/denied, nginx timeout/refused, ssh invalid/rejected).
const PRIORITY_KEYWORDS: &[&str] = &[
    "FATAL",
    "ERROR",
    "WARN",
    "WARNING",
    "CRITICAL",
    "EXCEPTION",
    "PANIC",
    "FAILED",
    "DENIED",
    "REFUSED",
    "TIMEOUT",
    "INVALID",
    "FAULT",
    "ABORT",
    "REJECTED",
    "PERMISSION",
    "NT_STATUS_",
];

/// Aggregate/sample log content before sending to LLM.
///
/// Strategy:
/// 1. Extract all ERROR/WARN/FATAL lines first (up to MAX_SAMPLE_LINES).
/// 2. Deduplicate repeated identical lines (keep first occurrence + count).
/// 3. If fewer than MAX_SAMPLE_LINES after step 1, fill remaining slots with
///    evenly-sampled lines from the full log to give the LLM context.
///
/// Returns `(aggregated_text, used_lines_description, capped)`.
/// `capped` is true when the log exceeded MAX_SAMPLE_LINES and some lines were not analysed.
pub fn aggregate_logs(input: &str) -> (String, String, bool) {
    let all_lines: Vec<&str> = input.lines().collect();
    let total = all_lines.len();

    if total == 0 {
        return (String::new(), String::new(), false);
    }

    // Step 1: collect high-priority lines (ERROR/WARN/FATAL etc.)
    let mut priority: Vec<(usize, &str)> = Vec::new();
    for (idx, line) in all_lines.iter().enumerate() {
        let upper = line.to_uppercase();
        if PRIORITY_KEYWORDS.iter().any(|kw| upper.contains(kw)) {
            priority.push((idx, line));
        }
    }

    // Step 2: deduplicate priority lines (keep first + append count note)
    let mut seen: std::collections::BTreeMap<&str, (usize, usize)> =
        std::collections::BTreeMap::new();
    for (idx, line) in &priority {
        let entry = seen.entry(line).or_insert((*idx, 0));
        entry.1 += 1;
    }
    // Sort by line number (BTreeMap gives alphabetical, re-sort numerically)
    // Rebuild with original order preserved via the index
    let mut ordered: Vec<(usize, String)> = seen
        .iter()
        .map(|(line, (idx, count))| {
            let s = if *count > 1 {
                format!("[line {}] {} (x{})", idx + 1, line, count)
            } else {
                format!("[line {}] {}", idx + 1, line)
            };
            (*idx, s)
        })
        .collect();
    ordered.sort_by_key(|(idx, _)| *idx);
    let deduped_priority: Vec<String> = ordered.into_iter().map(|(_, s)| s).collect();

    let mut result: Vec<String> = Vec::new();
    let used_lines: String;

    let capped;

    if deduped_priority.len() >= MAX_SAMPLE_LINES {
        // Too many priority lines — cap with a note.
        result.push(format!(
            "# Log excerpt: {} total lines, showing first {} high-priority lines",
            total, MAX_SAMPLE_LINES
        ));
        result.extend(deduped_priority.into_iter().take(MAX_SAMPLE_LINES));
        used_lines = format!(
            "first {} of >={} high-priority lines ({} total)",
            MAX_SAMPLE_LINES, MAX_SAMPLE_LINES, total
        );
        capped = true;
    } else {
        // Fill remaining slots with sampled context lines.
        let remaining = MAX_SAMPLE_LINES.saturating_sub(deduped_priority.len());

        // Sample evenly from the full log for context.
        let step = if remaining > 0 && total > remaining {
            total / remaining
        } else {
            1
        };

        let context: Vec<String> = all_lines
            .iter()
            .enumerate()
            .filter(|(idx, line)| {
                // Skip lines already in priority set
                let upper = line.to_uppercase();
                !PRIORITY_KEYWORDS.iter().any(|kw| upper.contains(kw))
                    && (step <= 1 || idx % step == 0)
            })
            .take(remaining)
            .map(|(idx, line)| format!("[line {}] {}", idx + 1, line))
            .collect();

        let priority_count = deduped_priority.len();
        let context_count = context.len();
        result.push(format!(
            "# Log excerpt: {} total lines, {} high-priority, {} context samples",
            total, priority_count, context_count
        ));
        result.extend(deduped_priority);
        if !context.is_empty() {
            result.push("# Context samples:".to_string());
            result.extend(context);
        }
        used_lines = format!(
            "{} high-priority + {} context samples ({} total lines)",
            priority_count, context_count, total
        );
        capped = total > MAX_SAMPLE_LINES;
    }

    (result.join("\n"), used_lines, capped)
}

// ── Core logic ────────────────────────────────────────────────────────────────

/// Core logic for `lxlog` — with mandatory redaction (§8.1).
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no log content provided; pipe a log file or use --file <path>".to_string(),
        ));
    }

    // MANDATORY: redact before LLM — logs frequently contain credentials and PII.
    let level = RedactLevel::parse(&config.redact.level);
    let redacted = redact(input, level)
        .map_err(|e| LxError::SecurityAbort(format!("redaction failed: {e}")))?;

    run_inner(&redacted, config, client)
}

/// Variant used when `--no-redact` is passed by the user.
pub fn run_no_redact(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no log content provided; pipe a log file or use --file <path>".to_string(),
        ));
    }
    run_inner(input, config, client)
}

fn run_inner(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    // Local aggregation: sample/deduplicate before sending to LLM.
    let (aggregated, used_lines, capped) = aggregate_logs(input);

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: &aggregated,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let mut out = parse_response::<Output>(&resp.content)?;
    out.used_lines = used_lines;
    out.capped = capped;

    Ok(out)
}
