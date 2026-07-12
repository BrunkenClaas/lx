use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

/// Dangerous Makefile/shell patterns that must be flagged on stderr.
///
/// Detection is deterministic and local — never delegated to the LLM (§8.3).
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf /", "recursive deletion of root filesystem"),
    ("| sh", "piping to shell — executes untrusted commands"),
    ("|sh", "piping to shell — executes untrusted commands"),
    ("| bash", "piping to bash — executes untrusted commands"),
    ("|bash", "piping to bash — executes untrusted commands"),
    ("> /dev/", "direct write to device file"),
    (">/dev/", "direct write to device file"),
    ("curl | sh", "executing untrusted remote script"),
    ("curl|sh", "executing untrusted remote script"),
    ("wget | sh", "executing untrusted remote script"),
    ("wget|sh", "executing untrusted remote script"),
    ("dd if=", "raw disk copy — may overwrite data"),
    ("mkfs", "filesystem creation — destroys existing data"),
];

/// A dangerous pattern found in generated content: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan the generated content for dangerous patterns. Pure — no I/O.
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
            "⚠  DANGER: generated content contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review carefully before using. This content was NOT executed.");
    }
}

/// Output of `lxmakefile`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub content: String,
    pub dangerous: bool,
}

/// Core logic for lxmakefile.
///
/// Create mode (`existing` is None): generates a fresh Makefile/justfile from a description.
/// Edit mode (`existing` is Some): applies `intent` to the existing file, preserving
/// everything not mentioned in the intent verbatim.
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
            "Edit the following Makefile/justfile — apply this change ONLY: {}\n\nPreserve every other line verbatim.\n\n---\n{}",
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

    #[derive(Deserialize)]
    struct RawOutput {
        content: String,
    }

    let raw = parse_response::<RawOutput>(&resp.content)?;

    if raw.content.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty content".to_string(),
        ));
    }

    // Local danger detection — deterministic. Emission is main.rs's job
    // (tier-3 stderr); run() stays pure.
    let findings = check(&raw.content);

    Ok((
        Output {
            content: raw.content,
            dangerous: !findings.is_empty(),
        },
        findings,
    ))
}
