use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

/// The regex flavors supported by `lxregex`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flavor {
    Pcre,
    Rust,
    Python,
    Js,
    Go,
    Ere,
}

impl Flavor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Flavor::Pcre => "pcre",
            Flavor::Rust => "rust",
            Flavor::Python => "python",
            Flavor::Js => "js",
            Flavor::Go => "go",
            Flavor::Ere => "ere",
        }
    }
}

impl std::str::FromStr for Flavor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pcre" => Ok(Flavor::Pcre),
            "rust" => Ok(Flavor::Rust),
            "python" => Ok(Flavor::Python),
            "js" | "javascript" => Ok(Flavor::Js),
            "go" | "golang" => Ok(Flavor::Go),
            "ere" | "posix" => Ok(Flavor::Ere),
            other => Err(format!(
                "unknown flavor '{}'; expected one of: pcre, rust, python, js, go, ere",
                other
            )),
        }
    }
}

/// Output of `lxregex`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub pattern: String,
    pub explanation: String,
    pub dangerous: bool,
}

impl Output {
    /// Plain-text output: the regex pattern only (pipe-safe).
    /// Explanation goes to stderr via main.rs.
    pub fn to_plain(&self) -> String {
        self.pattern.clone()
    }
}

/// Heuristic ReDoS detector — deterministic, local, never delegated to the LLM.
///
/// Checks for nested quantifier patterns that can cause catastrophic backtracking:
/// `(X+)+`, `(X*)+`, `(X+)*`, `(X|Y)+` with overlapping branches, etc.
pub fn is_potentially_redos(pattern: &str) -> bool {
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let is_quantifier = |c: char| matches!(c, '+' | '*' | '?');

    for i in 0..n {
        // Look for `)` followed by an optional lazy `?` then a greedy quantifier.
        // This means a group is quantified at the outer level: )+  )*  )+?  )*?
        if chars[i] != ')' {
            continue;
        }
        let mut j = i + 1;
        if j < n && chars[j] == '?' {
            j += 1; // skip lazy marker
        }
        if j >= n || !is_quantifier(chars[j]) {
            continue;
        }
        // Outer quantifier found. Walk back to find the matching `(` and check
        // whether the group interior itself contains a quantifier — if so, ReDoS risk.
        let mut depth = 0usize;
        let mut k = i;
        loop {
            match chars[k] {
                ')' => depth += 1,
                '(' => {
                    if depth == 1 {
                        // Found the matching open paren; check interior for quantifiers.
                        if chars[k + 1..i].iter().any(|&c| is_quantifier(c)) {
                            return true;
                        }
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
            if k == 0 {
                break;
            }
            k -= 1;
        }
    }

    false
}

/// Build the ReDoS warning lines for a pattern flagged dangerous. Pure — no I/O.
///
/// main.rs emits the returned lines as a tier-3 danger warning (always shown,
/// never suppressed by --quiet).
pub fn redos_warning_lines(pattern: &str) -> Vec<String> {
    vec![
        "WARNING: generated pattern may have catastrophic backtracking (ReDoS) risk.".to_string(),
        format!("         Review '{pattern}' before using in production."),
        "         Pattern was NOT executed.".to_string(),
    ]
}

/// Emit ReDoS warning lines on stderr (tier-3: always shown, never suppressed by --quiet).
pub fn warn_redos(lines: &[String]) {
    for line in lines {
        eprintln!("{line}");
    }
}

/// Core logic for `lxregex`. Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// When `existing` is `None`, generates a regex from the description.
/// When `existing` is `Some(pattern)`, edits that pattern applying only the described change.
/// NEVER executes the pattern. Checks for ReDoS risk locally (§8.3 nocmd);
/// returns the warning lines for main.rs to emit — this function never prints.
pub fn run(
    description: &str,
    flavor: &Flavor,
    existing: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_message = match existing {
        Some(pat) if !pat.trim().is_empty() => format!(
            "Edit the following {} regex — apply this change ONLY: {}\n\nPreserve every other part verbatim.\n\n---\n{}",
            flavor.as_str(),
            description.trim(),
            pat.trim()
        ),
        _ => format!("Input ({}): {}", flavor.as_str(), description.trim()),
    };

    let req = Request {
        system: &system,
        user: &user_message,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client.complete(&req).map_err(LxError::from)?;

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.pattern.is_empty() {
        return Err(LxError::LogicalError(
            "model returned an empty pattern".to_string(),
        ));
    }

    // Local ReDoS detection — deterministic, overrides model's dangerous:false.
    if is_potentially_redos(&out.pattern) {
        out.dangerous = true;
    }

    // Build the ReDoS warning if the pattern is potentially dangerous (§8.3 nocmd).
    // Emission is main.rs's job (tier-3 stderr); run() stays pure.
    let warnings = if out.dangerous {
        redos_warning_lines(&out.pattern)
    } else {
        Vec::new()
    };

    Ok((out, warnings))
}
