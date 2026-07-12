use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// Dangerous curl-specific patterns detected locally (§8.3 nocmd).
static DANGEROUS_PATTERNS: &[(&str, &str)] = &[
    ("| sh", "curl output piped to shell"),
    ("| bash", "curl output piped to shell"),
    ("| zsh", "curl output piped to shell"),
    ("| fish", "curl output piped to shell"),
    ("|sh", "curl output piped to shell"),
    ("|bash", "curl output piped to shell"),
    ("|zsh", "curl output piped to shell"),
    ("|fish", "curl output piped to shell"),
    ("| iex", "curl output piped to PowerShell Invoke-Expression"),
    ("|iex", "curl output piped to PowerShell Invoke-Expression"),
    ("--output /etc/", "writing to system directory /etc/"),
    ("--output /usr/", "writing to system directory /usr/"),
    ("-o /etc/", "writing to system directory /etc/"),
    ("-o /usr/", "writing to system directory /usr/"),
    ("file:///etc/", "reading system files via file:// URI"),
    ("file:///usr/", "reading system files via file:// URI"),
];

/// A dangerous pattern found in a command: the matched pattern and why it is dangerous.
#[derive(Debug)]
pub struct Finding {
    pub pattern: &'static str,
    pub description: &'static str,
}

/// Scan the generated curl command for dangerous patterns. Pure — no I/O.
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
            "⚠  DANGER: command contains '{}' — {}",
            f.pattern, f.description
        );
    }
    if !findings.is_empty() {
        eprintln!("   Review carefully before executing. This command was NOT run.");
    }
}

/// Output of `lxcurl`.
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

/// Core logic for lxcurl. Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// Generates a curl command from a plain-English API description.
/// NEVER executes the command. Checks for dangerous patterns locally (§8.3 nocmd);
/// returns any findings for main.rs to emit on stderr — this function never prints.
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
    // If our pattern check detects danger, override whatever the model said.
    // Emission of the findings is main.rs's job (tier-3 stderr); run() stays pure.
    let findings = check(&out.command);
    if !findings.is_empty() {
        out.dangerous = true;
    }

    Ok((out, findings))
}
