use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

/// Dangerous patterns that could appear as embedded commands in generated diagrams.
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("| sh", "pipe to shell — possible command injection"),
    ("|sh", "pipe to shell — possible command injection"),
    ("| bash", "pipe to bash — possible command injection"),
    ("|bash", "pipe to bash — possible command injection"),
    ("| iex", "pipe to PowerShell Invoke-Expression"),
    ("|iex", "pipe to PowerShell Invoke-Expression"),
    ("curl|sh", "executing untrusted remote script"),
    ("wget|sh", "executing untrusted remote script"),
    ("rm -rf /", "recursive deletion of root filesystem"),
    ("drop table", "destructive SQL — permanently deletes table"),
    (
        "delete from",
        "SQL deletion — may remove all rows if no WHERE clause",
    ),
];

/// A dangerous pattern found in a diagram: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan a generated diagram for embedded dangerous patterns. Pure — no I/O.
///
/// Returns every matched pattern; main.rs emits them as tier-3 danger warnings.
pub fn check_diagram_danger(diagram: &str) -> Vec<Finding> {
    let lower = diagram.to_lowercase();
    DANGEROUS_PATTERNS
        .iter()
        .filter(|(pattern, _)| lower.contains(&pattern.to_lowercase()))
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
            "DANGER: diagram contains '{}' — {}",
            f.pattern, f.description
        );
    }
}

/// Output of `lxmermaid`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub diagram: String,
}

impl Output {
    pub fn to_plain(&self) -> String {
        self.diagram.clone()
    }
}

/// Core logic for lxmermaid.
///
/// When `existing` is `None`, generates a Mermaid diagram from the description.
/// When `existing` is `Some(diagram)`, edits that diagram applying only the described change.
/// Never executes any generated content (nocmd).
pub fn run(
    input: &str,
    existing: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<Finding>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_msg = match existing {
        Some(diagram) if !diagram.trim().is_empty() => format!(
            "Edit the following Mermaid diagram — apply this change ONLY: {}\n\nPreserve every other line verbatim.\n\n---\n{}",
            input.trim(),
            diagram.trim()
        ),
        _ => input.trim().to_string(),
    };

    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let out = parse_response::<Output>(&resp.content)?;

    if out.diagram.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty diagram".to_string(),
        ));
    }

    // Local danger scan — deterministic. Emission is main.rs's job (tier-3
    // stderr, never suppressed); run() stays pure.
    let findings = check_diagram_danger(&out.diagram);

    Ok((out, findings))
}
