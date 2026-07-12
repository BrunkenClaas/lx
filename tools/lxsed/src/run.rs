use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Dangerous patterns for awk/sed one-liners (§8.3 nocmd).
/// Uses string-contains, no regex crate.
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("| sh", "piping output into a shell"),
    ("|sh", "piping output into a shell"),
    ("| bash", "piping output into bash"),
    ("|bash", "piping output into bash"),
    ("| zsh", "piping output into zsh"),
    ("|zsh", "piping output into zsh"),
    ("| iex", "piping output into PowerShell Invoke-Expression"),
    ("|iex", "piping output into PowerShell Invoke-Expression"),
    ("rm -rf /", "recursive deletion of root filesystem"),
    ("drop table", "destructive SQL — permanently deletes table"),
    ("delete from", "SQL deletion — may remove all rows"),
    ("> /dev/", "direct write to device file"),
    (">/dev/", "direct write to device file"),
];

/// Output of `lxsed`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub command: String,
    /// Either "awk" or "sed".
    pub tool: String,
    /// Set to true if a locally-detected dangerous pattern is found.
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    pub fn to_plain(&self) -> String {
        self.command.clone()
    }
}

/// A dangerous pattern found in a command: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan the command for dangerous patterns. Pure — no I/O.
///
/// Returns every matched pattern; main.rs emits them as tier-3 danger warnings.
pub fn check_danger(command: &str) -> Vec<Finding> {
    let lower = command.to_lowercase();
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
            "⚠  DANGER: command contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review carefully before executing. This command was NOT run.");
    }
}

/// Core logic for lxsed.
///
/// Generates an awk or sed one-liner from a plain-English description.
/// NEVER executes the command. Checks for dangerous patterns locally (§8.3).
pub fn run(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<Finding>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
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

    if out.command.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty command".to_string(),
        ));
    }

    // Validate tool field
    if out.tool != "awk" && out.tool != "sed" {
        return Err(LxError::LogicalError(format!(
            "model returned invalid tool value: '{}' (expected 'awk' or 'sed')",
            out.tool
        )));
    }

    // Local danger detection — deterministic, not delegated to the LLM.
    // Emission is main.rs's job (tier-3 stderr); run() stays pure.
    let findings = check_danger(&out.command);
    if !findings.is_empty() {
        out.dangerous = true;
    }

    Ok((out, findings))
}
