use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// Generous token budget: annotated code can be as large as the input.
const MAX_TOKENS: u32 = 2048;

/// Dangerous patterns that must be flagged on stderr before output (§8.3 nocmd).
///
/// This check is deterministic and local — never delegated to the LLM.
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("rm -rf /", "recursive deletion of root filesystem"),
    ("rm -rf ~", "recursive deletion of home directory"),
    ("rm -fr /", "recursive deletion of root filesystem"),
    ("dd of=/dev/", "direct write to block device"),
    ("mkfs", "filesystem creation — destroys existing data"),
    (":(){ :|:& };:", "fork bomb — will crash the system"),
    (":(){:|:&};:", "fork bomb variant"),
    ("curl | sh", "executing untrusted remote script"),
    ("curl|sh", "executing untrusted remote script"),
    ("wget | sh", "executing untrusted remote script"),
    ("wget|sh", "executing untrusted remote script"),
    ("curl | bash", "executing untrusted remote script"),
    ("curl|bash", "executing untrusted remote script"),
    (
        "iwr | iex",
        "executing untrusted remote script (PowerShell)",
    ),
    ("iwr|iex", "executing untrusted remote script (PowerShell)"),
    ("Invoke-Expression", "dynamic script execution (PowerShell)"),
    (
        "Remove-Item -Recurse /",
        "recursive deletion via PowerShell",
    ),
    (
        "Remove-Item -Recurse C:\\",
        "recursive deletion of system drive",
    ),
    ("DROP TABLE", "destructive SQL — permanently deletes table"),
    (
        "DROP DATABASE",
        "destructive SQL — permanently deletes database",
    ),
    (
        "DELETE FROM",
        "SQL deletion — may remove all rows if no WHERE clause",
    ),
    ("truncate table", "SQL truncation — removes all rows"),
    ("FORMAT C:", "formats the Windows system drive"),
    ("> /dev/sda", "direct write to disk device"),
    ("shred", "irreversible file shredding"),
];

/// A dangerous pattern found in generated code: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan generated code for dangerous patterns. Pure — no I/O.
///
/// Returns every matched pattern; main.rs emits them as tier-3 danger warnings.
pub fn check(code: &str) -> Vec<Finding> {
    let lower = code.to_lowercase();
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
            "WARNING: annotated code contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review the annotated code carefully before using it.");
    }
}

/// Output of `lxtypehint`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The annotated source code with type hints added.
    pub code: String,
}

/// Core logic for lxtypehint.
///
/// Adds type hints/annotations to the given source code.
/// SEC: nocmd — output is text only, never executed.
/// SEC: untrusted — system prompt instructs model to ignore embedded instructions.
pub fn run(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<Finding>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no code provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let req = Request {
        system: &system,
        user: input.trim(),
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;

    let out = parse_response::<Output>(&resp.content)?;

    if out.code.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty annotated code".to_string(),
        ));
    }

    // Local danger detection — deterministic, never delegated to the LLM (§8.3).
    // Emission is main.rs's job (tier-3 stderr); run() stays pure.
    let findings = check(&out.code);

    Ok((out, findings))
}
