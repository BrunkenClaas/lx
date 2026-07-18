use crate::danger;
use lx_config::Config;
use lx_core::error::LxError;
use lx_llm::{inject_lang, parse_response};
use lx_llm::{LlmClient, Request};
use serde::{Deserialize, Serialize};

pub const SYSTEM_TEMPLATE: &str = include_str!("../prompts/system.txt");
const MAX_TOKENS: u32 = 1024;

pub fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut days = secs / 86400;
    let mut year = 1970u64;
    loop {
        let diy = if is_leap(year) { 366 } else { 365 };
        if days < diy {
            break;
        }
        days -= diy;
        year += 1;
    }
    let month_days: [u64; 12] = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0usize;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    format!("{:04}-{:02}-{:02}", year, month + 1, days + 1)
}

fn is_leap(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rename {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RenameOutput {
    pub renames: Vec<Rename>,
    pub script: String,
    #[serde(default)]
    pub dangerous: bool,
}

impl RenameOutput {
    pub fn to_plain(&self) -> String {
        if self.script.is_empty() {
            String::from("\n")
        } else {
            format!("{}\n", self.script)
        }
    }
}

/// Build the mv script locally from the renames list.
pub fn build_script(renames: &[Rename]) -> String {
    renames
        .iter()
        .map(|r| format!("mv {:?} {:?}", r.from, r.to))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `file_list` is the list of files (one per line or from stdin). `intent` is the positional arg.
/// `dir_name` is the name of the source directory, if known (from --in path); None for stdin.
pub fn run(
    file_list: &str,
    intent: &str,
    dir_name: Option<&str>,
    config: &Config,
    client: &dyn LlmClient,
) -> Result<RenameOutput, LxError> {
    if intent.trim().is_empty() {
        return Err(LxError::BadUsage("no rename intent provided".to_string()));
    }
    // Check line-by-line, NOT `file_list.trim().is_empty()`: a filename may consist
    // entirely of blanks on Linux, and trimming the whole blob would make a lone
    // "   " entry indistinguishable from no input at all. `l.is_empty()` (not
    // `l.trim().is_empty()`) is deliberate for the same reason.
    if file_list.lines().all(|l| l.is_empty()) {
        return Err(LxError::BadUsage(
            "no file list provided; pipe a file list or use --in <path>".to_string(),
        ));
    }

    let today = today_utc();
    let system = inject_lang(SYSTEM_TEMPLATE, &config.output.lang).replace("{today}", &today);
    // Trim only the trailing newline(s), never the blanks: a leading or trailing
    // entry may legitimately be a blanks-only filename.
    let files = file_list.trim_end_matches('\n').trim_end_matches('\r');
    let user_msg = match dir_name {
        Some(dir) => format!(
            "Directory: {}\n\nIntent: {}\n\nFiles:\n{}",
            dir,
            intent.trim(),
            files
        ),
        None => format!("Intent: {}\n\nFiles:\n{}", intent.trim(), files),
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
    let mut out = parse_response::<RenameOutput>(&resp.content)?;

    if out.renames.is_empty() {
        return Err(LxError::LogicalError(
            "model returned no renames — the intent may require information unavailable at rename time (e.g. file timestamps)".to_string(),
        ));
    }

    // Build script locally — override whatever the model may have returned.
    out.script = build_script(&out.renames);

    // Scan for catastrophic patterns in the script.
    if danger::check_and_warn(&out.script) {
        out.dangerous = true;
    }

    Ok(out)
}
