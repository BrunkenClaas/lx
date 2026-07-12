use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Dangerous jq/shell-injection patterns that must be flagged on stderr (§8.3 nocmd).
///
/// jq expressions are normally safe, but crafted expressions can be used
/// as input to shell expansions (e.g. `jq -r '.x' | sh`). We warn on patterns
/// that suggest shell injection or dangerous data manipulation.
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (
        "@sh",
        "shell string escaping — output may be used in shell eval",
    ),
    (
        "input",
        "reads additional JSON input — ensure the source is trusted",
    ),
    (
        "env",
        "accesses environment variables — may leak sensitive data",
    ),
    (
        "path(",
        "path expression — verify it does not escape expected structure",
    ),
    ("$ENV", "accesses environment object — may expose secrets"),
    ("halt", "terminates jq — unexpected exit behaviour"),
    ("debug", "emits debug output to stderr — may leak data"),
];

/// Output of `lxjq`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub expression: String,
    pub explanation: String,
    /// True if a dangerous pattern was detected locally (not set by the LLM).
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    /// Plain-text representation: expression only (pipe-safe).
    /// Explanation goes to stderr via main.rs.
    pub fn to_plain(&self) -> String {
        self.expression.clone()
    }
}

/// A dangerous pattern found in a jq expression: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan `expression` for dangerous jq patterns. Pure — no I/O.
///
/// Case-sensitive (jq syntax is). Returns every matched pattern;
/// main.rs emits them as tier-3 danger warnings.
pub fn check(expression: &str) -> Vec<Finding> {
    DANGEROUS_PATTERNS
        .iter()
        .filter(|(pattern, _)| expression.contains(pattern))
        .map(|(pattern, description)| Finding {
            pattern,
            description,
        })
        .collect()
}

/// Emit danger findings on stderr (tier-3: always shown, never suppressed by --quiet).
pub fn warn_findings(findings: &[Finding]) {
    for f in findings {
        eprintln!(
            "warning: jq expression contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("         Review the expression before use. It was NOT executed.");
    }
}

/// Core logic for `lxjq`.
///
/// When `existing` is `None`, generates a jq expression from a natural-language description.
/// When `existing` is `Some(expr)`, edits that expression applying only the described change.
/// An optional `json_context` string (raw JSON) is appended to the user message in create mode.
///
/// NEVER executes the generated expression (§8.3 nocmd).
pub fn run(
    description: &str,
    json_context: Option<&str>,
    existing: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<Finding>), LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_message = match existing {
        Some(expr) if !expr.trim().is_empty() => format!(
            "Edit the following jq expression — apply this change ONLY: {}\n\nPreserve every other part verbatim.\n\n---\n{}",
            description.trim(),
            expr.trim()
        ),
        _ => match json_context {
            Some(ctx) if !ctx.trim().is_empty() => {
                format!(
                    "{}\n\nJSON context (shape of the input):\n{}",
                    description.trim(),
                    ctx.trim()
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

    let resp = client.complete(&req).map_err(LxError::from)?;

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.expression.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty expression".to_string(),
        ));
    }

    // Local danger detection — deterministic, not delegated to the LLM (§8.3).
    // Emission is main.rs's job (tier-3 stderr); run() stays pure.
    let findings = check(&out.expression);
    if !findings.is_empty() {
        out.dangerous = true;
    }

    Ok((out, findings))
}
