use crate::danger;
use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Generate,
    Explain,
    /// Edit mode: positional arg is change description, stdin is existing crontab line.
    Edit,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CronOutput {
    pub crontab: String,
    pub explanation: String,
    #[serde(default)]
    pub dangerous: bool,
}

impl CronOutput {
    /// Plain output depends on mode:
    /// - Generate/Edit: return the crontab line
    /// - Explain: return the explanation
    pub fn to_plain(&self, mode: Mode) -> String {
        match mode {
            Mode::Generate | Mode::Edit => self.crontab.clone(),
            Mode::Explain => self.explanation.clone(),
        }
    }
}

/// Detect if `input` looks like an existing crontab line (5-field schedule).
/// Returns `Mode::Explain` if the first 5 whitespace-separated tokens each match
/// a cron field pattern; otherwise `Mode::Generate`.
pub fn detect_mode(input: &str) -> Mode {
    let line = input.trim().lines().next().unwrap_or("").trim();
    if line.starts_with('#') || line.is_empty() {
        return Mode::Generate;
    }
    let fields: Vec<&str> = line.split_whitespace().collect();
    if fields.len() < 6 {
        return Mode::Generate;
    }
    let is_cron_field = |s: &str| -> bool {
        if s == "*" {
            return true;
        }
        // */N
        if let Some(rest) = s.strip_prefix("*/") {
            return rest.parse::<u32>().is_ok();
        }
        // N or N-M or N,M or day/month names
        let lower = s.to_ascii_lowercase();
        if [
            "mon", "tue", "wed", "thu", "fri", "sat", "sun", "jan", "feb", "mar", "apr", "may",
            "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ]
        .iter()
        .any(|d| lower.contains(d))
        {
            return true;
        }
        // Numeric range or list — at least starts with digit
        s.chars().next().is_some_and(|c| c.is_ascii_digit())
    };
    if fields[..5].iter().all(|f| is_cron_field(f)) {
        Mode::Explain
    } else {
        Mode::Generate
    }
}

pub fn run(
    input: &str,
    mode: Mode,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<CronOutput, LxError> {
    if input.trim().is_empty() {
        return Err(LxError::BadUsage("no input provided".to_string()));
    }
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang);
    let user_msg = match mode {
        Mode::Generate => format!("Generate a crontab line for: {}", input.trim()),
        Mode::Explain => format!("Explain this crontab line: {}", input.trim()),
        Mode::Edit => {
            // input is "change_desc\n\n<existing crontab line>"
            let parts: Vec<&str> = input.splitn(2, "\n\n").collect();
            let change = parts.first().unwrap_or(&input.trim()).trim();
            let existing = parts.get(1).unwrap_or(&"").trim();
            format!(
                "Edit this crontab line — apply this change ONLY: {}\n\n{}",
                change, existing
            )
        }
    };
    let req = Request {
        system: &system,
        user: &user_msg,
        max_tokens: MAX_TOKENS,
        temperature: 0.0,
        image: None,
    };
    let resp = client.complete(&req).map_err(LxError::from)?;
    let mut out = parse_response::<CronOutput>(&resp.content)?;

    // In generate/edit mode, scan the command part of the crontab for danger.
    if (mode == Mode::Generate || mode == Mode::Edit) && !out.crontab.is_empty() {
        // Command part is everything after the 5 schedule fields.
        let cmd_part = out
            .crontab
            .splitn(6, char::is_whitespace)
            .last()
            .unwrap_or("")
            .to_string();
        if danger::check_and_warn(&cmd_part) {
            out.dangerous = true;
        }
    }
    Ok(out)
}
