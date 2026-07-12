use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Dangerous kubectl patterns detected locally (§8.3 nocmd).
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("kubectl delete", "permanently removes Kubernetes resources"),
    (
        "kubectl drain",
        "evicts all pods from a node, affecting cluster availability",
    ),
    (
        "kubectl cordon",
        "marks a node unschedulable, affecting cluster availability",
    ),
    (
        "kubectl exec",
        "executes commands inside a running container",
    ),
    (
        "--all-namespaces",
        "operates across all namespaces — verify scope carefully",
    ),
    ("| sh", "pipes output into a shell for execution"),
    ("| bash", "pipes output into bash for execution"),
    ("| iex", "pipes output into PowerShell Invoke-Expression"),
    ("; rm", "contains a shell remove command"),
    ("$(rm", "contains a shell remove subcommand"),
];

/// A dangerous pattern found in a command: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan the generated kubectl command for dangerous patterns. Pure — no I/O.
///
/// Returns every matched pattern; main.rs emits them as tier-3 danger warnings.
pub fn check(command: &str) -> Vec<Finding> {
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
            "DANGER: command contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review carefully before executing. This command was NOT run.");
    }
}

/// Output of `lxkubectl`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub command: String,
    pub dangerous: bool,
}

impl Output {
    pub fn to_plain(&self) -> String {
        self.command.clone()
    }
}

/// Core logic for lxkubectl.
///
/// Generates a kubectl command from a plain-English description.
/// NEVER executes the command. Checks for dangerous patterns locally (§8.3 nocmd).
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

    // Local danger detection — deterministic, never delegated to the LLM.
    // Emission is main.rs's job (tier-3 stderr); run() stays pure.
    let findings = check(&out.command);
    if !findings.is_empty() {
        out.dangerous = true;
    }

    Ok((out, findings))
}
