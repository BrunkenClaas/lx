use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Dangerous patterns for ffmpeg commands (§8.3 nocmd).
/// Each entry is (pattern, description).
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("| sh", "pipes output to a shell — arbitrary execution risk"),
    ("| bash", "pipes output to bash — arbitrary execution risk"),
    ("|sh", "pipes output to a shell — arbitrary execution risk"),
    ("|bash", "pipes output to bash — arbitrary execution risk"),
    ("| zsh", "pipes output to zsh — arbitrary execution risk"),
    ("|zsh", "pipes output to zsh — arbitrary execution risk"),
    ("| iex", "pipes output to PowerShell Invoke-Expression"),
    ("|iex", "pipes output to PowerShell Invoke-Expression"),
    ("/dev/sd", "writes to a raw block device — may destroy data"),
    (
        "/dev/nvme",
        "writes to a raw NVMe device — may destroy data",
    ),
    (
        "/dev/disk",
        "writes to a raw disk device — may destroy data",
    ),
    (
        "-f /dev/",
        "writes to a /dev/ path — dangerous raw device access",
    ),
    (
        "/etc/",
        "writes into /etc/ — may corrupt system configuration",
    ),
];

/// A dangerous pattern found in a command: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan the generated ffmpeg command for dangerous patterns. Pure — no I/O.
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

/// Output of `lxffmpeg`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub command: String,
    pub dangerous: bool,
}

/// Core logic for lxffmpeg.
///
/// Generates an ffmpeg command from a plain-English description.
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
