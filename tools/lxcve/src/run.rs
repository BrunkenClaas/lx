use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// 1024 tokens: generous enough for ~20 CVE entries with descriptions.
const MAX_TOKENS: u32 = 1024;

// ── Output types ──────────────────────────────────────────────────────────────

/// A single CVE finding for a package.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Vuln {
    pub package: String,
    pub version: String,
    pub cve_id: String,
    pub severity: String,
    pub description: String,
    /// Model's self-reported confidence: "high", "medium", or "low".
    #[serde(default = "default_confidence")]
    pub confidence: String,
}

fn default_confidence() -> String {
    "low".to_string()
}

/// Output of `lxcve`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub vulns: Vec<Vuln>,
}

impl Output {
    /// Render as human-readable plain text (one vulnerability per block).
    pub fn to_plain(&self) -> String {
        if self.vulns.is_empty() {
            return "No known CVEs found in the provided lockfile.".to_string();
        }
        self.vulns
            .iter()
            .map(|v| {
                let cve = if v.cve_id.is_empty() {
                    "CVE unknown".to_string()
                } else {
                    v.cve_id.clone()
                };
                format!(
                    "[{}] {} {} ({}): {} [confidence: {}]",
                    v.severity, v.package, v.version, cve, v.description, v.confidence
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── Core logic ────────────────────────────────────────────────────────────────

/// Core logic for `lxcve`.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
///
/// SEC: nonet — no network calls outside the LLM call.
/// SEC: fsbound — file boundary enforcement is done in main.rs before calling run().
/// SEC: untrusted — system prompt instructs the model to ignore embedded instructions.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no lockfile content provided; pipe a lockfile or use --file <path>".to_string(),
        ));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_msg = format!(
        "Analyse this lockfile for known CVE vulnerabilities:\n\n{}",
        input.trim()
    );

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

    parse_response::<Output>(&resp.content)
}
