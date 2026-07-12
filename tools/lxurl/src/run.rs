#![forbid(unsafe_code)]

use lx_config::Config;
use lx_core::exit::LxError;
use lx_llm::lang::inject_lang;
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

use crate::fetch::{fetch_and_extract, DEFAULT_MAX_URL_BYTES};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 512;

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub url: String,
    pub title: Option<String>,
    pub answer: String,
    pub truncated: bool,
}

impl Output {
    pub fn to_plain(&self) -> String {
        let mut out = format!("URL: {}\n", self.url);
        if let Some(ref t) = self.title {
            out.push_str(&format!("Title: {t}\n"));
        }
        if self.truncated {
            out.push_str("(page content was truncated)\n");
        }
        out.push('\n');
        out.push_str(&self.answer);
        out
    }
}

pub fn run(
    url: &str,
    question: &str,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<Output, LxError> {
    let max_bytes = config.limits.max_input_bytes.min(DEFAULT_MAX_URL_BYTES);

    let timeout = config.llm.timeout_secs;
    let (text, truncated) = fetch_and_extract(url, max_bytes, timeout)?;

    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);

    let q = if question.trim().is_empty() {
        "Summarise this page."
    } else {
        question.trim()
    };

    let user_msg = format!("URL: {url}\n\nQuestion: {q}\n\nPage content:\n{text}");

    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };

    let resp = client
        .complete(&req)
        .map_err(|e| LxError::NetworkLlm(e.to_string()))?;

    let mut out: Output = lx_llm::schema::parse_response::<Output>(&resp.content)
        .map_err(|e| LxError::NetworkLlm(format!("invalid LLM response: {e}")))?;

    // Ensure truncated flag reflects the actual fetch state.
    out.truncated = out.truncated || truncated;
    Ok(out)
}
