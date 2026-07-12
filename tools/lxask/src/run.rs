use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

/// Output of `lxask`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub answer: String,
    pub sources: Vec<String>,
}

/// Core logic for lxask.
///
/// Both `question` and `context` are already redacted by the time they arrive
/// here — redaction happens in main.rs before `run()` is called.
pub fn run(
    question: &str,
    context: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if question.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no question provided; pass a question as a positional argument or via stdin"
                .to_string(),
        ));
    }

    let user_message = match context {
        Some(ctx) if !ctx.trim().is_empty() => {
            format!("[Context: {}]\n{}", ctx.trim(), question.trim())
        }
        _ => question.trim().to_string(),
    };

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

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

    let out = parse_response::<Output>(&resp.content)?;

    if out.answer.is_empty() {
        return Err(LxError::LogicalError(
            "model returned empty answer".to_string(),
        ));
    }

    Ok(out)
}
