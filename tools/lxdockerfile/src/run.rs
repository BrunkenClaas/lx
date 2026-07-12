use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

/// Dangerous patterns for Dockerfile content (§8.3 nocmd).
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("curl | sh", "executing untrusted remote script"),
    ("curl|sh", "executing untrusted remote script"),
    ("wget | sh", "executing untrusted remote script"),
    ("wget|sh", "executing untrusted remote script"),
    ("| sh", "piping into shell — potential script execution"),
    ("|sh", "piping into shell — potential script execution"),
    ("| bash", "piping into bash — potential script execution"),
    ("|bash", "piping into bash — potential script execution"),
    ("rm -rf /", "recursive deletion of root filesystem"),
    (
        "--privileged",
        "privileged container — bypasses Docker security isolation",
    ),
    ("> /dev/", "direct write to device node"),
];

/// Output of `lxdockerfile`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub content: String,
    pub dangerous: bool,
}

/// A dangerous pattern found in a Dockerfile: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan Dockerfile content for dangerous patterns. Pure — no I/O.
///
/// Returns every matched pattern; main.rs emits them as tier-3 danger warnings.
pub fn check(content: &str) -> Vec<Finding> {
    let lower = content.to_lowercase();
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
            "⚠  DANGER: Dockerfile contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review carefully before building this image.");
    }
}

/// Core logic for lxdockerfile.
///
/// Create mode (`existing` is None): generates a fresh Dockerfile from a description.
/// Edit mode (`existing` is Some): applies `intent` to the existing Dockerfile,
/// preserving everything not mentioned in the intent verbatim.
pub fn run(
    intent: &str,
    existing: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<Finding>), LxError> {
    if intent.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_msg = match existing {
        Some(content) if !content.trim().is_empty() => format!(
            "Edit the following Dockerfile — apply this change ONLY: {}\n\nPreserve every other line verbatim.\n\n---\n{}",
            intent.trim(),
            content.trim()
        ),
        _ => intent.trim().to_string(),
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

    if out.content.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty Dockerfile content".to_string(),
        ));
    }

    // Local danger detection — deterministic. Emission is main.rs's job
    // (tier-3 stderr); run() stays pure.
    let findings = check(&out.content);

    Ok((
        Output {
            content: out.content,
            dangerous: out.dangerous || !findings.is_empty(),
        },
        findings,
    ))
}
