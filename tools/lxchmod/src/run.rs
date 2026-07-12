use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Dangerous chmod-related patterns that should never be suggested.
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (
        "chmod 777",
        "world-writable and world-executable — grants full access to all users",
    ),
    (
        "chmod a+w",
        "adds world-writable bit — all users can modify the file",
    ),
    ("chmod o+w", "adds other-write bit — unsafe for most files"),
    ("chmod +s", "setuid/setgid bit — can escalate privileges"),
    (
        "chmod 666",
        "world-writable (no execute) — still unsafe for sensitive files",
    ),
    (
        "chmod 4755",
        "setuid with execute — privilege escalation risk",
    ),
    (
        "chmod 6755",
        "setuid+setgid — high privilege escalation risk",
    ),
];

/// Output of `lxchmod`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub suggestion: String,
    pub reason: String,
    /// True if the suggestion matched a local dangerous pattern.
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    /// Render the suggestion as plain text (result field only).
    pub fn to_plain(&self) -> String {
        self.suggestion.clone()
    }
}

/// A dangerous pattern found in a suggestion: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan a chmod suggestion for dangerous patterns. Pure — no I/O.
///
/// Returns every matched pattern; main.rs emits them as tier-3 danger warnings.
pub fn check(suggestion: &str) -> Vec<Finding> {
    let lower = suggestion.to_lowercase();
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
            "WARNING: suggestion contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review carefully before executing. This command was NOT run.");
    }
}

/// Core logic for lxchmod.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
/// SEC flags: nonet (no network), nocmd (suggests only, never executes),
/// fsbound (caller must validate paths stay within allowed directory).
pub fn run(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<Finding>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: input.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.suggestion.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty suggestion".to_string(),
        ));
    }

    // Local danger detection — deterministic, never delegated to the LLM.
    // Emission is main.rs's job (tier-3 stderr); run() stays pure.
    let findings = check(&out.suggestion);
    out.dangerous = !findings.is_empty();

    Ok((out, findings))
}
