use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// Allow up to 2048 tokens for documented output — code can be long.
const MAX_TOKENS: u32 = 2048;

/// Docstring style hint passed by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocStyle {
    /// Automatically detect the right style from the code language.
    Auto,
    /// Python-style triple-quoted docstrings.
    Docstring,
    /// JavaDoc `/** … */` comments.
    Javadoc,
    /// Rust `///` doc-comments.
    Rustdoc,
}

impl DocStyle {
    pub fn as_hint(&self) -> Option<&'static str> {
        match self {
            DocStyle::Auto => None,
            DocStyle::Docstring => Some("Use Python-style triple-quoted docstrings."),
            DocStyle::Javadoc => Some("Use JavaDoc /** ... */ comments."),
            DocStyle::Rustdoc => Some("Use Rust /// doc-comments."),
        }
    }
}

/// Output produced by `lxdoc`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The original code with docstrings/comments inserted.
    pub code: String,
}

impl Output {
    /// Render as plain text: just the documented code.
    pub fn to_plain(&self) -> String {
        self.code.clone()
    }
}

/// Core logic for `lxdoc`.
///
/// Pure function: no I/O, no `process::exit`. Testable with `MockLlmClient`.
///
/// # Security (untrusted)
/// The system prompt instructs the LLM to ignore any instructions embedded in
/// the user-supplied code. System and user messages are always kept separate.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    run_with_style(input, config, client, &DocStyle::Auto)
}

/// Like [`run`] but accepts an explicit [`DocStyle`] hint.
pub fn run_with_style(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
    style: &DocStyle,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    // Append style hint when explicitly requested.
    let system = if let Some(hint) = style.as_hint() {
        format!("{system}\nStyle instruction: {hint}")
    } else {
        system
    };

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

    parse_response::<Output>(&resp.content)
}
