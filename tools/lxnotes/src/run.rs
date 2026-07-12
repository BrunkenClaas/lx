use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
pub const ACTIONS_SYSTEM_TEMPLATE: &str = include_str!("../prompts/actions_system.txt");
// Restates full meeting notes into sectioned bullets (~1:1, no input cap). A
// long transcript exceeded 1024 and truncated. 2048 covers an hour of notes.
const MAX_TOKENS: u32 = 2048;

/// A single structured section extracted from meeting notes.
#[derive(Debug, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub content: Vec<String>,
}

/// Output of `lxnotes`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub sections: Vec<Section>,
}

impl Output {
    /// Format sections as plain text suitable for stdout.
    pub fn to_plain(&self) -> String {
        let mut lines = Vec::new();
        for section in &self.sections {
            lines.push(format!("## {}", section.title));
            for item in &section.content {
                lines.push(format!("- {}", item));
            }
            lines.push(String::new());
        }
        // Remove trailing blank line if present
        if lines.last().map(|l| l.is_empty()).unwrap_or(false) {
            lines.pop();
        }
        lines.join("\n")
    }
}

/// A single action item extracted from meeting notes (--actions mode).
#[derive(Debug, Serialize, Deserialize)]
pub struct Action {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub who: Option<String>,
    pub what: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
}

/// Output of `lxnotes --actions`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ActionsOutput {
    pub actions: Vec<Action>,
}

impl ActionsOutput {
    pub fn to_plain(&self) -> String {
        if self.actions.is_empty() {
            return "No action items found.".to_string();
        }
        self.actions
            .iter()
            .map(|a| {
                let who = a.who.as_deref().unwrap_or("—");
                let due = a
                    .due
                    .as_deref()
                    .map(|d| format!(" (due: {d})"))
                    .unwrap_or_default();
                format!("- [{who}] {}{due}", a.what)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Core logic for `lxnotes --actions` — extracts action items from notes.
///
/// `input` must already be redacted before calling this function.
pub fn run_actions(
    input: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<ActionsOutput, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no meeting notes provided; pipe raw notes into lxnotes".to_string(),
        ));
    }

    let system = inject_lang(ACTIONS_SYSTEM_TEMPLATE, &config.output.lang);

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

    parse_response::<ActionsOutput>(&resp.content)
}

/// Core logic for lxnotes — structures raw meeting notes into sections.
///
/// `input` must already be redacted before calling this function.
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage(
            "no meeting notes provided; pipe raw notes into lxnotes".to_string(),
        ));
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

    let out = parse_response::<Output>(&resp.content)?;

    if out.sections.is_empty() {
        return Err(LxError::LogicalError(
            "model returned no sections from meeting notes".to_string(),
        ));
    }

    Ok(out)
}
