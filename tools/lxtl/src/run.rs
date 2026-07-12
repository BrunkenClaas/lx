use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 2048;

/// Output of `lxtl`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub text: String,
}

/// Core logic for lxtl.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(
    input: &str,
    target_lang: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }
    if target_lang.trim().is_empty() {
        return Err(LxError::BadUsage(
            "target language must be specified with --to".to_string(),
        ));
    }

    // Replace {target_lang} first, then inject {lang} for output language.
    let system_with_target = SYSTEM_TEMPLATE.replace("{target_lang}", target_lang);
    let system = inject_lang(&system_with_target, &config.output.lang);

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

    let output: Output = parse_response(&resp.content)?;
    if output.text.is_empty() {
        return Err(LxError::LogicalError(
            "LLM returned an empty translation".to_string(),
        ));
    }
    Ok(output)
}
