use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response, LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 2048;

/// Output of `lxtable`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Output {
    /// Render as a Markdown table for plain (non-JSON) output.
    pub fn to_plain(&self) -> String {
        if self.columns.is_empty() {
            return String::new();
        }

        // Compute column widths (min = header length).
        let mut widths: Vec<usize> = self.columns.iter().map(|c| c.len()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate() {
                if i < widths.len() {
                    widths[i] = widths[i].max(cell.len());
                }
            }
        }

        let mut out = String::new();

        // Header row.
        out.push('|');
        for (i, col) in self.columns.iter().enumerate() {
            out.push(' ');
            out.push_str(col);
            let pad = widths[i].saturating_sub(col.len());
            for _ in 0..pad {
                out.push(' ');
            }
            out.push_str(" |");
        }
        out.push('\n');

        // Separator row.
        out.push('|');
        for w in &widths {
            out.push(' ');
            for _ in 0..*w {
                out.push('-');
            }
            out.push_str(" |");
        }
        out.push('\n');

        // Data rows.
        for row in &self.rows {
            out.push('|');
            for (i, w) in widths.iter().enumerate() {
                let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
                out.push(' ');
                out.push_str(cell);
                let pad = w.saturating_sub(cell.len());
                for _ in 0..pad {
                    out.push(' ');
                }
                out.push_str(" |");
            }
            out.push('\n');
        }

        // Remove trailing newline for cleaner output.
        if out.ends_with('\n') {
            out.pop();
        }
        out
    }
}

/// Core logic for lxtable.
///
/// Pure function: no I/O, no process::exit. Testable with MockLlmClient.
pub fn run(input: &str, config: &Config, client: &dyn LlmClient) -> Result<Output, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
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

    parse_response::<Output>(&resp.content)
}
