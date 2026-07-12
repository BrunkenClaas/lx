use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// Output of `lxundo`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub undo_command: String,
    pub caution: String,
}

/// Danger patterns for nocmd flag — checked locally, never delegated to the LLM.
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("| sh", "piping to shell — executes untrusted remote script"),
    (
        "| bash",
        "piping to bash — executes untrusted remote script",
    ),
    ("| zsh", "piping to zsh — executes untrusted remote script"),
    ("|sh", "piping to shell — executes untrusted remote script"),
    ("|bash", "piping to bash — executes untrusted remote script"),
    ("| iex", "PowerShell dynamic execution of remote script"),
    ("|iex", "PowerShell dynamic execution of remote script"),
    ("rm -rf /", "recursive deletion of root filesystem"),
    (
        "reset --hard",
        "discards all local changes and uncommitted work permanently",
    ),
    ("drop table", "destructive SQL — permanently deletes table"),
    (
        "delete from",
        "SQL deletion — may remove all rows if no WHERE clause",
    ),
    ("format c:", "formats the Windows system drive"),
    ("mkfs", "filesystem creation — destroys existing data"),
    ("> /dev/", "direct write to block device"),
    ("dd if=", "direct disk copy — potentially destructive"),
    ("fork bomb", "fork bomb — will crash the system"),
    (":(){ :|:& };:", "fork bomb — will crash the system"),
];

/// A dangerous pattern found in an undo command: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan the undo command for dangerous patterns. Pure — no I/O.
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
            "⚠  DANGER: undo command contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review carefully before executing. This command was NOT run.");
    }
}

/// Core logic for lxundo.
///
/// Suggests how to undo a previously run command.
/// NEVER executes the undo command. Checks for dangerous patterns locally (§8.3).
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no command provided".to_string()));
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

    let out = parse_response::<Output>(&resp.content)?;

    if out.undo_command.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty undo_command".to_string(),
        ));
    }

    Ok(out)
}
