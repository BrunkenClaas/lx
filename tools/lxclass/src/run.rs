use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

/// One label + its confidence score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabelScore {
    pub label: String,
    pub confidence: f64,
}

/// Output of `lxclass`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    /// The winning label.
    pub label: String,
    /// Confidence of the winning label (0.0 – 1.0).
    pub confidence: f64,
    /// Scores for all provided labels.
    pub all: Vec<LabelScore>,
}

/// Core logic for lxclass.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(
    input: &str,
    labels: &[String],
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }
    if labels.is_empty() {
        return Err(LxError::BadUsage(
            "--labels must specify at least one label".to_string(),
        ));
    }

    // Build the labels string for the {labels} placeholder.
    let labels_str = labels.join(", ");

    // Replace {labels} first, then inject {lang}.
    let system = inject_lang(
        &SYSTEM_TEMPLATE.replace("{labels}", &labels_str),
        &config.output.lang,
    );

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

    let output = parse_response::<Output>(&resp.content)?;

    // Validate: the returned label must be one of the provided labels.
    if !labels.contains(&output.label) {
        return Err(LxError::LogicalError(format!(
            "LLM returned invalid label {:?}; expected one of: {}",
            output.label, labels_str
        )));
    }

    Ok(output)
}
