use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
/// A .gitignore for a mid-size project is rarely more than 200 lines.
const MAX_TOKENS: u32 = 2048;

/// Output of `lxgitignore`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub content: String,
}

impl Output {
    /// Render as plain text: the gitignore content is the result.
    pub fn to_plain(&self) -> String {
        self.content.clone()
    }
}

/// Core logic for `lxgitignore`.
///
/// Create mode (`existing` is None):
///   `input` is a project structure listing (file names, extensions, dirs).
///   Generates a fresh .gitignore appropriate for the detected project type.
///
/// Edit mode (`existing` is Some):
///   `input` is the change intent; `existing` is the current .gitignore content.
///   Applies the intent to the existing file, preserving everything else verbatim.
///
/// Security flags: `fsbound`. The fsbound check is done in `main.rs`.
pub fn run(
    input: &str,
    existing: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no project structure provided".to_string(),
        ));
    }

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let user_msg = match existing {
        Some(content) if !content.trim().is_empty() => format!(
            "Edit the following .gitignore — apply this change ONLY: {}\n\nPreserve every other line verbatim.\n\n---\n{}",
            input.trim(),
            content.trim()
        ),
        _ => input.trim().to_string(),
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

    parse_response::<Output>(&resp.content)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_plain_returns_content() {
        let out = Output {
            content: "# .gitignore\ntarget/\n*.log\n".to_string(),
        };
        assert_eq!(out.to_plain(), "# .gitignore\ntarget/\n*.log\n");
    }

    #[test]
    fn to_plain_empty_content() {
        let out = Output {
            content: String::new(),
        };
        assert_eq!(out.to_plain(), "");
    }
}
