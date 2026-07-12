use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 150;

/// Dangerous docker patterns detected locally (§8.3 nocmd).
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    (
        "--privileged",
        "grants host-level privileges to the container",
    ),
    (
        "docker rm -f",
        "force-removes containers without confirmation",
    ),
    (
        "system prune",
        "removes all unused docker data (images, containers, volumes)",
    ),
    ("container prune", "removes all stopped containers"),
    ("image prune", "removes dangling (or all unused) images"),
    (
        "volume rm",
        "permanently deletes named volumes and their data",
    ),
    ("network rm", "permanently removes docker networks"),
    (
        "| sh",
        "pipes output to a shell — remote code execution risk",
    ),
    (
        "| bash",
        "pipes output to bash — remote code execution risk",
    ),
    (
        "|sh",
        "pipes output to a shell — remote code execution risk",
    ),
    ("|bash", "pipes output to bash — remote code execution risk"),
    (
        "| iex",
        "pipes output to PowerShell — remote code execution risk",
    ),
    (
        "|iex",
        "pipes output to PowerShell — remote code execution risk",
    ),
];

/// Dangerous volume-mount patterns (host root mount).
static DANGEROUS_MOUNT_PATTERNS: &[(&str, &str)] = &[
    (
        "-v /:/",
        "mounts the host root filesystem into the container",
    ),
    ("-v /:", "mounts a sensitive host path into the container"),
    ("--mount source=/,", "mounts host root into the container"),
];

/// A dangerous pattern found in a command: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan the generated docker command for dangerous patterns. Pure — no I/O.
///
/// Checks both the general and mount-specific pattern lists. Returns every
/// matched pattern; main.rs emits them as tier-3 danger warnings.
pub fn check(command: &str) -> Vec<Finding> {
    let lower = command.to_lowercase();
    DANGEROUS_PATTERNS
        .iter()
        .chain(DANGEROUS_MOUNT_PATTERNS.iter())
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

/// Output of `lxdockercmd`.
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

/// Core logic for lxdockercmd.
///
/// Generates a docker command from a plain-English description.
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
