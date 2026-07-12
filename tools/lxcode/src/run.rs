use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

use crate::danger;

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 2048;

/// Output of `lxcode`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub code: String,
    pub language: String,
    /// True if generated code matched a local dangerous pattern.
    #[serde(default)]
    pub dangerous: bool,
}

impl Output {
    /// Returns only the code string — the primary output (no wrapper, no fences).
    pub fn to_plain(&self) -> String {
        self.code.clone()
    }
}

/// Core logic for lxcode.
///
/// Generates code from a natural-language description.
/// - `lang_hint`: optional target language (e.g. "rust", "python"). When `None`
///   the model auto-detects from the description.
/// - NEVER executes the generated code (§8.3 nocmd).
/// - Runs local dangerous-pattern detection before returning.
pub fn run(
    description: &str,
    lang_hint: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if description.trim().is_empty() {
        return Err(LxError::BadUsage("no description provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    // If the caller specified a target language, append it to the user message
    // so the model can use it without us modifying the system prompt at runtime.
    let user_message = match lang_hint {
        Some(lang) if !lang.is_empty() && lang != "auto" => {
            format!("Language: {lang}\n\n{}", description.trim())
        }
        _ => description.trim().to_string(),
    };

    let req = Request {
        system: &system,
        user: &user_message,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(lx_core::error::LxError::from)?;

    let mut out = parse_response::<Output>(&resp.content)?;

    if out.code.trim().is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty code".to_string(),
        ));
    }
    if out.language.trim().is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty language field".to_string(),
        ));
    }

    // §8.3 nocmd: local danger detection — deterministic, never delegated to LLM.
    out.dangerous = danger::check_and_warn(&out.code);

    Ok(out)
}
