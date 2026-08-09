use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 2048;

/// Maximum input bytes sent to the model.
///
/// Translation returns text of roughly the same length as its input, so the
/// input budget must leave room for the reply inside one context window: at
/// `MAX_TOKENS = 2048` out, ~24 KB in keeps both sides comfortable. Longer
/// documents should be translated in chunks (`split` then pipe).
const MAX_INPUT_BYTES: usize = 24_000;

/// Output of `lxtl`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub text: String,
}

/// Truncate very large input to keep the request inside the model's context
/// window, collecting a tier-2 warning (emitted by main.rs) if it fired.
/// Pure — no I/O.
fn truncate_input(input: &str) -> (&str, Vec<String>) {
    if input.len() > MAX_INPUT_BYTES {
        (
            lx_core::io::truncate_at_char_boundary(input, MAX_INPUT_BYTES),
            vec![format!("input truncated to {MAX_INPUT_BYTES} bytes")],
        )
    } else {
        (input, Vec::new())
    }
}

/// Core logic for lxtl.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(
    input: &str,
    target_lang: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<(Output, Vec<String>), LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }
    if target_lang.trim().is_empty() {
        return Err(LxError::BadUsage(
            "target language must be specified with --to".to_string(),
        ));
    }

    let (input, warnings) = truncate_input(input);

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
    Ok((output, warnings))
}
